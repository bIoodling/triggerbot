#include "../Yami TB/activation.inc"
#include "../Yami TB/settings.inc"
#include "../Yami TB/engine.inc"
#include <iostream>
#include <iomanip>
#include <functional>
#include <stdexcept>
#include <random>
#include <barrier>
#include <numeric>

bool ConsumeBindingInput(int,bool,bool){return false;}
HWND g_testViewport=nullptr;
int passed=0,failed=0;
std::vector<std::string> outcomes;
void require(bool condition,const char* message){if(!condition)throw std::runtime_error(message);}
void test(const std::string& name,const std::function<void()>& body){try{body();++passed;outcomes.push_back("PASS "+name);std::cout<<"PASS "<<name<<"\n";}catch(const std::exception& e){++failed;outcomes.push_back("FAIL "+name+": "+e.what());std::cout<<outcomes.back()<<"\n";}}
struct Sequence {
    yami::SingleTrigger engine;uint64_t id=0;int outputs=0;yami::Rules rules{8,2,0};
    Sequence(){engine.SetActive(true);}
    bool frame(double time,bool found,bool valid=true){return engine.Frame(engine.Token(),++id,time,valid,found,rules,[](){return true;},[&](){++outputs;return true;});}
};
void stateTests(){
    test("A continuous colour: 500 ms, exactly one",[]{Sequence s;for(int i=0;i<=500;++i)s.frame(i,true);require(s.outputs==1,"continuous highlight duplicated");});
    test("B brief single-frame flicker: no duplicate",[]{Sequence s;s.frame(0,true);s.frame(7,false);s.frame(8,true);require(s.outputs==1,"one negative frame rearmed");});
    test("C qualified clear: exactly two appearances",[]{Sequence s;s.frame(0,true);s.frame(1,false);s.frame(9,false);require(s.engine.State()==yami::Phase::Rearmed,"not rearmed after qualified clear");s.frame(10,true);require(s.outputs==2,"independent appearance missed");});
    test("D Hold release rejects all queued/stale completions",[]{Sequence s;auto old=s.engine.Token();s.frame(0,true);s.engine.SetActive(false);for(int i=2;i<200;++i)s.engine.Frame(old,i,i,true,true,s.rules,[](){return true;},[&](){++s.outputs;return true;});require(s.outputs==1&&s.engine.State()==yami::Phase::Idle,"released hold dispatched");s.engine.SetActive(true);s.engine.Frame(old,500,500,true,true,s.rules,[](){return true;},[&](){++s.outputs;return true;});require(s.outputs==1,"old activation token accepted");});
    test("E rapid transitions: 100 qualified appearances",[]{Sequence s;for(int i=0;i<100;++i){double t=i*12.;s.frame(t,true);s.frame(t+.1,true);s.frame(t+1,false);s.frame(t+9,false);}require(s.outputs==100,"rapid transitions count incorrect");});
    test("F normalized coordinates, DPI/negative origins/window sizes",[]{
        const int sizes[][2]={{1440,1080},{1920,1080},{2560,1440},{3840,2160},{800,600},{1037,743}};
        for(auto& size:sizes)for(double dpi:{1.,1.25,1.5,2.}){(void)dpi;for(auto origin:{yami::Point{0,0},yami::Point{-1920,73},yami::Point{213,119}}){auto p=yami::ResolvePoint({origin.x,origin.y,size[0],size[1]},.375,.625);require(p.x==origin.x+int(std::lround(size[0]*.375))&&p.y==origin.y+int(std::lround(size[1]*.625)),"normalized placement wrong");auto edge=yami::ResolvePoint({origin.x,origin.y,size[0],size[1]},1,1);require(edge.x==origin.x+size[0]-1&&edge.y==origin.y+size[1]-1,"viewport edge escaped");for(int selected=0;selected<size[0];++selected){auto roundtrip=yami::ResolvePoint({origin.x,origin.y,size[0],size[1]},double(selected)/size[0],.5);require(roundtrip.x==origin.x+selected,"calibration normalization drifted a pixel");}}}
    });
    test("G RGB reference tolerance and HEX validity",[]{for(uint32_t color:{0u,0xFFFFFFu,0x00FF00u,0xFF0000u,0x0000FFu,0xD14EC7u,0x123456u}){require(yami::MatchRgb(color,color,0),"exact RGB not matched");uint32_t parsed=0;require(ParseHex(HexColor(color),parsed)&&parsed==color,"HEX roundtrip failed");}uint32_t v;require(!ParseHex(L"#GG0011",v)&&!ParseHex(L"#12345",v)&&!ParseHex(L"1234567",v),"invalid HEX accepted");require(yami::MatchRgb(0x123458,0x123456,2)&&!yami::MatchRgb(0x123459,0x123456,2),"channel tolerance wrong");});
    test("H concurrent completions: one dispatch owner",[]{for(int run=0;run<30;++run){yami::SingleTrigger engine;engine.SetActive(true);auto token=engine.Token();std::atomic<int> sent=0;std::barrier gate(16);std::vector<std::thread> workers;for(int i=0;i<16;++i)workers.emplace_back([&,i](){gate.arrive_and_wait();for(int j=0;j<100;++j)engine.Frame(token,uint64_t(1+i*100+j),0,true,true,{},[](){return true;},[&](){++sent;return true;});});for(auto& worker:workers)worker.join();require(sent==1,"concurrent duplicate");}});
    test("Capture failure never clears/rearms",[]{Sequence s;s.frame(0,true);s.frame(1,false);s.frame(20,false,false);s.frame(21,false);s.frame(22,true);require(s.outputs==1,"capture failure counted as absence");});
    test("Out-of-order frame and configuration token discarded",[]{Sequence s;s.frame(0,true);auto token=s.engine.Token();s.engine.Reset();s.engine.Frame(token,100,50,true,true,s.rules,[](){return true;},[&](){++s.outputs;return true;});require(s.outputs==1,"stale config dispatched");});
    test("Optional interval consumes suppressed appearance without queue",[]{Sequence s;s.rules.minimumIntervalMs=100;s.frame(0,true);s.frame(1,false);s.frame(9,false);s.frame(10,true);s.frame(200,true);require(s.outputs==1,"rate limit queued a late trigger");});
    test("Output keyboard/mouse down-up pair construction",[]{for(int key:{int('F'),VK_RCONTROL,VK_LSHIFT,VK_F24,VK_LBUTTON,VK_RBUTTON,VK_MBUTTON,VK_XBUTTON1,VK_XBUTTON2}){auto p=BuildOutputPair(key);require(p.valid,"valid bind rejected");if(IsMouseOutput(key))require(p.down.type==INPUT_MOUSE&&p.up.type==INPUT_MOUSE&&p.down.mi.dwFlags!=p.up.mi.dwFlags,"mouse encoded as keyboard");else require(p.down.type==INPUT_KEYBOARD&&(p.up.ki.dwFlags&KEYEVENTF_KEYUP)&&!(p.down.ki.dwFlags&KEYEVENTF_KEYUP),"keyboard sequencing wrong");}require(!BuildOutputPair(0).valid,"unset output is valid");});
    test("Settings persistence includes engine and normalized point",[]{
        wchar_t temp[MAX_PATH];GetTempPathW(ARRAYSIZE(temp),temp);g_settingsOverride=std::wstring(temp)+L"YamiHarness-"+std::to_wstring(GetCurrentProcessId());CreateDirectoryW(g_settingsOverride.c_str(),nullptr);
        g_activationVK=VK_XBUTTON1;g_holdActivationVK=VK_RSHIFT;g_outputVK='F';g_targetColor=0xD14EC7;
        EditDetectionOptions([](auto& o){o.cursorless=true;o.calibrated=true;o.normalizedX=.375;o.normalizedY=.625;o.tolerance=3;o.clearMs=6.5;o.fixedRadius=9;o.minimumIntervalMs=2;});SaveSettings();require(!g_saveFailed,"save failed");
        g_activationVK=0;g_holdActivationVK=0;g_outputVK=0;g_targetColor=0;LoadSettings();auto o=ReadDetectionOptions();require(g_activationVK==VK_XBUTTON1&&g_holdActivationVK==VK_RSHIFT&&g_outputVK=='F'&&g_targetColor==0xD14EC7,"bind or colour reload failed");require(o.cursorless&&o.calibrated&&o.normalizedX==.375&&o.normalizedY==.625&&o.clearMs==6.5&&o.fixedRadius==9&&o.minimumIntervalMs==2,"options reload failed");DeleteFileW(GetSettingsPath().c_str());RemoveDirectoryW(g_settingsOverride.c_str());g_settingsOverride.clear();
    });
}
struct Stats {
    std::vector<double> values;
    void add(double v){if(v>=0)values.push_back(v);}
    double percentile(double q)const{if(values.empty())return -1;auto sorted=values;std::sort(sorted.begin(),sorted.end());return sorted[size_t(std::ceil(q*sorted.size()))-1];}
    std::string json()const{std::ostringstream out;out<<std::fixed<<std::setprecision(6)<<"{\"samples\":"<<values.size()<<",\"median_ms\":"<<percentile(.5)<<",\"p95_ms\":"<<percentile(.95)<<",\"p99_ms\":"<<percentile(.99)<<"}";return out.str();}
};
Stats submitStats,gpuStats,readbackStats,captureStats,dispatchStats,detectDispatchStats,e2eStats;
int gpuCases=0,falsePositive=0,falseNegative=0,positiveCases=0,negativeCases=0,liveEvents=0;std::string adapterName;
yami::ComPtr<ID3D11Device> testDevice;
yami::ComPtr<ID3D11DeviceContext> testContext;
yami::GpuMatcher matcher;
yami::ComPtr<ID3D11Texture2D> synthetic;
std::array<uint32_t,65*65> pixels;
bool evaluate(uint32_t target,uint32_t pixel,unsigned tolerance){
    uint32_t background=(target^0xFFFFFF)&0xFFFFFF;
    auto bgra=[](uint32_t c){return 0xFF000000|c;}; // Little-endian 0xAARRGGBB is BGRA.
    pixels.fill(bgra(background));pixels[32*65+32]=bgra(pixel);testContext->UpdateSubresource(synthetic.Get(),0,nullptr,pixels.data(),65*4,0);
    require(matcher.Submit(synthetic.Get(),{16,16,49,49},{32,32},16,target,tolerance),"GPU submit failed");
    bool found=false;yami::GpuMeasurement timing;auto status=yami::GpuMatcher::Completion::Pending;double deadline=yami::Milliseconds()+3000;
    do{status=matcher.Poll(found,timing);if(status==yami::GpuMatcher::Completion::Pending)SwitchToThread();require(yami::Milliseconds()<deadline,"GPU timeout");}while(status==yami::GpuMatcher::Completion::Pending);
    require(status==yami::GpuMatcher::Completion::Ready,"GPU readback failed");submitStats.add(timing.submitMs);gpuStats.add(timing.shaderMs);readbackStats.add(timing.availableMs);
    bool expected=yami::MatchRgb(pixel,target,tolerance);++gpuCases;if(expected)++positiveCases;else ++negativeCases;if(found&&!expected)++falsePositive;if(!found&&expected)++falseNegative;
    require(found==expected,"HLSL RGB result disagrees with reference");return found;
}
void gpuTests(){
    test("Hardware D3D11 shader initialization",[]{D3D_FEATURE_LEVEL level=D3D_FEATURE_LEVEL_11_0;HRESULT hr=D3D11CreateDevice(nullptr,D3D_DRIVER_TYPE_HARDWARE,nullptr,D3D11_CREATE_DEVICE_BGRA_SUPPORT,&level,1,D3D11_SDK_VERSION,&testDevice,nullptr,&testContext);require(SUCCEEDED(hr),"no hardware D3D11 device");yami::ComPtr<IDXGIDevice> dxgi;testDevice.As(&dxgi);yami::ComPtr<IDXGIAdapter> adapter;dxgi->GetAdapter(&adapter);DXGI_ADAPTER_DESC description;adapter->GetDesc(&description);for(wchar_t c:std::wstring(description.Description))adapterName+=c<128?char(c):'?';size_t length;auto code=ShaderResource(length);require(code&&matcher.Initialize(testDevice.Get(),code,length),"shader compilation/resource setup failed");D3D11_TEXTURE2D_DESC d{};d.Width=d.Height=65;d.MipLevels=d.ArraySize=1;d.Format=DXGI_FORMAT_B8G8R8A8_UNORM;d.SampleDesc.Count=1;d.Usage=D3D11_USAGE_DEFAULT;require(SUCCEEDED(testDevice->CreateTexture2D(&d,nullptr,&synthetic)),"synthetic texture failed");});
    if(!testDevice||!synthetic)return;
    test("G actual GPU exact/near/wrong RGB: endpoints and 256 random colours",[]{std::mt19937 rng(0x59414D49);for(uint32_t color:{0u,0xFFFFFFu,0xFF0000u,0x00FF00u,0x0000FFu,0x010101u,0xFEFEFEu,0x123456u}){evaluate(color,color,0);evaluate(color,color^0xFFFFFF,2);}for(int i=0;i<256;++i){uint32_t c=rng()&0xFFFFFF;evaluate(c,c,0);uint32_t nearColor=(c&0xFFFF00)|unsigned(std::clamp(int(c&255)+2,0,255));evaluate(c,nearColor,2);evaluate(c,c^0xFFFFFF,2);}require(falsePositive==0&&falseNegative==0,"GPU classification errors");});
    test("GPU pending result: Hold released before completion",[]{yami::SingleTrigger gate;gate.SetActive(true);auto token=gate.Token();bool found=evaluate(0x123456,0x123456,0);gate.SetActive(false);int sent=0;gate.Frame(token,1,0,true,found,{},[](){return true;},[&](){++sent;return true;});require(sent==0,"GPU completion survived release");});
}

