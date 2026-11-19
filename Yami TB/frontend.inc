// Native Windows frontend. All coordinates below are 96-DPI design units.
using namespace Gdiplus;
namespace ui {
constexpr float W=520, H=630;
const Color bg(255,11,11,15), panel(255,18,18,22), border(255,34,34,40);
const Color ink(255,242,242,245), dim(255,180,180,188), purple(255,186,117,255);
HWND picker=nullptr, menu=nullptr, settings=nullptr, hex=nullptr,calibration=nullptr;
HINSTANCE instance{};
HICON icon{};
ULONG_PTR gdip{};
float scale=1, pickerScale=1, menuScale=1, settingsScale=1;
HFONT editFont{};
HBRUSH editBrush{};
UINT taskbarCreated=0;
bool trayAdded=false, closing=false, validHex=true, settingHex=false, eyedrop=false;
int drag=0, menuHover=-1, menuSelected=0;
bool suppressBindClick=false;
double hue=120, sat=1, val=1;
uint32_t draft=0x00FF00;
bool lastActive=false;
std::wstring saveLabel=L"Save";
constexpr int TOGGLE=101, COLOR=102, PRIMARY=103, HOLD=104, OUTPUT=105,
    STARTUP=106, SAVE=107, MINIMIZE=108, CLOSE=109, APPLY=110, EYE=111,
    LAUNCH=112, SETTINGS_CLOSE=113, CURSORLESS=114, CALIBRATE=115;
constexpr UINT TRAY=WM_APP+50;
struct Box { float x,y,w,h; };
bool inside(Box b,float x,float y) {return x>=b.x&&y>=b.y&&x<b.x+b.w&&y<b.y+b.h;}
GraphicsPath* path(Box b,float r) {
    auto p=new GraphicsPath; float d=r*2;
    p->AddArc(b.x,b.y,d,d,180,90); p->AddArc(b.x+b.w-d,b.y,d,d,270,90);
    p->AddArc(b.x+b.w-d,b.y+b.h-d,d,d,0,90); p->AddArc(b.x,b.y+b.h-d,d,d,90,90); p->CloseFigure(); return p;
}
void round(Graphics& g,Box b,float r,Color fill,Color stroke=Color(0,0,0,0),float width=1.4f) {
    auto p=path(b,r); SolidBrush brush(fill); g.FillPath(&brush,p);
    if(stroke.GetA()) {Pen pen(stroke,width);g.DrawPath(&pen,p);} delete p;
}
void text(Graphics& g,const std::wstring& s,Box b,float size=18,Color c=ink,bool bold=false,bool center=false) {
    FontFamily family(bold?L"Segoe UI Semibold":L"Segoe UI"); Font font(&family,size,FontStyleRegular,UnitPixel);
    StringFormat format;format.SetLineAlignment(StringAlignmentCenter);
    format.SetAlignment(center?StringAlignmentCenter:StringAlignmentNear);
    format.SetFormatFlags(StringFormatFlagsNoWrap);format.SetTrimming(StringTrimmingEllipsisCharacter);
    SolidBrush brush(c);g.DrawString(s.c_str(),-1,&font,RectF(b.x,b.y,b.w,b.h),&format,&brush);
}
void line(Graphics& g,float x,float y,float x2,float y2,Color c,float width=1.4f) {Pen p(c,width);p.SetStartCap(LineCapRound);p.SetEndCap(LineCapRound);g.DrawLine(&p,x,y,x2,y2);}
void logo(Graphics& g,float x,float y,float size) {
    line(g,x,y,x+size*.5f,y+size*.9f,purple,2.8f);
    line(g,x+size*.5f,y+size*.9f,x+size,y,purple,2.8f);
    line(g,x+size,y,x+size*.5f,y+size*.28f,purple,2.8f);
    line(g,x+size*.5f,y+size*.28f,x,y,purple,2.8f);
}
Color rgb(uint32_t c) {return Color(255,BYTE(c>>16),BYTE(c>>8),BYTE(c));}
uint32_t hsv(double h,double s,double v) {
    double c=v*s,x=c*(1-std::abs(std::fmod(h/60,2)-1)),m=v-c,r=0,g=0,b=0;
    if(h<60){r=c;g=x;}else if(h<120){r=x;g=c;}else if(h<180){g=c;b=x;}else if(h<240){g=x;b=c;}else if(h<300){r=x;b=c;}else{r=c;b=x;}
    return (uint32_t(std::round((r+m)*255))<<16)|(uint32_t(std::round((g+m)*255))<<8)|uint32_t(std::round((b+m)*255));
}
void toHsv(uint32_t color) {
    double r=double((color>>16)&255)/255,g=double((color>>8)&255)/255,b=double(color&255)/255;
    double mx=(std::max)({r,g,b}),mn=(std::min)({r,g,b}),d=mx-mn;
    val=mx;sat=mx==0?0:d/mx;
    if(d){if(mx==r)hue=60*std::fmod((g-b)/d,6);else if(mx==g)hue=60*((b-r)/d+2);else hue=60*((r-g)/d+4);if(hue<0)hue+=360;}
}
void invalidate() {if(g_hwnd)RedrawWindow(g_hwnd,nullptr,nullptr,RDW_INVALIDATE|RDW_ALLCHILDREN);}
void cancelCapture() {
    g_activationCapture=false;g_activationCaptureArmed=false;g_captureWaitingForRelease=false;
    g_primaryKeyDown=false;g_holdKeyDown=false;UpdateActivationActive();
    invalidate();
}
void pause() {g_engineSuspended=true;cancelCapture();}
void resumeConfiguration() {if(!picker&&!settings&&!calibration){g_engineSuspended=false;UpdateActivationActive();}}
float fitScale(HWND hwnd,float width,float height) {
    float dpi=float(GetDpiForWindow(hwnd))/96;
    MONITORINFO mi{sizeof(mi)};GetMonitorInfoW(MonitorFromWindow(hwnd,MONITOR_DEFAULTTONEAREST),&mi);
    return (std::min)({dpi,float(mi.rcWork.right-mi.rcWork.left-24)/width,float(mi.rcWork.bottom-mi.rcWork.top-24)/height});
}
void windowRegion(HWND hwnd,int w,int h,float s) {SetWindowRgn(hwnd,CreateRoundRectRgn(0,0,w+1,h+1,int(32*s),int(32*s)),TRUE);}
void control(HWND parent,int id,const wchar_t* title) {
    CreateWindowExW(0,L"BUTTON",title,WS_CHILD|WS_VISIBLE|WS_TABSTOP|BS_OWNERDRAW,0,0,1,1,parent,HMENU(INT_PTR(id)),instance,nullptr);
}
void place(HWND parent,int id,Box b,float s) {MoveWindow(GetDlgItem(parent,id),int(b.x*s),int(b.y*s),int(b.w*s),int(b.h*s),TRUE);}
void layoutMain() {
    place(g_hwnd,TOGGLE,{390,107,92,44},scale);place(g_hwnd,COLOR,{314,232,166,42},scale);
    place(g_hwnd,PRIMARY,{300,356,180,40},scale);place(g_hwnd,HOLD,{300,415,180,40},scale);place(g_hwnd,OUTPUT,{300,474,180,40},scale);
    place(g_hwnd,STARTUP,{26,568,290,40},scale);place(g_hwnd,SAVE,{358,566,138,44},scale);
    place(g_hwnd,MINIMIZE,{396,22,48,44},scale);place(g_hwnd,CLOSE,{456,22,48,44},scale);
    place(g_hwnd,CURSORLESS,{302,305,194,34},scale);
}
void sizeMain(const RECT* suggested=nullptr) {
    scale=fitScale(g_hwnd,W,H);int w=int(W*scale),h=int(H*scale);
    RECT old{};GetWindowRect(g_hwnd,&old);
    int x=suggested?suggested->left:old.left,y=suggested?suggested->top:old.top;
    MONITORINFO mi{sizeof(mi)};GetMonitorInfoW(MonitorFromRect(suggested?suggested:&old,MONITOR_DEFAULTTONEAREST),&mi);
    x=(std::clamp)(x,int(mi.rcWork.left),int(mi.rcWork.right-w));y=(std::clamp)(y,int(mi.rcWork.top),int(mi.rcWork.bottom-h));
    SetWindowPos(g_hwnd,nullptr,x,y,w,h,SWP_NOZORDER|SWP_NOACTIVATE);windowRegion(g_hwnd,w,h,scale);layoutMain();
}
void drawMain(Graphics& g) {
    g.Clear(bg);round(g,{.9f,.9f,W-1.8f,H-1.8f},16,bg,Color(255,61,61,67),1.8f);
    logo(g,37,30,32);text(g,L"Yami",{94,24,280,44},28,ink,true);
    round(g,{24,92,472,72},13,panel,border);
    bool active=g_toggleState.load();Pen ring(active?purple:Color(255,95,95,103),2);
    g.DrawEllipse(&ring,43.f,117.f,22.f,22.f);
    if(active){SolidBrush dot(purple);g.FillEllipse(&dot,48.f,122.f,12.f,12.f);}
    text(g,active?L"Enabled":L"Disabled",{80,99,250,58},21);
    text(g,L"Detection",{26,178,450,35},20,ink,true);
    round(g,{24,220,472,66},13,panel,border);text(g,L"Target Colour",{45,229,250,48},18);
    text(g,L"Keybinds",{26,305,450,35},20,ink,true);
    round(g,{24,346,472,176},13,panel,border);
    line(g,25,405,495,405,border);line(g,25,464,495,464,border);
    text(g,L"Toggle",{45,350,250,54},18);text(g,L"Hold",{45,409,250,54},18);text(g,L"Trigger",{45,468,250,54},18);
    line(g,24,548,496,548,border);
}
void drawCheck(Graphics& g,bool checked,float x,float y) {
    round(g,{x,y,24,24},4,Color(255,13,13,17),checked?purple:Color(255,91,91,100),1.8f);
    if(checked){line(g,x+5,y+12,x+10,y+17,purple,2);line(g,x+10,y+17,x+19,y+6,purple,2);}
}
void drawButton(const DRAWITEMSTRUCT& d,float s) {
    Graphics g(d.hDC);g.SetSmoothingMode(SmoothingModeAntiAlias);g.SetTextRenderingHint(TextRenderingHintClearTypeGridFit);g.ScaleTransform(s,s);
    float w=(d.rcItem.right-d.rcItem.left)/s,h=(d.rcItem.bottom-d.rcItem.top)/s;
    int id=d.CtlID;bool pressed=d.itemState&ODS_SELECTED,focus=d.itemState&ODS_FOCUS,disabled=d.itemState&ODS_DISABLED;
    g.Clear(id==TOGGLE||id==COLOR||id==PRIMARY||id==HOLD||id==OUTPUT?panel:bg);
    if(id==MINIMIZE){line(g,15,22,31,22,dim,1.8f);return;}
    if(id==CLOSE||id==SETTINGS_CLOSE){line(g,w/2-7,h/2-7,w/2+7,h/2+7,dim,1.8f);line(g,w/2+7,h/2-7,w/2-7,h/2+7,dim,1.8f);return;}
    if(id==TOGGLE){bool active=g_toggleState.load();round(g,{18,5,60,34},17,active?Color(255,141,77,204):Color(255,67,67,73));SolidBrush b(ink);g.FillEllipse(&b,active?49.f:21.f,8.f,28.f,28.f);return;}
    if(id==CURSORLESS){bool active=ReadDetectionOptions().cursorless;round(g,{6,9,30,16},8,active?Color(255,141,77,204):Color(255,67,67,73));SolidBrush b(ink);g.FillEllipse(&b,active?22.f:8.f,11.f,12.f,12.f);text(g,L"Cursorless TB",{47,0,w-47,h},16,dim);return;}
    if(id==COLOR){round(g,{2,3,34,36},7,rgb(picker?draft:g_targetColor.load()));round(g,{44,1,w-46,h-2},8,Color(255,13,13,17),border);text(g,HexColor(picker?draft:g_targetColor.load()),{45,0,w-47,h},17,ink,false,true);return;}
    if(id==STARTUP||id==LAUNCH){drawCheck(g,id==STARTUP?g_startWindows:g_launchMinimized,2,(h-24)/2);text(g,id==STARTUP?L"Start with Windows":L"Launch minimised to tray",{42,0,w-42,h},17,dim);return;}
    if(id==EYE){round(g,{0,0,w,h},5,Color(255,29,29,35));line(g,12,29,26,15,ink,2);line(g,21,14,28,21,ink,2);line(g,12,29,10,33,ink,2);return;}
    std::wstring title;
    wchar_t buffer[128]{};GetWindowTextW(d.hwndItem,buffer,ARRAYSIZE(buffer));title=buffer;
    bool binding=id==PRIMARY||id==HOLD||id==OUTPUT;
    if(binding){int vk=id==PRIMARY?g_activationVK.load():id==HOLD?g_holdActivationVK.load():g_outputVK.load();int target=id==PRIMARY?0:id==HOLD?1:2;
        title=g_activationCapture.load()&&int(g_activationCaptureTarget.load())==target?L"Press a key...":ActivationKeyName(vk);}
    Color edge=binding?Color(255,62,58,75):purple;
    round(g,{1,1,w-2,h-2},binding?20.f:9.f,pressed?Color(255,36,27,46):Color(255,22,20,29),focus?purple:edge,1.7f);
    text(g,title,{5,0,w-10,h},binding?17.f:18.f,disabled?dim:binding?ink:purple,false,true);
}
void updateBindings() {
    SetWindowTextW(GetDlgItem(g_hwnd,PRIMARY),ActivationKeyName(g_activationVK.load()));
    SetWindowTextW(GetDlgItem(g_hwnd,HOLD),ActivationKeyName(g_holdActivationVK.load()));
    SetWindowTextW(GetDlgItem(g_hwnd,OUTPUT),ActivationKeyName(g_outputVK.load()));
    invalidate();
}
void tray(bool add=false) {
    NOTIFYICONDATAW data{sizeof(data)};data.hWnd=g_hwnd;data.uID=1;
    data.uFlags=NIF_ICON|NIF_MESSAGE|NIF_TIP;data.uCallbackMessage=TRAY;data.hIcon=icon;
    wcscpy_s(data.szTip,g_toggleState.load()?L"Yami - Enabled (Hold to monitor)":L"Yami - Disabled");
    if(add){if(trayAdded)return;trayAdded=Shell_NotifyIconW(NIM_ADD,&data)!=FALSE;if(trayAdded){data.uVersion=NOTIFYICON_VERSION_4;Shell_NotifyIconW(NIM_SETVERSION,&data);}}
    else if(trayAdded)Shell_NotifyIconW(NIM_MODIFY,&data);
}
void removeTray() {if(trayAdded){NOTIFYICONDATAW d{sizeof(d)};d.hWnd=g_hwnd;d.uID=1;Shell_NotifyIconW(NIM_DELETE,&d);trayAdded=false;}}
void restore() {ShowWindow(g_hwnd,SW_RESTORE);SetForegroundWindow(g_hwnd);invalidate();}
void hide() {if(picker)DestroyWindow(picker);if(settings)DestroyWindow(settings);if(g_activationCapture.load())cancelCapture();if(!trayAdded)tray(true);if(trayAdded)ShowWindow(g_hwnd,SW_HIDE);}
void toggle() {if(g_activationCapture.load())return;g_toggleState=!g_toggleState.load();UpdateActivationActive();invalidate();tray();}
void saved() {SaveSettings();saveLabel=g_saveFailed?L"Retry save":L"Saved";SetWindowTextW(GetDlgItem(g_hwnd,SAVE),saveLabel.c_str());SetTimer(g_hwnd,3,1600,nullptr);invalidate();}
void commitColor() {if(!validHex)return;g_targetColor=draft;ResetEngineConfiguration();SaveSettings();DestroyWindow(picker);invalidate();}
void updateDraft(bool updateText=true) {
    draft=hsv(hue,sat,val);validHex=true;
    if(updateText&&hex){settingHex=true;SetWindowTextW(hex,HexColor(draft).c_str());settingHex=false;}
    if(picker){EnableWindow(GetDlgItem(picker,APPLY),TRUE);InvalidateRect(picker,nullptr,FALSE);}invalidate();
}
void drawPicker(Graphics& g) {
    g.Clear(bg);round(g,{1,1,250,374},16,Color(255,15,15,19),Color(255,61,61,67),1.8f);
    for(int i=0;i<360;++i){Pen p(rgb(hsv(i,1,1)),17);g.DrawArc(&p,30.f,26.f,192.f,192.f,float(-i),-1.5f);}
    double angle=-hue*3.141592653589793/180;float x=126+96*float(std::cos(angle)),y=122+96*float(std::sin(angle));
    SolidBrush knob(rgb(hsv(hue,1,1)));Pen white(ink,2);g.FillEllipse(&knob,x-11,y-11,22.f,22.f);g.DrawEllipse(&white,x-11,y-11,22.f,22.f);
    RectF square(74,70,104,104);LinearGradientBrush horizontal(square,Color(255,255,255,255),rgb(hsv(hue,1,1)),LinearGradientModeHorizontal);g.FillRectangle(&horizontal,square);
    LinearGradientBrush vertical(square,Color(0,0,0,0),Color(255,0,0,0),LinearGradientModeVertical);g.FillRectangle(&vertical,square);
    g.DrawEllipse(&white,float(74+sat*104-5),float(70+(1-val)*104-5),10.f,10.f);
    round(g,{16,248,220,44},9,Color(255,12,12,16),validHex?border:Color(255,237,99,118));
}
void pickerPoint(float x,float y,bool first) {
    double dx=x-126,dy=y-122,distance=std::sqrt(dx*dx+dy*dy);
    if(first)drag=distance>=84&&distance<=109?1:inside({74,70,104,104},x,y)?2:0;
    if(drag==1){hue=std::fmod(-std::atan2(dy,dx)*180/3.141592653589793+360,360);}
    if(drag==2){sat=(std::clamp)(double(x-74)/104,0.,1.);val=1-(std::clamp)(double(y-70)/104,0.,1.);}
    if(drag)updateDraft();
}
void paint(HWND hwnd,float s,void(*draw)(Graphics&)) {
    PAINTSTRUCT ps{};HDC dc=BeginPaint(hwnd,&ps);RECT r{};GetClientRect(hwnd,&r);
    HDC mem=CreateCompatibleDC(dc);HBITMAP bitmap=CreateCompatibleBitmap(dc,r.right,r.bottom);auto old=SelectObject(mem,bitmap);
    {Graphics g(mem);g.SetSmoothingMode(SmoothingModeAntiAlias);g.SetTextRenderingHint(TextRenderingHintClearTypeGridFit);g.ScaleTransform(s,s);draw(g);}
    BitBlt(dc,0,0,r.right,r.bottom,mem,0,0,SRCCOPY);SelectObject(mem,old);DeleteObject(bitmap);DeleteDC(mem);EndPaint(hwnd,&ps);
}
LRESULT CALLBACK hexProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l);
WNDPROC oldHexProc{};
LRESULT CALLBACK PickerProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l) {
    switch(msg){
    case WM_CREATE:{picker=hwnd;pickerScale=float(GetDpiForWindow(hwnd))/96;draft=g_targetColor.load();toHsv(draft);validHex=true;
        settingHex=true;
        hex=CreateWindowExW(0,L"EDIT",HexColor(draft).c_str(),WS_CHILD|WS_VISIBLE|WS_TABSTOP|ES_AUTOHSCROLL,0,0,1,1,hwnd,HMENU(200),instance,nullptr);
        settingHex=false;
        SendMessageW(hex,EM_SETLIMITTEXT,7,0);editFont=CreateFontW(-int(18*pickerScale),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Segoe UI");SendMessageW(hex,WM_SETFONT,WPARAM(editFont),TRUE);
        oldHexProc=reinterpret_cast<WNDPROC>(SetWindowLongPtrW(hex,GWLP_WNDPROC,LONG_PTR(hexProc)));
        control(hwnd,APPLY,L"Apply");control(hwnd,EYE,L"Pick screen colour");
        MoveWindow(hex,int(28*pickerScale),int(257*pickerScale),int(157*pickerScale),int(28*pickerScale),TRUE);
        place(hwnd,EYE,{189,249,46,42},pickerScale);place(hwnd,APPLY,{16,310,220,48},pickerScale);return 0;}
    case WM_ERASEBKGND:return 1;
    case WM_PAINT:paint(hwnd,pickerScale,drawPicker);return 0;
    case WM_DRAWITEM:drawButton(*reinterpret_cast<DRAWITEMSTRUCT*>(l),pickerScale);return TRUE;
    case WM_CTLCOLOREDIT:SetTextColor(HDC(w),RGB(242,242,245));SetBkColor(HDC(w),RGB(12,12,16));return LRESULT(editBrush);
    case WM_COMMAND:
        if(LOWORD(w)==IDCANCEL){DestroyWindow(hwnd);return 0;}
        if(LOWORD(w)==IDOK){commitColor();return 0;}
        if(LOWORD(w)==APPLY){commitColor();return 0;}
        if(LOWORD(w)==EYE){eyedrop=true;SetCapture(hwnd);SetCursor(LoadCursorW(nullptr,IDC_CROSS));return 0;}
        if(LOWORD(w)==200&&HIWORD(w)==EN_CHANGE&&!settingHex){wchar_t buffer[32]{};GetWindowTextW(hex,buffer,ARRAYSIZE(buffer));uint32_t c=0;validHex=ParseHex(buffer,c);if(validHex){draft=c;toHsv(c);}EnableWindow(GetDlgItem(hwnd,APPLY),validHex);InvalidateRect(hwnd,nullptr,FALSE);invalidate();return 0;}break;
    case WM_LBUTTONDOWN:pickerPoint(GET_X_LPARAM(l)/pickerScale,GET_Y_LPARAM(l)/pickerScale,true);if(drag)SetCapture(hwnd);return 0;
    case WM_MOUSEMOVE:
        if(eyedrop){POINT p{};GetCursorPos(&p);HDC dc=GetDC(nullptr);COLORREF c=GetPixel(dc,p.x,p.y);ReleaseDC(nullptr,dc);if(c!=CLR_INVALID){draft=(uint32_t(GetRValue(c))<<16)|(uint32_t(GetGValue(c))<<8)|GetBValue(c);toHsv(draft);updateDraft();}SetCursor(LoadCursorW(nullptr,IDC_CROSS));}
        else if(drag)pickerPoint(GET_X_LPARAM(l)/pickerScale,GET_Y_LPARAM(l)/pickerScale,false);return 0;
    case WM_LBUTTONUP:if(eyedrop||drag){eyedrop=false;drag=0;ReleaseCapture();}return 0;
    case WM_KEYDOWN:if(w==VK_ESCAPE){DestroyWindow(hwnd);return 0;}if(w==VK_RETURN){commitColor();return 0;}break;
    case WM_ACTIVATE:if(LOWORD(w)==WA_INACTIVE&&!eyedrop){DestroyWindow(hwnd);return 0;}break;
    case WM_DPICHANGED:DestroyWindow(hwnd);return 0;
    case WM_DESTROY:picker=nullptr;hex=nullptr;drag=0;eyedrop=false;if(editFont){DeleteObject(editFont);editFont=nullptr;}resumeConfiguration();invalidate();return 0;
    }return DefWindowProcW(hwnd,msg,w,l);
}
LRESULT CALLBACK hexProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){if(msg==WM_KEYDOWN&&w==VK_RETURN){commitColor();return 0;}if(msg==WM_KEYDOWN&&w==VK_ESCAPE){DestroyWindow(picker);return 0;}return CallWindowProcW(oldHexProc,hwnd,msg,w,l);}
void showPicker(){if(picker){SetForegroundWindow(picker);return;}pause();RECT r{};GetWindowRect(g_hwnd,&r);pickerScale=float(GetDpiForWindow(g_hwnd))/96;
    MONITORINFO mi{sizeof(mi)};GetMonitorInfoW(MonitorFromWindow(g_hwnd,MONITOR_DEFAULTTONEAREST),&mi);
    int width=int(252*pickerScale),height=int(376*pickerScale),x=r.right+int(20*pickerScale),y=r.top+int(148*scale);
    if(x+width>mi.rcWork.right)x=r.left-width-int(12*pickerScale);
    x=(std::clamp)(x,int(mi.rcWork.left),int(mi.rcWork.right-width));y=(std::clamp)(y,int(mi.rcWork.top),int(mi.rcWork.bottom-height));
    HWND p=CreateWindowExW(WS_EX_TOOLWINDOW,L"YamiColorPicker",L"Colour",WS_POPUP,x,y,width,height,g_hwnd,nullptr,instance,nullptr);
    windowRegion(p,width,height,pickerScale);ShowWindow(p,SW_SHOW);SetForegroundWindow(p);SetFocus(hex);}

