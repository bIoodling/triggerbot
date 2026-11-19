#include "gpu_detector.h"
#include <psapi.h>

yami::SingleTrigger g_singleTrigger;
void SetEngineActive(bool active){g_singleTrigger.SetActive(active);}
void ResetEngineConfiguration(){g_singleTrigger.Reset();if(g_activationWakeEvent)SetEvent(g_activationWakeEvent);}
void ReplaceDetectionOptions(const DetectionOptions& options){g_singleTrigger.Configure([&](){std::lock_guard lock(g_optionsMutex);g_options=options;});if(g_activationWakeEvent)SetEvent(g_activationWakeEvent);}
double QpcNowSeconds(){return yami::Milliseconds()/1000.;}

bool IsMouseOutput(int vk){return vk==VK_LBUTTON||vk==VK_RBUTTON||vk==VK_MBUTTON||vk==VK_XBUTTON1||vk==VK_XBUTTON2;}
struct OutputPair { INPUT down{},up{};bool valid=false; };
OutputPair BuildOutputPair(int vk){
    OutputPair pair;
    if(vk<1||vk>254)return pair;
    if(IsMouseOutput(vk)){
        pair.down.type=INPUT_MOUSE;pair.up.type=INPUT_MOUSE;
        switch(vk){
        case VK_LBUTTON:pair.down.mi.dwFlags=MOUSEEVENTF_LEFTDOWN;pair.up.mi.dwFlags=MOUSEEVENTF_LEFTUP;break;
        case VK_RBUTTON:pair.down.mi.dwFlags=MOUSEEVENTF_RIGHTDOWN;pair.up.mi.dwFlags=MOUSEEVENTF_RIGHTUP;break;
        case VK_MBUTTON:pair.down.mi.dwFlags=MOUSEEVENTF_MIDDLEDOWN;pair.up.mi.dwFlags=MOUSEEVENTF_MIDDLEUP;break;
        default:pair.down.mi.dwFlags=MOUSEEVENTF_XDOWN;pair.up.mi.dwFlags=MOUSEEVENTF_XUP;pair.down.mi.mouseData=pair.up.mi.mouseData=vk==VK_XBUTTON1?XBUTTON1:XBUTTON2;break;
        }
    }else{
        UINT scan=MapVirtualKeyExW(vk,MAPVK_VK_TO_VSC_EX,GetKeyboardLayout(0));
        pair.down.type=INPUT_KEYBOARD;
        if(scan){pair.down.ki.wScan=WORD(scan&255);pair.down.ki.dwFlags=KEYEVENTF_SCANCODE|((scan&0xFF00)==0xE000?KEYEVENTF_EXTENDEDKEY:0);}
        else pair.down.ki.wVk=WORD(vk);
        pair.up=pair.down;pair.up.ki.dwFlags|=KEYEVENTF_KEYUP;
    }
    if(pair.down.type==INPUT_MOUSE){pair.down.mi.dwExtraInfo=pair.up.mi.dwExtraInfo=YAMI_OUTPUT_TAG;}
    else pair.down.ki.dwExtraInfo=pair.up.ki.dwExtraInfo=YAMI_OUTPUT_TAG;
    pair.valid=true;return pair;
}
bool SendOutputKeyFast(){
    // Scanner thread is the only caller. The pair is sent synchronously;
    // there is no queue, timer, callback, hook or second output dispatcher.
    static thread_local int cached=-1;static thread_local OutputPair prepared;
    int vk=g_outputVK.load();if(vk!=cached){cached=vk;prepared=BuildOutputPair(vk);}if(!prepared.valid)return false;
    INPUT pair[]={prepared.down,prepared.up};UINT sent=SendInput(2,pair,sizeof(INPUT));
    if(sent==1){INPUT release=prepared.up;SendInput(1,&release,sizeof(INPUT));}
    return sent==2;
}
bool IsRobloxWindow(HWND hwnd){
    if(!IsWindow(hwnd)||!IsWindowVisible(hwnd)||IsIconic(hwnd))return false;
    DWORD pid=0;GetWindowThreadProcessId(hwnd,&pid);HANDLE process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,FALSE,pid);if(!process)return false;
    wchar_t path[32768]{};DWORD size=ARRAYSIZE(path);bool ok=QueryFullProcessImageNameW(process,0,path,&size)!=FALSE;CloseHandle(process);
    if(!ok)return false;const wchar_t* name=wcsrchr(path,L'\\');name=name?name+1:path;
    return _wcsicmp(name,L"RobloxPlayerBeta.exe")==0||_wcsicmp(name,L"Windows10Universal.exe")==0;
}
HWND FindGameViewport(){
    HWND hint=HWND(g_viewportHint.load());if(IsRobloxWindow(hint))return hint;
    HWND foreground=GetAncestor(GetForegroundWindow(),GA_ROOT);if(IsRobloxWindow(foreground)){g_viewportHint=uintptr_t(foreground);return foreground;}
    HWND found=nullptr;EnumWindows([](HWND hwnd,LPARAM data)->BOOL{if(IsRobloxWindow(hwnd)){*reinterpret_cast<HWND*>(data)=hwnd;return FALSE;}return TRUE;},LPARAM(&found));
    if(found)g_viewportHint=uintptr_t(found);return found;
}
bool ClientViewport(HWND hwnd,yami::Viewport& viewport){
    if(!IsWindow(hwnd)||!IsWindowVisible(hwnd)||IsIconic(hwnd))return false;RECT r{};if(!GetClientRect(hwnd,&r)||r.right<=0||r.bottom<=0)return false;
    POINT origin{};if(!ClientToScreen(hwnd,&origin))return false;viewport={origin.x,origin.y,r.right,r.bottom};return true;
}
bool ResolveDetectionPoint(const DetectionOptions& options,yami::Point& point){
    if(!options.cursorless){POINT cursor{};if(!GetCursorPos(&cursor))return false;point={cursor.x,cursor.y};return true;}
    if(!options.calibrated)return false;yami::Viewport viewport;
    HWND target=FindGameViewport();
#ifdef YAMI_TEST_BUILD
    // A test-owned client replaces only Roblox process discovery. Capture,
    // shaders, coordinates, state transitions and SendInput stay production.
    extern HWND g_testViewport;
    if(g_testViewport)target=g_testViewport;
#endif
    if(!ClientViewport(target,viewport))return false;
    // Only the actual Roblox client viewport is used; no assumed full-screen
    // monitor, DPI-virtualized coordinates or hardcoded camera position.
    if(GetAncestor(GetForegroundWindow(),GA_ROOT)!=target)return false;
    point=yami::ResolvePoint(viewport,options.normalizedX,options.normalizedY);return true;
}
const char* ShaderResource(size_t& length){HRSRC resource=FindResourceW(GetModuleHandleW(nullptr),MAKEINTRESOURCEW(101),RT_RCDATA);if(!resource){length=0;return nullptr;}length=SizeofResource(GetModuleHandleW(nullptr),resource);return static_cast<const char*>(LockResource(LoadResource(GetModuleHandleW(nullptr),resource)));}
bool PhysicalHoldAllowed(){
    return g_running.load()&&g_activationActive.load()&&g_toggleState.load()&&!g_activationCapture.load()&&!g_engineSuspended.load()&&IsActivationBindDown(g_holdActivationVK.load());
}
void ScannerThread(){
    SetThreadPriority(GetCurrentThread(),THREAD_PRIORITY_ABOVE_NORMAL);
    yami::DesktopCapture capture;yami::GpuMatcher gpu;bool ready=false;uint64_t sequence=0;
    size_t shaderLength=0;const char* shader=ShaderResource(shaderLength);
    if(!shader){OutputDebugStringW(L"Yami: embedded detector shader missing.\n");return;}
    // Initialize in this background thread before Hold, so the normal first
    // activation does not pay shader/device creation latency on the UI thread.
    HMONITOR initialMonitor=MonitorFromPoint(POINT{0,0},MONITOR_DEFAULTTOPRIMARY);
    ready=capture.Initialize(initialMonitor)&&gpu.Initialize(capture.Device(),shader,shaderLength);
    g_detectorReady=ready;
    auto failure=[&](){g_detectorReady=false;g_greenDetected=false;g_singleTrigger.Frame(g_singleTrigger.Token(),++sequence,yami::Milliseconds(),false,false,{},[](){return false;},[](){return false;});};
    while(g_running.load()){
        if(!g_activationActive.load()){
            g_scanning=false;g_greenDetected=false;
            if(g_activationWakeEvent){ResetEvent(g_activationWakeEvent);if(!g_activationActive.load()&&g_running.load())WaitForSingleObject(g_activationWakeEvent,INFINITE);}else Sleep(1);
            continue;
        }
        if(!PhysicalHoldAllowed()){g_holdKeyDown=false;UpdateActivationActive();continue;}
        const auto options=ReadDetectionOptions();const uint32_t target=g_targetColor.load();const int output=g_outputVK.load(),hold=g_holdActivationVK.load();
        const uint64_t token=g_singleTrigger.Token();yami::Point center;
        if(!ResolveDetectionPoint(options,center)){failure();if(g_activationWakeEvent)WaitForSingleObject(g_activationWakeEvent,50);continue;}
        HMONITOR monitor=MonitorFromPoint(POINT{center.x,center.y},MONITOR_DEFAULTTONULL);
        if(!monitor){failure();if(g_activationWakeEvent)WaitForSingleObject(g_activationWakeEvent,50);continue;}
        if(!ready||capture.Monitor()!=monitor){
            ready=capture.Initialize(monitor)&&gpu.Initialize(capture.Device(),shader,shaderLength);g_detectorReady=ready;
            if(!ready){failure();if(g_activationWakeEvent)WaitForSingleObject(g_activationWakeEvent,250);continue;}
        }
        yami::ComPtr<ID3D11Texture2D> frame;double captureMs=0;
        auto acquired=capture.Acquire(frame,captureMs);
        if(acquired==yami::DesktopCapture::Status::Timeout)continue;
        if(acquired==yami::DesktopCapture::Status::Failure){ready=false;failure();continue;}
        center=capture.TexturePoint(center,frame.Get());D3D11_TEXTURE2D_DESC description{};frame->GetDesc(&description);
        const unsigned radius=options.cursorless?options.fixedRadius:options.cursorRadius;
        RECT region=yami::RegionAround(center,radius,description.Width,description.Height);
        bool submitted=gpu.Submit(frame.Get(),region,center,radius,target,options.tolerance);capture.Release();frame.Reset();
        if(!submitted){ready=false;failure();continue;}
        bool found=false;yami::GpuMeasurement timing;auto status=yami::GpuMatcher::Completion::Pending;unsigned polls=0;
        do{status=gpu.Poll(found,timing);if(status==yami::GpuMatcher::Completion::Pending){if(++polls%128==0)SwitchToThread();else _mm_pause();}}while(status==yami::GpuMatcher::Completion::Pending&&g_running.load());
        if(!g_running.load())break;
        if(status==yami::GpuMatcher::Completion::Failed){ready=false;failure();continue;}
        // Parameters changed while the GPU worked: discard the entire result.
        if(token!=g_singleTrigger.Token()||options!=ReadDetectionOptions()||target!=g_targetColor.load()||output!=g_outputVK.load()||hold!=g_holdActivationVK.load())continue;
        g_scanning=true;g_greenDetected=found;
        yami::Rules rules{options.clearMs,options.clearFrames,options.minimumIntervalMs};
        g_singleTrigger.Frame(token,++sequence,yami::Milliseconds(),true,found,rules,
            [&](){return output!=0&&PhysicalHoldAllowed()&&target==g_targetColor.load()&&output==g_outputVK.load()&&hold==g_holdActivationVK.load()&&options==ReadDetectionOptions();},
            [](){return SendOutputKeyFast();});
    }
    capture.Release();g_scanning=false;g_greenDetected=false;g_detectorReady=false;SetEngineActive(false);
}