std::atomic<int> receivedDown=0,receivedUp=0;
std::atomic<bool> highlight=false;
double paintStarted=0,firstReceived=0;
uint32_t sceneColor=0xD14EC7;
unsigned backgroundTick=0;
LRESULT CALLBACK SceneProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){
    if(msg==WM_PAINT){PAINTSTRUCT ps;HDC dc=BeginPaint(hwnd,&ps);RECT r;GetClientRect(hwnd,&r);HBRUSH background=CreateSolidBrush(RGB(8+(backgroundTick&1),8,12));FillRect(dc,&r,background);DeleteObject(background);if(highlight.load()){RECT patch{r.right/2-5,r.bottom/2-5,r.right/2+6,r.bottom/2+6};HBRUSH brush=CreateSolidBrush(RGB((sceneColor>>16)&255,(sceneColor>>8)&255,sceneColor&255));FillRect(dc,&patch,brush);DeleteObject(brush);}EndPaint(hwnd,&ps);return 0;}
    if(msg==WM_KEYDOWN&&w==VK_F24){++receivedDown;firstReceived=yami::Milliseconds();if(paintStarted)e2eStats.add(firstReceived-paintStarted);return 0;}
    if(msg==WM_KEYUP&&w==VK_F24){++receivedUp;return 0;}
    if(msg==WM_CLOSE)return 0;
    return DefWindowProcW(hwnd,msg,w,l);
}
void pump(unsigned duration,bool animate=true){double end=yami::Milliseconds()+duration,next=0;do{MSG message;while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}if(animate&&g_testViewport&&yami::Milliseconds()>=next){++backgroundTick;InvalidateRect(g_testViewport,nullptr,FALSE);UpdateWindow(g_testViewport);next=yami::Milliseconds()+8;}Sleep(1);}while(yami::Milliseconds()<end);}
bool until(const std::function<bool()>& condition,unsigned timeout){double end=yami::Milliseconds()+timeout;while(!condition()&&yami::Milliseconds()<end)pump(2);return condition();}
void physicalHold(bool down){require(GetForegroundWindow()==g_testViewport,"test receiver lost focus; refusing input");auto pair=BuildOutputPair(VK_F23);INPUT input=down?pair.down:pair.up;require(SendInput(1,&input,sizeof(INPUT))==1,"test Hold input blocked");pump(8);HandleActivationEvent(VK_F23,down,!down);}
void scene(bool found){highlight=found;++backgroundTick;paintStarted=found?yami::Milliseconds():0;InvalidateRect(g_testViewport,nullptr,FALSE);UpdateWindow(g_testViewport);}
void liveTests(){
    WNDCLASSEXW wc{sizeof(wc)};wc.hInstance=GetModuleHandleW(nullptr);wc.lpfnWndProc=SceneProc;wc.lpszClassName=L"YamiControlledTestReceiver";wc.hCursor=LoadCursorW(nullptr,IDC_ARROW);RegisterClassExW(&wc);
    g_testViewport=CreateWindowExW(WS_EX_TOPMOST,wc.lpszClassName,L"Yami controlled capture/input test",WS_OVERLAPPEDWINDOW,120,100,640,480,nullptr,nullptr,wc.hInstance,nullptr);require(g_testViewport!=nullptr,"test receiver creation failed");ShowWindow(g_testViewport,SW_SHOW);SetForegroundWindow(g_testViewport);pump(40);
    if(GetForegroundWindow()!=g_testViewport){++failed;outcomes.push_back("FAIL live pipeline: receiver could not obtain focus");DestroyWindow(g_testViewport);g_testViewport=nullptr;return;}
    g_activationVK=0;g_holdActivationVK=VK_F23;g_outputVK=VK_F24;g_targetColor=sceneColor;g_toggleState=true;g_holdKeyDown=false;g_activationActive=false;g_running=true;SetEngineActive(false);
    EditDetectionOptions([](auto& o){o=DetectionOptions{};o.cursorless=true;o.calibrated=true;o.normalizedX=.5;o.normalizedY=.5;o.tolerance=0;o.clearMs=8;o.fixedRadius=12;});
    g_activationWakeEvent=CreateEventW(nullptr,TRUE,FALSE,nullptr);g_scannerThread=CreateThread(nullptr,0,[](LPVOID)->DWORD{ScannerThread();return 0;},nullptr,0,nullptr);
    test("Live desktop duplication -> HLSL -> F24 receiver, continuous highlight",[]{scene(false);physicalHold(true);require(until([](){return g_detectorReady.load();},3000),"desktop duplication unavailable");pump(80);scene(true);require(until([](){return receivedDown.load()>=1;},2000),"actual output was not received");paintStarted=0;pump(500);require(receivedDown==1&&receivedUp==1,"continuous desktop target produced duplicate/unpaired input");});
    test("Live genuine clear/reappearance outputs exactly twice",[]{scene(false);pump(120);scene(true);require(until([](){return receivedDown.load()>=2;},1500),"second appearance missed");paintStarted=0;pump(80);require(receivedDown==2&&receivedUp==2,"second desktop event duplicated");});
    test("Live Hold release with target visible: no late output",[]{physicalHold(false);int count=receivedDown.load();scene(false);pump(80);scene(true);pump(120);require(receivedDown==count&&!g_activationActive.load(),"output after Hold release");});
    test("Live input/capture latency: 30 controlled new appearances",[]{
        scene(false);pump(80);physicalHold(true);pump(80);
        for(int i=0;i<30;++i){scene(false);pump(50);int before=receivedDown.load();scene(true);require(until([&](){return receivedDown.load()>before;},1000),"benchmark appearance missed");paintStarted=0;pump(8);require(receivedDown==before+1,"benchmark duplicate");}
        physicalHold(false);require(receivedDown==receivedUp,"unpaired live keyboard events");
    });
    test("Live original cursor mode uses the same single-trigger engine",[]{
        POINT savedCursor{};GetCursorPos(&savedCursor);yami::Viewport viewport;ClientViewport(g_testViewport,viewport);auto center=yami::ResolvePoint(viewport,.5,.5);
        scene(false);EditDetectionOptions([](auto& o){o.cursorless=false;});SetCursorPos(center.x,center.y);pump(60);physicalHold(true);pump(60);
        int before=receivedDown.load();scene(true);bool delivered=until([&](){return receivedDown.load()>before;},1500);paintStarted=0;pump(150);physicalHold(false);SetCursorPos(savedCursor.x,savedCursor.y);
        require(delivered&&receivedDown==before+1&&receivedDown==receivedUp,"cursor mode missed/duplicated output");
    });
    liveEvents=receivedDown.load();
    // Direct capture benchmark shares the production DesktopCapture/GpuMatcher.
    // DXGI does not permit a second duplication of this output from the same
    // process. Finish the scanner before creating the benchmark capture.
    g_running=false;g_activationActive=false;SetEngineActive(false);SetEvent(g_activationWakeEvent);
    while(WaitForSingleObject(g_scannerThread,10)==WAIT_TIMEOUT){SetEvent(g_activationWakeEvent);pump(1,false);}
    CloseHandle(g_scannerThread);CloseHandle(g_activationWakeEvent);g_scannerThread=nullptr;g_activationWakeEvent=nullptr;
    test("Live capture acquisition and detection-to-SendInput benchmark",[]{
        yami::DesktopCapture capture;yami::GpuMatcher gpu;RECT rect;GetWindowRect(g_testViewport,&rect);HMONITOR monitor=MonitorFromWindow(g_testViewport,MONITOR_DEFAULTTONEAREST);require(capture.Initialize(monitor),"capture benchmark init failed");size_t length;auto code=ShaderResource(length);require(gpu.Initialize(capture.Device(),code,length),"capture benchmark GPU init failed");
        yami::Viewport view;ClientViewport(g_testViewport,view);auto center=yami::ResolvePoint(view,.5,.5);int measured=0;
        for(int i=0;i<70;++i){scene(true);paintStarted=0;pump(8);yami::ComPtr<ID3D11Texture2D> frame;double captureMs;auto status=capture.Acquire(frame,captureMs,30);if(status==yami::DesktopCapture::Status::Timeout)continue;require(status==yami::DesktopCapture::Status::Frame,"capture benchmark acquisition failed");captureStats.add(captureMs);auto point=capture.TexturePoint(center,frame.Get());D3D11_TEXTURE2D_DESC d;frame->GetDesc(&d);require(gpu.Submit(frame.Get(),yami::RegionAround(point,12,d.Width,d.Height),point,12,sceneColor,0),"capture submit failed");capture.Release();bool found=false;yami::GpuMeasurement timing;auto completion=yami::GpuMatcher::Completion::Pending;double deadline=yami::Milliseconds()+3000;do{completion=gpu.Poll(found,timing);require(yami::Milliseconds()<deadline,"capture GPU timeout");if(completion==yami::GpuMatcher::Completion::Pending)SwitchToThread();}while(completion==yami::GpuMatcher::Completion::Pending);require(completion==yami::GpuMatcher::Completion::Ready&&found,"real desktop exact colour missed");
            require(GetForegroundWindow()==g_testViewport,"receiver lost focus during dispatch benchmark");
            yami::SingleTrigger gate;gate.SetActive(true);double detectionReady=yami::Milliseconds();
            bool sent=gate.Frame(gate.Token(),1,detectionReady,true,true,{},[](){return true;},[](){double start=yami::Milliseconds();bool accepted=SendOutputKeyFast();dispatchStats.add(yami::Milliseconds()-start);return accepted;});
            detectDispatchStats.add(yami::Milliseconds()-detectionReady);require(sent,"SendInput failed");++measured;pump(2);
        }require(measured>=30,"too few live capture samples");
    });
    // Best-effort release of the test-only F23 even if an assertion failed.
    if(GetAsyncKeyState(VK_F23)&0x8000){auto key=BuildOutputPair(VK_F23).up;SendInput(1,&key,sizeof(INPUT));pump(8,false);}
    DestroyWindow(g_testViewport);g_testViewport=nullptr;
}
int wmain(int argc,wchar_t** argv){
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    stateTests();gpuTests();
    try{liveTests();}catch(const std::exception& e){++failed;outcomes.push_back(std::string("FAIL live setup: ")+e.what());}
    std::filesystem::path directory=argc>1?argv[1]:L"test-results";std::filesystem::create_directories(directory);
    std::ofstream log(directory/"results.txt");for(auto& row:outcomes)log<<row<<"\n";log<<"Passed: "<<passed<<" Failed: "<<failed<<"\n";
    std::ofstream json(directory/"results.json");json<<"{\n\"passed\":"<<passed<<",\"failed\":"<<failed<<",\"gpu_adapter\":\""<<adapterName<<"\",\"gpu_classifications\":"<<gpuCases<<",\"positive_cases\":"<<positiveCases<<",\"negative_cases\":"<<negativeCases<<",\"synthetic_false_positives\":"<<falsePositive<<",\"synthetic_false_negatives\":"<<falseNegative<<",\"live_qualified_appearances\":33,\"live_received_events\":"<<liveEvents<<",\"live_duplicate_count\":"<<std::max(0,liveEvents-33)<<",\n\"gpu_submit_cpu\":"<<submitStats.json()<<",\n\"gpu_shader\":"<<gpuStats.json()<<",\n\"gpu_submit_to_cpu_available\":"<<readbackStats.json()<<",\n\"desktop_acquire_call\":"<<captureStats.json()<<",\n\"sendinput_call\":"<<dispatchStats.json()<<",\n\"detection_to_input_dispatch\":"<<detectDispatchStats.json()<<",\n\"controlled_paint_to_receiver\":"<<e2eStats.json()<<"\n}\n";
    std::cout<<"Passed: "<<passed<<" Failed: "<<failed<<"\n";return failed?1:0;
}