std::array<bool,5> numericValid{true,true,true,true,true};
HFONT settingsFont{};
constexpr float settingsWidth=400,settingsHeight=462;
void startCalibration();
void drawSettings(Graphics& g){
    g.Clear(bg);round(g,{1,1,398,460},15,bg,Color(255,61,61,67),1.8f);
    text(g,L"Settings",{24,16,300,40},23,ink,true);
    const wchar_t* labels[]={L"Tolerance (RGB)",L"Region radius (px)",L"Clear debounce (ms)",L"Clear frames",L"Minimum interval (ms)"};
    for(int i=0;i<5;++i){float y=174+38.f*i;text(g,labels[i],{24,y,250,30},16,dim);round(g,{280,y,96,30},6,Color(255,12,12,16),numericValid[i]?border:Color(255,237,99,118));}
}
void fillNumeric(HWND hwnd){
    auto o=ReadDetectionOptions();double values[]={double(o.tolerance),double(o.cursorless?o.fixedRadius:o.cursorRadius),o.clearMs,double(o.clearFrames),o.minimumIntervalMs};
    for(int i=0;i<5;++i){wchar_t value[32];swprintf_s(value,L"%.4g",values[i]);SetWindowTextW(GetDlgItem(hwnd,300+i),value);numericValid[i]=true;}
}
LRESULT CALLBACK SettingsProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){switch(msg){
    case WM_CREATE:{settings=hwnd;settingsScale=float(GetDpiForWindow(hwnd))/96;
        control(hwnd,LAUNCH,L"Launch minimised to tray");control(hwnd,STARTUP,L"Start with Windows");control(hwnd,SETTINGS_CLOSE,L"Close settings");control(hwnd,CALIBRATE,ReadDetectionOptions().calibrated?L"Recalibrate detection point":L"Calibrate detection point");
        place(hwnd,LAUNCH,{24,68,352,36},settingsScale);place(hwnd,STARTUP,{24,110,352,36},settingsScale);place(hwnd,SETTINGS_CLOSE,{340,16,36,40},settingsScale);place(hwnd,CALIBRATE,{24,392,352,44},settingsScale);
        settingsFont=CreateFontW(-int(16*settingsScale),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Segoe UI");
        for(int i=0;i<5;++i){HWND edit=CreateWindowExW(0,L"EDIT",L"",WS_CHILD|WS_VISIBLE|WS_TABSTOP|ES_CENTER|ES_AUTOHSCROLL,int(286*settingsScale),int((178+38*i)*settingsScale),int(84*settingsScale),int(24*settingsScale),hwnd,HMENU(INT_PTR(300+i)),instance,nullptr);SendMessageW(edit,WM_SETFONT,WPARAM(settingsFont),TRUE);SendMessageW(edit,EM_SETLIMITTEXT,8,0);}
        fillNumeric(hwnd);return 0;}
    case WM_ERASEBKGND:return 1;case WM_PAINT:paint(hwnd,settingsScale,drawSettings);return 0;
    case WM_CTLCOLOREDIT:SetTextColor(HDC(w),RGB(242,242,245));SetBkColor(HDC(w),RGB(12,12,16));return LRESULT(editBrush);
    case WM_DRAWITEM:drawButton(*reinterpret_cast<DRAWITEMSTRUCT*>(l),settingsScale);return TRUE;
    case WM_COMMAND:{int id=LOWORD(w);
        if(id==IDCANCEL){DestroyWindow(hwnd);return 0;}
        if(id>=300&&id<=304&&HIWORD(w)==EN_KILLFOCUS){int index=id-300;wchar_t value[32]{};GetWindowTextW(GetDlgItem(hwnd,id),value,ARRAYSIZE(value));wchar_t* end=nullptr;double n=wcstod(value,&end);
            const double minimum[]={0,1,0,2,0},maximum[]={16,128,100,16,1000};
            bool integer=index==0||index==1||index==3;
            numericValid[index]=end!=value&&*end==0&&std::isfinite(n)&&n>=minimum[index]&&n<=maximum[index]&&(!integer||std::floor(n)==n);
            if(numericValid[index]){EditDetectionOptions([&](DetectionOptions& o){if(index==0)o.tolerance=unsigned(n);else if(index==1){if(o.cursorless)o.fixedRadius=unsigned(n);else o.cursorRadius=unsigned(n);}else if(index==2)o.clearMs=n;else if(index==3)o.clearFrames=unsigned(n);else o.minimumIntervalMs=n;});SaveSettings();}
            InvalidateRect(hwnd,nullptr,FALSE);return 0;}
        if(id==SETTINGS_CLOSE)DestroyWindow(hwnd);
        else if(id==LAUNCH){g_launchMinimized=!g_launchMinimized;SaveSettings();InvalidateRect(hwnd,nullptr,FALSE);}
        else if(id==STARTUP){if(SetStartup(!g_startWindows))g_startWindows=!g_startWindows;else MessageBoxW(hwnd,L"Windows startup could not be updated.",L"Yami",MB_OK|MB_ICONERROR);InvalidateRect(hwnd,nullptr,FALSE);invalidate();}
        else if(id==CALIBRATE)startCalibration();return 0;}
    case WM_KEYDOWN:if(w==VK_ESCAPE)DestroyWindow(hwnd);return 0;
    case WM_DPICHANGED:DestroyWindow(hwnd);return 0;
    case WM_DESTROY:settings=nullptr;if(settingsFont){DeleteObject(settingsFont);settingsFont=nullptr;}resumeConfiguration();return 0;
    }return DefWindowProcW(hwnd,msg,w,l);
}
void showSettings(){restore();pause();if(settings){SetForegroundWindow(settings);return;}
    RECT r{};GetWindowRect(g_hwnd,&r);float s=fitScale(g_hwnd,settingsWidth,settingsHeight);
    MONITORINFO mi{sizeof(mi)};GetMonitorInfoW(MonitorFromWindow(g_hwnd,MONITOR_DEFAULTTONEAREST),&mi);int width=int(settingsWidth*s),height=int(settingsHeight*s);
    int x=std::clamp(int(r.left+60*scale),int(mi.rcWork.left),int(mi.rcWork.right-width)),y=std::clamp(int(r.top+90*scale),int(mi.rcWork.top),int(mi.rcWork.bottom-height));
    HWND p=CreateWindowExW(WS_EX_TOOLWINDOW,L"YamiSettings",L"Yami Settings",WS_POPUP,x,y,width,height,g_hwnd,nullptr,instance,nullptr);settingsScale=s;
    // Normally scale is the monitor DPI. On a very small work area, controls
    // are compressed with the same fit ratio as the main configuration window.
    if(settingsScale!=float(GetDpiForWindow(p))/96){place(p,LAUNCH,{24,68,352,36},s);place(p,STARTUP,{24,110,352,36},s);place(p,SETTINGS_CLOSE,{340,16,36,40},s);place(p,CALIBRATE,{24,392,352,44},s);for(int i=0;i<5;++i)MoveWindow(GetDlgItem(p,300+i),int(286*s),int((178+38*i)*s),int(84*s),int(24*s),TRUE);}
    windowRegion(p,width,height,s);ShowWindow(p,SW_SHOW);SetForegroundWindow(p);
}
LRESULT CALLBACK CalibrationProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){switch(msg){
    case WM_CREATE:calibration=hwnd;SetLayeredWindowAttributes(hwnd,RGB(1,1,1),0,LWA_COLORKEY);return 0;
    case WM_ERASEBKGND:return 1;
    case WM_PAINT:{PAINTSTRUCT ps{};HDC dc=BeginPaint(hwnd,&ps);RECT rect{};GetClientRect(hwnd,&rect);HBRUSH background=CreateSolidBrush(RGB(1,1,1));FillRect(dc,&rect,background);DeleteObject(background);
        POINT p{};GetCursorPos(&p);ScreenToClient(hwnd,&p);HPEN pen=CreatePen(PS_SOLID,2,RGB(186,117,255));auto old=SelectObject(dc,pen);auto oldBrush=SelectObject(dc,GetStockObject(NULL_BRUSH));Ellipse(dc,p.x-8,p.y-8,p.x+9,p.y+9);MoveToEx(dc,p.x-16,p.y,nullptr);LineTo(dc,p.x+17,p.y);MoveToEx(dc,p.x,p.y-16,nullptr);LineTo(dc,p.x,p.y+17);SelectObject(dc,oldBrush);SelectObject(dc,old);DeleteObject(pen);SetTextColor(dc,RGB(242,242,245));SetBkMode(dc,TRANSPARENT);const wchar_t* help=L"Select a point inside the Roblox viewport. Escape cancels.";TextOutW(dc,24,24,help,int(wcslen(help)));EndPaint(hwnd,&ps);return 0;}
    case WM_MOUSEMOVE:InvalidateRect(hwnd,nullptr,FALSE);return 0;
    case WM_LBUTTONUP:{POINT p{};GetCursorPos(&p);ReleaseCapture();ShowWindow(hwnd,SW_HIDE);HWND target=GetAncestor(WindowFromPoint(p),GA_ROOT);yami::Viewport viewport;
        bool valid=IsRobloxWindow(target)&&ClientViewport(target,viewport)&&p.x>=viewport.x&&p.x<viewport.x+viewport.width&&p.y>=viewport.y&&p.y<viewport.y+viewport.height;
        if(valid){g_viewportHint=uintptr_t(target);EditDetectionOptions([&](DetectionOptions& o){o.normalizedX=double(p.x-viewport.x)/viewport.width;o.normalizedY=double(p.y-viewport.y)/viewport.height;o.calibrated=true;});SaveSettings();if(settings)SetWindowTextW(GetDlgItem(settings,CALIBRATE),L"Recalibrate detection point");}
        DestroyWindow(hwnd);if(settings)SetForegroundWindow(settings);if(!valid)MessageBoxW(settings,L"Calibration needs a point inside a visible Roblox client area. Use Calibrate to try again.",L"Yami",MB_OK|MB_ICONINFORMATION);return 0;}
    case WM_KEYDOWN:if(w==VK_ESCAPE){ReleaseCapture();DestroyWindow(hwnd);if(settings)SetForegroundWindow(settings);}return 0;
    case WM_COMMAND:if(LOWORD(w)==IDCANCEL){ShowWindow(hwnd,SW_HIDE);ReleaseCapture();DestroyWindow(hwnd);if(settings)SetForegroundWindow(settings);}return 0;
    case WM_CAPTURECHANGED:if(calibration==hwnd&&HWND(l)!=hwnd&&IsWindowVisible(hwnd))DestroyWindow(hwnd);return 0;
    case WM_DESTROY:calibration=nullptr;resumeConfiguration();return 0;
    }return DefWindowProcW(hwnd,msg,w,l);
}
void startCalibration(){if(calibration)return;pause();int x=GetSystemMetrics(SM_XVIRTUALSCREEN),y=GetSystemMetrics(SM_YVIRTUALSCREEN),w=GetSystemMetrics(SM_CXVIRTUALSCREEN),h=GetSystemMetrics(SM_CYVIRTUALSCREEN);
    HWND overlay=CreateWindowExW(WS_EX_LAYERED|WS_EX_TOPMOST|WS_EX_TOOLWINDOW,L"YamiCalibration",L"Yami Calibration",WS_POPUP,x,y,w,h,g_hwnd,nullptr,instance,nullptr);ShowWindow(overlay,SW_SHOW);SetForegroundWindow(overlay);SetFocus(overlay);SetCapture(overlay);
}
void drawMenu(Graphics& g){g.Clear(bg);round(g,{1,1,222,214},14,Color(255,15,15,19),Color(255,61,61,67),1.8f);
    const wchar_t* labels[]={L"Open",L"Enable",L"Settings",L"Exit"};for(int i=0;i<4;++i){float y=8+i*50.f;if(i==menuHover)round(g,{8,y,208,48},8,Color(255,31,31,37));text(g,labels[i],{80,y,130,48},20);}
    line(g,35,31,42,38,ink,1.8f);line(g,42,38,54,23,ink,1.8f);
    round(g,{33,75,24,16},8,Color(255,15,15,19),dim,1.6f);SolidBrush b(g_toggleState.load()?purple:dim);g.FillEllipse(&b,g_toggleState.load()?45.f:35.f,78.f,10.f,10.f);
    Pen gear(dim,1.7f);g.DrawEllipse(&gear,35.f,123.f,20.f,20.f);g.DrawEllipse(&gear,41.f,129.f,8.f,8.f);
    for(int i=0;i<8;++i){double a=i*3.141592653589793/4;line(g,45+10*float(cos(a)),133+10*float(sin(a)),45+13*float(cos(a)),133+13*float(sin(a)),dim,2);}
    g.DrawArc(&gear,35.f,173.f,20.f,20.f,-55.f,290.f);line(g,45,171,45,182,ink,1.8f);
}
void menuAction(int item){if(menu)DestroyWindow(menu);if(item==0)restore();else if(item==1)toggle();else if(item==2)showSettings();else if(item==3)PostMessageW(g_hwnd,WM_CLOSE,0,0);}
LRESULT CALLBACK MenuProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){switch(msg){
    case WM_CREATE:menu=hwnd;menuScale=float(GetDpiForWindow(hwnd))/96;menuHover=0;return 0;
    case WM_ERASEBKGND:return 1;case WM_PAINT:paint(hwnd,menuScale,drawMenu);return 0;
    case WM_MOUSEMOVE:{int row=int((GET_Y_LPARAM(l)/menuScale-8)/50);if(row>=0&&row<4&&menuHover!=row){menuHover=row;InvalidateRect(hwnd,nullptr,FALSE);}return 0;}
    case WM_LBUTTONUP:{float y=GET_Y_LPARAM(l)/menuScale;if(y>=8&&y<208)menuAction(int((y-8)/50));return 0;}
    case WM_KEYDOWN:if(w==VK_ESCAPE)DestroyWindow(hwnd);else if(w==VK_DOWN||w==VK_UP){menuHover=(menuHover+(w==VK_DOWN?1:3))%4;InvalidateRect(hwnd,nullptr,FALSE);}else if(w==VK_RETURN||w==VK_SPACE)menuAction(menuHover);return 0;
    case WM_ACTIVATE:if(LOWORD(w)==WA_INACTIVE)DestroyWindow(hwnd);return 0;case WM_DESTROY:menu=nullptr;return 0;
    }return DefWindowProcW(hwnd,msg,w,l);}
