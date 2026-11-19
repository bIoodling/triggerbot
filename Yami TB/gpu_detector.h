#pragma once
#include <wrl/client.h>
#include <dxgi1_2.h>
#include <d3d11.h>
#include <d3dcompiler.h>
#include "detection_core.h"
namespace yami {
using Microsoft::WRL::ComPtr;
inline double Milliseconds(){static const double f=[](){LARGE_INTEGER n;QueryPerformanceFrequency(&n);return double(n.QuadPart)/1000.;}();LARGE_INTEGER n;QueryPerformanceCounter(&n);return double(n.QuadPart)/f;}
struct GpuMeasurement {double submitMs=0,availableMs=0,shaderMs=-1;};
class GpuMatcher {
    ComPtr<ID3D11Device> device_;
    ComPtr<ID3D11DeviceContext> context_;
    ComPtr<ID3D11ComputeShader> shader_;
    ComPtr<ID3D11Texture2D> region_;
    ComPtr<ID3D11ShaderResourceView> srv_;
    ComPtr<ID3D11Buffer> result_,readback_,parameters_;
    ComPtr<ID3D11UnorderedAccessView> uav_;
    ComPtr<ID3D11Query> disjoint_,begin_,end_;
    bool pending_=false;
    double submitted_=0,submitMs_=0;
public:
    HRESULT error=S_OK;
    enum class Completion {Pending,Ready,Failed};
    ID3D11Device* Device() const {return device_.Get();}
    bool Initialize(ID3D11Device* device,const char* code,size_t length) {
        *this=GpuMatcher{};device_=device;device_->GetImmediateContext(&context_);
        ComPtr<ID3DBlob> compiled,errors;
        error=D3DCompile(code,length,"ColorDetect.hlsl",nullptr,nullptr,"main","cs_5_0",D3DCOMPILE_OPTIMIZATION_LEVEL3,0,&compiled,&errors);
        if(FAILED(error)){if(errors)OutputDebugStringA(static_cast<const char*>(errors->GetBufferPointer()));return false;}
        if(FAILED(error=device_->CreateComputeShader(compiled->GetBufferPointer(),compiled->GetBufferSize(),nullptr,&shader_)))return false;
        D3D11_TEXTURE2D_DESC texture{};texture.Width=257;texture.Height=257;texture.MipLevels=1;texture.ArraySize=1;texture.Format=DXGI_FORMAT_B8G8R8A8_UNORM;texture.SampleDesc.Count=1;texture.Usage=D3D11_USAGE_DEFAULT;texture.BindFlags=D3D11_BIND_SHADER_RESOURCE;
        if(FAILED(error=device_->CreateTexture2D(&texture,nullptr,&region_))||FAILED(error=device_->CreateShaderResourceView(region_.Get(),nullptr,&srv_)))return false;
        D3D11_BUFFER_DESC buffer{};buffer.ByteWidth=4;buffer.Usage=D3D11_USAGE_DEFAULT;buffer.BindFlags=D3D11_BIND_UNORDERED_ACCESS;buffer.MiscFlags=D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS;
        if(FAILED(error=device_->CreateBuffer(&buffer,nullptr,&result_)))return false;
        D3D11_UNORDERED_ACCESS_VIEW_DESC view{};view.Format=DXGI_FORMAT_R32_TYPELESS;view.ViewDimension=D3D11_UAV_DIMENSION_BUFFER;view.Buffer.NumElements=1;view.Buffer.Flags=D3D11_BUFFER_UAV_FLAG_RAW;
        if(FAILED(error=device_->CreateUnorderedAccessView(result_.Get(),&view,&uav_)))return false;
        buffer.BindFlags=0;buffer.MiscFlags=0;buffer.Usage=D3D11_USAGE_STAGING;buffer.CPUAccessFlags=D3D11_CPU_ACCESS_READ;
        if(FAILED(error=device_->CreateBuffer(&buffer,nullptr,&readback_)))return false;
        buffer.ByteWidth=48;buffer.Usage=D3D11_USAGE_DEFAULT;buffer.CPUAccessFlags=0;buffer.BindFlags=D3D11_BIND_CONSTANT_BUFFER;
        if(FAILED(error=device_->CreateBuffer(&buffer,nullptr,&parameters_)))return false;
        D3D11_QUERY_DESC q{D3D11_QUERY_TIMESTAMP_DISJOINT,0};
        if(FAILED(error=device_->CreateQuery(&q,&disjoint_)))return false;q.Query=D3D11_QUERY_TIMESTAMP;
        return SUCCEEDED(error=device_->CreateQuery(&q,&begin_))&&SUCCEEDED(error=device_->CreateQuery(&q,&end_));
    }
    bool Submit(ID3D11Texture2D* source,RECT region,Point center,unsigned radius,uint32_t rgb,unsigned tolerance) {
        if(pending_)return false;
        const UINT width=UINT(region.right-region.left),height=UINT(region.bottom-region.top);
        if(!width||!height||width>257||height>257||radius>128)return false;
        D3D11_TEXTURE2D_DESC sourceDesc{};source->GetDesc(&sourceDesc);
        if(region.left<0||region.top<0||region.right>LONG(sourceDesc.Width)||region.bottom>LONG(sourceDesc.Height)||sourceDesc.Format!=DXGI_FORMAT_B8G8R8A8_UNORM)return false;
        submitted_=Milliseconds();
        D3D11_BOX box{UINT(region.left),UINT(region.top),0,UINT(region.right),UINT(region.bottom),1};
        context_->Begin(disjoint_.Get());
        context_->CopySubresourceRegion(region_.Get(),0,0,0,0,source,0,&box);
        UINT constants[12]={width,height,UINT(center.x-region.left),UINT(center.y-region.top),(rgb>>16)&255,(rgb>>8)&255,rgb&255,tolerance,radius,0,0,0};
        context_->UpdateSubresource(parameters_.Get(),0,nullptr,constants,0,0);
        UINT clear[4]{};context_->ClearUnorderedAccessViewUint(uav_.Get(),clear);
        ID3D11ShaderResourceView* srvs[]={srv_.Get()};ID3D11UnorderedAccessView* uavs[]={uav_.Get()};ID3D11Buffer* constantsBuffer[]={parameters_.Get()};
        context_->CSSetShader(shader_.Get(),nullptr,0);context_->CSSetShaderResources(0,1,srvs);context_->CSSetUnorderedAccessViews(0,1,uavs,nullptr);context_->CSSetConstantBuffers(0,1,constantsBuffer);
        context_->End(begin_.Get());context_->Dispatch((width+7)/8,(height+7)/8,1);context_->End(end_.Get());
        ID3D11ShaderResourceView* noSrv=nullptr;ID3D11UnorderedAccessView* noUav=nullptr;context_->CSSetShaderResources(0,1,&noSrv);context_->CSSetUnorderedAccessViews(0,1,&noUav,nullptr);
        context_->CopyResource(readback_.Get(),result_.Get());context_->End(disjoint_.Get());
        // One submission flush is required for forward progress with a
        // nonblocking Map. No per-poll flush or blocking Map is used.
        context_->Flush();submitMs_=Milliseconds()-submitted_;pending_=true;return true;
    }
    Completion Poll(bool& found,GpuMeasurement& measurement) {
        if(!pending_)return Completion::Failed;
        D3D11_MAPPED_SUBRESOURCE mapped{};error=context_->Map(readback_.Get(),0,D3D11_MAP_READ,D3D11_MAP_FLAG_DO_NOT_WAIT,&mapped);
        if(error==DXGI_ERROR_WAS_STILL_DRAWING)return Completion::Pending;
        pending_=false;if(FAILED(error))return Completion::Failed;
        found=*static_cast<const uint32_t*>(mapped.pData)!=0;context_->Unmap(readback_.Get(),0);
        measurement.submitMs=submitMs_;measurement.availableMs=Milliseconds()-submitted_;measurement.shaderMs=-1;
        D3D11_QUERY_DATA_TIMESTAMP_DISJOINT timing{};UINT64 start=0,stop=0;
        if(context_->GetData(disjoint_.Get(),&timing,sizeof(timing),D3D11_ASYNC_GETDATA_DONOTFLUSH)==S_OK&&!timing.Disjoint&&timing.Frequency&&
           context_->GetData(begin_.Get(),&start,sizeof(start),D3D11_ASYNC_GETDATA_DONOTFLUSH)==S_OK&&context_->GetData(end_.Get(),&stop,sizeof(stop),D3D11_ASYNC_GETDATA_DONOTFLUSH)==S_OK)
            measurement.shaderMs=double(stop-start)*1000/double(timing.Frequency);
        return Completion::Ready;
    }
};
class DesktopCapture {
    ComPtr<ID3D11Device> device_;
    ComPtr<ID3D11DeviceContext> context_;
    ComPtr<IDXGIOutputDuplication> duplication_;
    DXGI_OUTPUT_DESC output_{};
    bool acquired_=false;
public:
    HRESULT error=S_OK;
    ID3D11Device* Device()const{return device_.Get();}
    HMONITOR Monitor()const{return output_.Monitor;}
    bool Initialize(HMONITOR monitor) {
        Release();duplication_.Reset();context_.Reset();device_.Reset();output_={};
        ComPtr<IDXGIFactory1> factory;
        if(FAILED(error=CreateDXGIFactory1(IID_PPV_ARGS(&factory))))return false;
        for(UINT ai=0;;++ai){ComPtr<IDXGIAdapter1> adapter;if(factory->EnumAdapters1(ai,&adapter)==DXGI_ERROR_NOT_FOUND)break;
            if(!adapter)continue;
            for(UINT oi=0;;++oi){ComPtr<IDXGIOutput> output;if(adapter->EnumOutputs(oi,&output)==DXGI_ERROR_NOT_FOUND)break;if(!output)continue;
                DXGI_OUTPUT_DESC description{};if(FAILED(output->GetDesc(&description))||description.Monitor!=monitor)continue;
                D3D_FEATURE_LEVEL feature=D3D_FEATURE_LEVEL_11_0;
                if(FAILED(error=D3D11CreateDevice(adapter.Get(),D3D_DRIVER_TYPE_UNKNOWN,nullptr,D3D11_CREATE_DEVICE_BGRA_SUPPORT,&feature,1,D3D11_SDK_VERSION,&device_,nullptr,&context_)))return false;
                ComPtr<IDXGIOutput1> one;if(FAILED(error=output.As(&one))||FAILED(error=one->DuplicateOutput(device_.Get(),&duplication_)))return false;
                output_=description;return true;
            }
        }error=DXGI_ERROR_NOT_FOUND;return false;
    }
    enum class Status {Frame,Timeout,Failure};
    Status Acquire(ComPtr<ID3D11Texture2D>& texture,double& captureMs,UINT timeout=8) {
        texture.Reset();if(!duplication_)return Status::Failure;Release();
        DXGI_OUTDUPL_FRAME_INFO info{};ComPtr<IDXGIResource> resource;double start=Milliseconds();
        error=duplication_->AcquireNextFrame(timeout,&info,&resource);captureMs=Milliseconds()-start;
        if(error==DXGI_ERROR_WAIT_TIMEOUT)return Status::Timeout;
        if(FAILED(error)||!resource)return Status::Failure;
        acquired_=true;if(FAILED(error=resource.As(&texture))){Release();return Status::Failure;}return Status::Frame;
    }
    Point TexturePoint(Point physical,ID3D11Texture2D* texture)const {
        D3D11_TEXTURE2D_DESC d{};texture->GetDesc(&d);int x=physical.x-output_.DesktopCoordinates.left,y=physical.y-output_.DesktopCoordinates.top;
        if(output_.Rotation==DXGI_MODE_ROTATION_ROTATE90)return {y,int(d.Height)-1-x};
        if(output_.Rotation==DXGI_MODE_ROTATION_ROTATE180)return {int(d.Width)-1-x,int(d.Height)-1-y};
        if(output_.Rotation==DXGI_MODE_ROTATION_ROTATE270)return {int(d.Width)-1-y,x};return {x,y};
    }
    void Release(){if(acquired_&&duplication_)duplication_->ReleaseFrame();acquired_=false;}
    ~DesktopCapture(){Release();}
};
inline RECT RegionAround(Point center,unsigned radius,unsigned width,unsigned height) {
    return {std::max(0,center.x-int(radius)),std::max(0,center.y-int(radius)),std::min(int(width),center.x+int(radius)+1),std::min(int(height),center.y+int(radius)+1)};
}
} // namespace yami