void showMenu(POINT p){if(menu){DestroyWindow(menu);return;}MONITORINFO mi{sizeof(mi)};auto monitor=MonitorFromPoint(p,MONITOR_DEFAULTTONEAREST);GetMonitorInfoW(monitor,&mi);
    menuScale=float(GetDpiForWindow(g_hwnd))/96;int width=int(224*menuScale),height=int(216*menuScale);int x=(std::clamp)(int(p.x-width/2),int(mi.rcWork.left),int(mi.rcWork.right-width)),y=(std::clamp)(int(p.y-height),int(mi.rcWork.top),int(mi.rcWork.bottom-height));
    HWND m=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_TOPMOST,L"YamiTrayMenu",L"Yami",WS_POPUP,x,y,width,height,g_hwnd,nullptr,instance,nullptr);windowRegion(m,width,height,menuScale);SetForegroundWindow(m);ShowWindow(m,SW_SHOW);SetFocus(m);}
HICON makeIcon(){HDC dc=GetDC(nullptr);HDC mem=CreateCompatibleDC(dc);BITMAPV5HEADER header{};header.bV5Size=sizeof(header);header.bV5Width=32;header.bV5Height=-32;header.bV5Planes=1;header.bV5BitCount=32;header.bV5Compression=BI_BITFIELDS;header.bV5RedMask=0x00FF0000;header.bV5GreenMask=0x0000FF00;header.bV5BlueMask=0x000000FF;header.bV5AlphaMask=0xFF000000;void* bits{};HBITMAP color=CreateDIBSection(dc,reinterpret_cast<BITMAPINFO*>(&header),DIB_RGB_COLORS,&bits,nullptr,0);auto old=SelectObject(mem,color);{Graphics g(mem);g.Clear(Color(0,0,0,0));g.SetSmoothingMode(SmoothingModeAntiAlias);logo(g,4,5,24);}HBITMAP mask=CreateBitmap(32,32,1,1,nullptr);ICONINFO info{TRUE,0,0,mask,color};HICON result=CreateIconIndirect(&info);SelectObject(mem,old);DeleteObject(mask);DeleteObject(color);DeleteDC(mem);ReleaseDC(nullptr,dc);return result;}
void startShutdown(){if(closing)return;closing=true;pause();SaveSettings();if(picker)DestroyWindow(picker);if(settings)DestroyWindow(settings);if(calibration)DestroyWindow(calibration);if(menu)DestroyWindow(menu);removeTray();ShowWindow(g_hwnd,SW_HIDE);g_running=false;if(g_activationWakeEvent)SetEvent(g_activationWakeEvent);SetTimer(g_hwnd,2,25,nullptr);}
LRESULT CALLBACK MainProc(HWND hwnd,UINT msg,WPARAM w,LPARAM l){if(taskbarCreated&&msg==taskbarCreated){trayAdded=false;tray(true);return 0;}switch(msg){
    case WM_CREATE:g_hwnd=hwnd;control(hwnd,TOGGLE,L"Enable");control(hwnd,COLOR,L"Target colour");control(hwnd,PRIMARY,L"Not Set");control(hwnd,HOLD,L"Not Set");control(hwnd,OUTPUT,L"Not Set");control(hwnd,STARTUP,L"Start with Windows");control(hwnd,SAVE,L"Save");control(hwnd,MINIMIZE,L"Minimise");control(hwnd,CLOSE,L"Exit");control(hwnd,CURSORLESS,L"Cursorless TB");if(!RegisterRawActivationInput(hwnd)){MessageBoxW(hwnd,L"Keyboard and mouse input registration failed.",L"Yami",MB_OK|MB_ICONERROR);return -1;}return 0;
    case WM_ERASEBKGND:return 1;case WM_PAINT:paint(hwnd,scale,drawMain);return 0;
    case WM_DRAWITEM:drawButton(*reinterpret_cast<DRAWITEMSTRUCT*>(l),scale);return TRUE;
    case WM_INPUT:HandleRawActivationInput(reinterpret_cast<HRAWINPUT>(l));return DefWindowProcW(hwnd,msg,w,l);
    case WM_APP+41:invalidate();tray();return 0;
    case WM_APP+42:SaveSettings();updateBindings();return 0;
    case WM_APP+43:CompleteBindCapture(int(w));return 0;
    case WM_COMMAND:{int id=LOWORD(w);if(suppressBindClick&&(id==PRIMARY||id==HOLD||id==OUTPUT)){suppressBindClick=false;return 0;}if(g_activationCapture.load()&&id!=PRIMARY&&id!=HOLD&&id!=OUTPUT)cancelCapture();
        if(id==CURSORLESS){EditDetectionOptions([](DetectionOptions& o){o.cursorless=!o.cursorless;});SaveSettings();invalidate();if(settings)fillNumeric(settings);}else if(id==TOGGLE)toggle();else if(id==COLOR)showPicker();else if(id==PRIMARY||id==HOLD||id==OUTPUT){BeginActivationCapture(id==PRIMARY?ActivationCaptureTarget::Primary:id==HOLD?ActivationCaptureTarget::Hold:ActivationCaptureTarget::Output);g_captureWaitingForRelease=false;g_activationCaptureArmed=true;SetWindowTextW(GetDlgItem(hwnd,id),L"Press a key...");}
        else if(id==SAVE)saved();else if(id==MINIMIZE)hide();else if(id==CLOSE)startShutdown();else if(id==STARTUP){if(SetStartup(!g_startWindows))g_startWindows=!g_startWindows;else MessageBoxW(hwnd,L"Windows startup could not be updated.",L"Yami",MB_OK|MB_ICONERROR);SaveSettings();invalidate();}return 0;}
    case WM_LBUTTONDOWN:if(g_activationCapture.load())cancelCapture();if(GET_Y_LPARAM(l)<int(88*scale)){ReleaseCapture();SendMessageW(hwnd,WM_NCLBUTTONDOWN,HTCAPTION,l);}return 0;
    case WM_CONTEXTMENU:{POINT point{};GetCursorPos(&point);showMenu(point);return 0;}
    case WM_KEYDOWN:if(w==VK_OEM_COMMA&&(GetKeyState(VK_CONTROL)&0x8000)&&!g_activationCapture.load()){showSettings();return 0;}break;
    case WM_ACTIVATE:if(LOWORD(w)==WA_INACTIVE&&g_activationCapture.load())cancelCapture();return 0;
    case WM_SYSCOMMAND:if((w&0xFFF0)==SC_MINIMIZE){hide();return 0;}break;
    case WM_SIZE:if(w==SIZE_MINIMIZED)hide();return 0;
    case WM_DPICHANGED:if(picker)DestroyWindow(picker);if(settings)DestroyWindow(settings);sizeMain(reinterpret_cast<RECT*>(l));return 0;
    case TRAY:{UINT event=LOWORD(l);if(event==WM_LBUTTONDBLCLK||event==NIN_SELECT||event==NIN_KEYSELECT)restore();else if(event==WM_CONTEXTMENU||event==WM_RBUTTONUP){POINT p{};GetCursorPos(&p);showMenu(p);}return 0;}
    case WM_TIMER:if(w==1){bool active=g_toggleState.load();if(active!=lastActive){lastActive=active;invalidate();tray();}}else if(w==2){if(g_activationWakeEvent)SetEvent(g_activationWakeEvent);bool done=(!g_scannerThread||WaitForSingleObject(g_scannerThread,0)==WAIT_OBJECT_0)&&(!g_controllerThread||WaitForSingleObject(g_controllerThread,0)==WAIT_OBJECT_0);if(done)DestroyWindow(hwnd);}else if(w==3){KillTimer(hwnd,3);SetWindowTextW(GetDlgItem(hwnd,SAVE),L"Save");}return 0;
    case WM_CLOSE:startShutdown();return 0;
    case WM_DESTROY:removeTray();KillTimer(hwnd,1);KillTimer(hwnd,2);if(g_scannerThread){CloseHandle(g_scannerThread);g_scannerThread=nullptr;}if(g_controllerThread){CloseHandle(g_controllerThread);g_controllerThread=nullptr;}if(g_activationWakeEvent){CloseHandle(g_activationWakeEvent);g_activationWakeEvent=nullptr;}PostQuitMessage(0);return 0;
    }return DefWindowProcW(hwnd,msg,w,l);}
} // namespace ui

// Capture is configuration only; it never changes the detector or simulation.
bool ConsumeBindingInput(int vk,bool down,bool up)
{
    if(g_captureBlockVK.load()==vk){if(up)g_captureBlockVK=0;return true;}
    if(g_activationCapture.load()){
        if(!g_activationCaptureArmed.load())return true;
        if(down){
            if(vk==VK_LBUTTON){POINT p{};GetCursorPos(&p);HWND hit=WindowFromPoint(p);HWND activeButton=GetDlgItem(g_hwnd,int(g_activationCaptureTarget.load())==0?ui::PRIMARY:int(g_activationCaptureTarget.load())==1?ui::HOLD:ui::OUTPUT);
                if(hit!=activeButton){ui::cancelCapture();return true;}ui::suppressBindClick=true;}
            CompleteBindCapture(vk);
        }return true;
    }
    if(ui::picker||ui::settings||ui::menu||ui::calibration){if(up)HandleActivationEvent(vk,false,true);return true;}
    if(vk==VK_LBUTTON||vk==VK_RBUTTON||vk==VK_MBUTTON||vk==VK_XBUTTON1||vk==VK_XBUTTON2){POINT p{};GetCursorPos(&p);HWND hit=WindowFromPoint(p);if(hit==g_hwnd||IsChild(g_hwnd,hit)){if(up){HandleActivationEvent(vk,false,true);}return true;}}
    return false;
}
int WINAPI WinMain(HINSTANCE instance,HINSTANCE,LPSTR,int)
{
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    HANDLE single=CreateMutexW(nullptr,FALSE,L"Local\\Yami.Desktop.SingleInstance");
    if(GetLastError()==ERROR_ALREADY_EXISTS){HWND existing=FindWindowW(L"YamiMain",nullptr);if(existing){ShowWindow(existing,SW_RESTORE);SetForegroundWindow(existing);}if(single)CloseHandle(single);return 0;}
    ui::instance=instance;GdiplusStartupInput startup;
    if(GdiplusStartup(&ui::gdip,&startup,nullptr)!=Ok){if(single)CloseHandle(single);return 1;}
    ui::editBrush=CreateSolidBrush(RGB(12,12,16));ui::icon=HICON(LoadImageW(instance,MAKEINTRESOURCEW(1),IMAGE_ICON,32,32,LR_DEFAULTCOLOR));if(!ui::icon)ui::icon=ui::makeIcon();
    struct Class{const wchar_t* name;WNDPROC proc;};
    for(auto c:{Class{L"YamiMain",ui::MainProc},Class{L"YamiColorPicker",ui::PickerProc},Class{L"YamiTrayMenu",ui::MenuProc},Class{L"YamiSettings",ui::SettingsProc},Class{L"YamiCalibration",ui::CalibrationProc}}){WNDCLASSEXW wc{sizeof(wc)};wc.hInstance=instance;wc.lpfnWndProc=c.proc;wc.lpszClassName=c.name;wc.hCursor=LoadCursorW(nullptr,IDC_ARROW);wc.hIcon=ui::icon;wc.hIconSm=ui::icon;RegisterClassExW(&wc);}
    LoadSettings();ui::taskbarCreated=RegisterWindowMessageW(L"TaskbarCreated");
    HWND hwnd=CreateWindowExW(WS_EX_APPWINDOW,L"YamiMain",L"Yami",WS_POPUP|WS_SYSMENU|WS_MINIMIZEBOX|WS_CLIPCHILDREN,100,100,520,630,nullptr,nullptr,instance,nullptr);
    if(!hwnd){DeleteObject(ui::editBrush);DestroyIcon(ui::icon);GdiplusShutdown(ui::gdip);if(single)CloseHandle(single);return 1;}
    ui::sizeMain();RECT r{};GetWindowRect(hwnd,&r);MONITORINFO mi{sizeof(mi)};GetMonitorInfoW(MonitorFromWindow(hwnd,MONITOR_DEFAULTTONEAREST),&mi);SetWindowPos(hwnd,nullptr,mi.rcWork.left+(mi.rcWork.right-mi.rcWork.left-(r.right-r.left))/2,mi.rcWork.top+(mi.rcWork.bottom-mi.rcWork.top-(r.bottom-r.top))/2,0,0,SWP_NOSIZE|SWP_NOZORDER);
    ui::updateBindings();ui::tray(true);
    g_activationWakeEvent=CreateEventW(nullptr,TRUE,FALSE,nullptr);
    g_scannerThread=CreateThread(nullptr,0,[](LPVOID)->DWORD{ScannerThread();return 0;},nullptr,0,nullptr);
    g_controllerThread=CreateThread(nullptr,0,[](LPVOID)->DWORD{ControllerInputThread();return 0;},nullptr,0,nullptr);
    if(!g_activationWakeEvent||!g_scannerThread||!g_controllerThread){MessageBoxW(hwnd,L"A background worker could not start. Yami will close.",L"Yami",MB_OK|MB_ICONERROR);ui::startShutdown();}
    SetTimer(hwnd,1,100,nullptr);
    bool trayCommand=wcsstr(GetCommandLineW(),L"--tray")!=nullptr;
    if(!ui::closing&&(!(g_launchMinimized||trayCommand)||!ui::trayAdded))ShowWindow(hwnd,SW_SHOW);
    MSG msg{};while(GetMessageW(&msg,nullptr,0,0)>0){HWND dialog=ui::calibration?ui::calibration:ui::picker?ui::picker:ui::settings?ui::settings:hwnd;
        if(!g_activationCapture.load()&&IsDialogMessageW(dialog,&msg))continue;TranslateMessage(&msg);DispatchMessageW(&msg);}
    DestroyIcon(ui::icon);DeleteObject(ui::editBrush);GdiplusShutdown(ui::gdip);if(single)CloseHandle(single);return int(msg.wParam);
}
