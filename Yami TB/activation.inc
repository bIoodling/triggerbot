#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include <windows.h>
#include <windowsx.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <d3dcompiler.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <chrono>
#include <cstdint>
#include <sstream>
#include <string>
#include <thread>
#include <vector>
#include <cwchar>
#include <cstring>
#include <emmintrin.h>
#include <intrin.h>
#include <Xinput.h>

static inline unsigned FindFirstSetBit(unsigned mask)
{
    unsigned long index = 0;
    _BitScanForward(&index, mask);
    return static_cast<unsigned>(index);
}

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#pragma comment(lib, "d3dcompiler.lib")
#pragma comment(lib, "xinput9_1_0.lib")

// ============================================================
// CONFIGURATION
// ============================================================

std::atomic<uint32_t> g_targetColor{ 0x00FF00u }; // RGB packed as 0xRRGGBB

// ============================================================
// GLOBAL STATE
// ============================================================

std::atomic<bool> g_running{ true };
std::atomic<bool> g_scanning{ false };
std::atomic<bool> g_detectorReady{ false };
std::atomic<bool> g_greenDetected{ false };


// Shared activation state.
// The primary bind is always TOGGLE. The dedicated hold bind is always HOLD. The output key is independently configurable.
// Windows virtual-key codes for the activation binds.
// All bindings start unset on a fresh installation.
std::atomic<int> g_activationVK{ 0 };
std::atomic<int> g_holdActivationVK{ 0 };
std::atomic<int> g_outputVK{ 0 };
std::atomic<bool> g_activationActive{ false };

std::atomic<bool> g_activationCapture{ false };

enum class ActivationCaptureTarget
{
    Primary,
    Hold,
    Output
};

std::atomic<ActivationCaptureTarget> g_activationCaptureTarget{
    ActivationCaptureTarget::Primary
};


// ============================================================
// XINPUT CONTROLLER BINDS
// ============================================================
// Controller binds use negative IDs so they cannot collide with normal
// Windows virtual-key codes (1..254) or the existing 0 = unset value.
constexpr int CONTROLLER_BIND_BASE = -1000;
constexpr int CONTROLLER_BIND_STRIDE = 64;

enum ControllerButtonIndex : int
{
    ControllerDPadUp = 0,
    ControllerDPadDown,
    ControllerDPadLeft,
    ControllerDPadRight,
    ControllerStart,
    ControllerBack,
    ControllerLeftThumb,
    ControllerRightThumb,
    ControllerLeftShoulder,
    ControllerRightShoulder,
    ControllerA,
    ControllerB,
    ControllerX,
    ControllerY,
    ControllerLeftTrigger,
    ControllerRightTrigger,
    ControllerCount
};

static int MakeControllerBind(int controllerIndex, int buttonIndex)
{
    return CONTROLLER_BIND_BASE -
        controllerIndex * CONTROLLER_BIND_STRIDE -
        buttonIndex - 1;
}

static bool DecodeControllerBind(int code, int& controllerIndex, int& buttonIndex)
{
    if (code > CONTROLLER_BIND_BASE)
        return false;

    const int value = CONTROLLER_BIND_BASE - code;
    controllerIndex = value / CONTROLLER_BIND_STRIDE;
    buttonIndex = value % CONTROLLER_BIND_STRIDE - 1;

    return controllerIndex >= 0 && controllerIndex < XUSER_MAX_COUNT &&
        buttonIndex >= 0 && buttonIndex < ControllerCount;
}

static const wchar_t* ControllerButtonName(int buttonIndex)
{
    switch (buttonIndex)
    {
    case ControllerDPadUp:          return L"D-Pad Up";
    case ControllerDPadDown:        return L"D-Pad Down";
    case ControllerDPadLeft:        return L"D-Pad Left";
    case ControllerDPadRight:       return L"D-Pad Right";
    case ControllerStart:           return L"Start";
    case ControllerBack:            return L"Back";
    case ControllerLeftThumb:       return L"Left Stick";
    case ControllerRightThumb:      return L"Right Stick";
    case ControllerLeftShoulder:    return L"LB";
    case ControllerRightShoulder:   return L"RB";
    case ControllerA:               return L"A";
    case ControllerB:               return L"B";
    case ControllerX:               return L"X";
    case ControllerY:               return L"Y";
    case ControllerLeftTrigger:     return L"LT";
    case ControllerRightTrigger:    return L"RT";
    default:                        return L"Controller";
    }
}

static bool ControllerButtonDown(const XINPUT_GAMEPAD& pad, int buttonIndex)
{
    switch (buttonIndex)
    {
    case ControllerDPadUp:          return (pad.wButtons & XINPUT_GAMEPAD_DPAD_UP) != 0;
    case ControllerDPadDown:        return (pad.wButtons & XINPUT_GAMEPAD_DPAD_DOWN) != 0;
    case ControllerDPadLeft:        return (pad.wButtons & XINPUT_GAMEPAD_DPAD_LEFT) != 0;
    case ControllerDPadRight:       return (pad.wButtons & XINPUT_GAMEPAD_DPAD_RIGHT) != 0;
    case ControllerStart:           return (pad.wButtons & XINPUT_GAMEPAD_START) != 0;
    case ControllerBack:            return (pad.wButtons & XINPUT_GAMEPAD_BACK) != 0;
    case ControllerLeftThumb:       return (pad.wButtons & XINPUT_GAMEPAD_LEFT_THUMB) != 0;
    case ControllerRightThumb:      return (pad.wButtons & XINPUT_GAMEPAD_RIGHT_THUMB) != 0;
    case ControllerLeftShoulder:    return (pad.wButtons & XINPUT_GAMEPAD_LEFT_SHOULDER) != 0;
    case ControllerRightShoulder:   return (pad.wButtons & XINPUT_GAMEPAD_RIGHT_SHOULDER) != 0;
    case ControllerA:               return (pad.wButtons & XINPUT_GAMEPAD_A) != 0;
    case ControllerB:               return (pad.wButtons & XINPUT_GAMEPAD_B) != 0;
    case ControllerX:               return (pad.wButtons & XINPUT_GAMEPAD_X) != 0;
    case ControllerY:               return (pad.wButtons & XINPUT_GAMEPAD_Y) != 0;
    case ControllerLeftTrigger:     return pad.bLeftTrigger >= 30;
    case ControllerRightTrigger:    return pad.bRightTrigger >= 30;
    default:                        return false;
    }
}

static std::atomic<bool> g_controllerThreadRunning{ false };
HANDLE g_controllerThread = nullptr;

// Activation/input state declarations needed by controller event handling.
std::atomic<bool> g_activationCaptureArmed{ false };
extern HWND g_hwnd;
void CompleteBindCapture(int code);
bool ConsumeBindingInput(int code, bool down, bool up);
extern std::atomic<int> g_captureBlockVK;
void UpdateActivationActive();
std::atomic<bool> g_primaryKeyDown{ false };
std::atomic<bool> g_holdKeyDown{ false };
std::atomic<bool> g_toggleState{ false };

void HandleControllerButtonEvent(int controllerIndex, int buttonIndex, bool down)
{
    const int code = MakeControllerBind(controllerIndex, buttonIndex);

    if (g_captureBlockVK.load() == code)
    {
        if (!down) g_captureBlockVK.store(0);
        return;
    }

    if (g_activationCapture.load(std::memory_order_acquire))
    {
        if (!g_activationCaptureArmed.load(std::memory_order_acquire))
            return;

        // Controller buttons/triggers are valid for Toggle and Hold,
        // but Output is intentionally restricted to keyboard/mouse.
        if (down)
        {
            const ActivationCaptureTarget target =
                g_activationCaptureTarget.load(std::memory_order_acquire);

            if (target != ActivationCaptureTarget::Output)
                PostMessageW(g_hwnd, WM_APP + 43, code, 0);
        }

        return;
    }

    const int primary = g_activationVK.load(std::memory_order_acquire);
    const int hold = g_holdActivationVK.load(std::memory_order_acquire);

    if (down && code == primary)
    {
        const bool wasAlreadyDown =
            g_primaryKeyDown.exchange(true, std::memory_order_acq_rel);

        if (!wasAlreadyDown)
        {
            bool currentToggle = g_toggleState.load(std::memory_order_acquire);
            while (!g_toggleState.compare_exchange_weak(
                currentToggle,
                !currentToggle,
                std::memory_order_acq_rel,
                std::memory_order_acquire))
            {
            }

            if (g_hwnd)
                PostMessageW(g_hwnd, WM_APP + 41, 0, 0);
        }
    }

    if (down && code == hold)
        g_holdKeyDown.store(true, std::memory_order_release);

    if (!down)
    {
        if (code == primary)
            g_primaryKeyDown.store(false, std::memory_order_release);

        if (code == hold)
            g_holdKeyDown.store(false, std::memory_order_release);
    }

    UpdateActivationActive();
}

void ControllerInputThread()
{
    SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);

    XINPUT_STATE previous[XUSER_MAX_COUNT]{};
    bool connected[XUSER_MAX_COUNT]{};

    g_controllerThreadRunning.store(true, std::memory_order_release);

    while (g_running.load(std::memory_order_relaxed))
    {
        for (DWORD i = 0; i < XUSER_MAX_COUNT; ++i)
        {
            XINPUT_STATE state{};
            const DWORD result = XInputGetState(i, &state);

            if (result != ERROR_SUCCESS)
            {
                if (connected[i])
                {
                    // Treat disconnect as release of every previously held
                    // button so HOLD binds cannot remain stuck active.
                    for (int b = 0; b < ControllerCount; ++b)
                    {
                        if (ControllerButtonDown(previous[i].Gamepad, b))
                            HandleControllerButtonEvent(static_cast<int>(i), b, false);
                    }
                }

                connected[i] = false;
                previous[i] = {};
                continue;
            }

            connected[i] = true;

            for (int b = 0; b < ControllerCount; ++b)
            {
                const bool wasDown = ControllerButtonDown(previous[i].Gamepad, b);
                const bool isDown = ControllerButtonDown(state.Gamepad, b);

                if (wasDown != isDown)
                    HandleControllerButtonEvent(static_cast<int>(i), b, isDown);
            }

            previous[i] = state;
        }

        // Controller polling is isolated from the screen detector. This does
        // not add any Sleep/yield to the active detection path.
        Sleep(2);
    }

    g_controllerThreadRunning.store(false, std::memory_order_release);
}

// Capture is completed directly by the hook. These atomics are only used for activation state.

// The mouse/key that opened the bind button must be released before the next
// input can be captured.
std::atomic<bool> g_captureWaitingForRelease{ false };
std::atomic<int> g_captureBlockVK{ 0 };

// Persistence is defined below the activation helpers.
void SaveSettings();
void LoadSettings();

bool IsActivationBindDown(int vk)
{
    int controllerIndex = 0;
    int buttonIndex = 0;
    if (DecodeControllerBind(vk, controllerIndex, buttonIndex))
    {
        XINPUT_STATE state{};
        if (XInputGetState(static_cast<DWORD>(controllerIndex), &state) != ERROR_SUCCESS)
            return false;
        return ControllerButtonDown(state.Gamepad, buttonIndex);
    }

    if (vk <= 0 || vk > 254)
        return false;

    return (GetAsyncKeyState(vk) & 0x8000) != 0;
}

const wchar_t* ActivationKeyName(int vk)
{
    int controllerIndex = 0;
    int buttonIndex = 0;
    if (DecodeControllerBind(vk, controllerIndex, buttonIndex))
    {
        static thread_local wchar_t controllerName[64]{};
        wsprintfW(
            controllerName,
            L"Controller %d %s",
            controllerIndex + 1,
            ControllerButtonName(buttonIndex)
        );
        return controllerName;
    }

    switch (vk)
    {
    case 0: return L"Not Set";
    case VK_LBUTTON:  return L"Left Mouse";
    case VK_RBUTTON:  return L"Right Mouse";
    case VK_MBUTTON:  return L"Middle Mouse";
    case VK_XBUTTON1: return L"Mouse 4";
    case VK_XBUTTON2: return L"Mouse 5";
    case VK_SPACE:    return L"Space";
    case VK_RETURN:   return L"Enter";
    case VK_TAB:      return L"Tab";
    case VK_ESCAPE:   return L"Esc";
    case VK_SHIFT:    return L"Shift";
    case VK_CONTROL:  return L"Ctrl";
    case VK_MENU:     return L"Alt";
    case VK_CAPITAL:  return L"Caps";
    case VK_BACK:     return L"Backspace";
    case VK_DELETE:   return L"Delete";
    case VK_INSERT:   return L"Insert";
    case VK_HOME:     return L"Home";
    case VK_END:      return L"End";
    case VK_PRIOR:    return L"Page Up";
    case VK_NEXT:     return L"Page Down";
    case VK_UP:       return L"Up";
    case VK_DOWN:     return L"Down";
    case VK_LEFT:     return L"Left";
    case VK_RIGHT:    return L"Right";
    case VK_LSHIFT:   return L"Left Shift";
    case VK_RSHIFT:   return L"Right Shift";
    case VK_LCONTROL: return L"Left Ctrl";
    case VK_RCONTROL: return L"Right Ctrl";
    case VK_LMENU:    return L"Left Alt";
    case VK_RMENU:    return L"Right Alt";
    default:
        break;
    }

    static thread_local wchar_t name[32]{};

    if (vk >= 'A' && vk <= 'Z')
    {
        name[0] = static_cast<wchar_t>(vk);
        name[1] = L'\0';
        return name;
    }

    if (vk >= '0' && vk <= '9')
    {
        name[0] = static_cast<wchar_t>(vk);
        name[1] = L'\0';
        return name;
    }

    if (vk >= VK_F1 && vk <= VK_F24)
    {
        wsprintfW(name, L"F%d", vk - VK_F1 + 1);
        return name;
    }

    UINT scan = MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC_EX, GetKeyboardLayout(0));
    LONG bits = (scan & 0xFF) << 16;
    if (scan & 0xFF00) bits |= 1 << 24;
    if (GetKeyNameTextW(bits, name, ARRAYSIZE(name))) return name;
    return L"Unknown key";
}

HANDLE g_activationWakeEvent = nullptr;

void UpdateActivationActive()
{
    const bool active =
        g_toggleState.load(std::memory_order_acquire) ||
        g_holdKeyDown.load(std::memory_order_acquire);

    const bool previous =
        g_activationActive.exchange(active, std::memory_order_acq_rel);

    // Wake the scanner immediately when activation changes.  This is only
    // used for the inactive/active transition; the active detector remains
    // free-running for minimum capture latency.
    if (previous != active && g_activationWakeEvent)
        SetEvent(g_activationWakeEvent);
}

void BeginActivationCapture(ActivationCaptureTarget target)
{
    g_activationCaptureTarget.store(target, std::memory_order_release);

    g_activationActive.store(false, std::memory_order_release);
    if (g_activationWakeEvent) SetEvent(g_activationWakeEvent);
    g_toggleState.store(false, std::memory_order_release);
    g_primaryKeyDown.store(false, std::memory_order_release);
    g_holdKeyDown.store(false, std::memory_order_release);

    // The bind button is activated by LMB. We wait for its matching
    // WM_LBUTTONUP before accepting any further raw input.
    g_captureWaitingForRelease.store(true, std::memory_order_release);
    g_activationCaptureArmed.store(false, std::memory_order_release);
    g_activationCapture.store(true, std::memory_order_release);

    InvalidateRect(g_hwnd, nullptr, FALSE);
}

void CompleteBindCapture(int vk)
{
    int controllerIndex = 0;
    int buttonIndex = 0;
    const bool isControllerBind = DecodeControllerBind(vk, controllerIndex, buttonIndex);

    if ((!isControllerBind && (vk < 0 || vk > 254)) ||
        (isControllerBind && buttonIndex < 0))
        return;

    // Output is keyboard/mouse only. Controller binds remain available
    // for Toggle and Hold.
    const ActivationCaptureTarget captureTarget =
        g_activationCaptureTarget.load(std::memory_order_acquire);

    if (captureTarget == ActivationCaptureTarget::Output && isControllerBind)
        return;

    if (!g_activationCapture.load(std::memory_order_acquire) ||
        !g_activationCaptureArmed.load(std::memory_order_acquire))
        return;

    const ActivationCaptureTarget target =
        g_activationCaptureTarget.load(std::memory_order_acquire);

    if (vk == VK_ESCAPE) vk = 0;
    g_captureBlockVK.store(vk, std::memory_order_release);

    if (target == ActivationCaptureTarget::Primary)
    {
        g_activationVK.store(vk, std::memory_order_release);
        g_primaryKeyDown.store(false, std::memory_order_release);
        g_toggleState.store(false, std::memory_order_release);
    }
    else if (target == ActivationCaptureTarget::Hold)
    {
        g_holdActivationVK.store(vk, std::memory_order_release);
        g_holdKeyDown.store(false, std::memory_order_release);
    }
    else
    {
        g_outputVK.store(vk, std::memory_order_release);
    }

    g_activationCapture.store(false, std::memory_order_release);
    g_activationCaptureArmed.store(false, std::memory_order_release);
    g_captureWaitingForRelease.store(false, std::memory_order_release);
    g_activationActive.store(false, std::memory_order_release);

    if (g_hwnd)
    {
        PostMessageW(
            g_hwnd,
            WM_APP + 42,
            static_cast<WPARAM>(target),
            static_cast<LPARAM>(vk)
        );
    }

    InvalidateRect(g_hwnd, nullptr, FALSE);
}

// Raw Input is used instead of GetAsyncKeyState polling for activation.
// RIDEV_INPUTSINK makes the window receive keyboard/mouse input even when
// the Yami window is not the foreground window. This gives us actual input
// edges rather than hoping a polling loop catches them.
bool RegisterRawActivationInput(HWND hwnd)
{
    RAWINPUTDEVICE devices[2]{};

    devices[0].usUsagePage = 0x01; // Generic Desktop Controls
    devices[0].usUsage = 0x06; // Keyboard
    devices[0].dwFlags = RIDEV_INPUTSINK;
    devices[0].hwndTarget = hwnd;

    devices[1].usUsagePage = 0x01; // Generic Desktop Controls
    devices[1].usUsage = 0x02; // Mouse
    devices[1].dwFlags = RIDEV_INPUTSINK;
    devices[1].hwndTarget = hwnd;

    const bool ok = RegisterRawInputDevices(
        devices,
        ARRAYSIZE(devices),
        sizeof(RAWINPUTDEVICE)
    ) != FALSE;

    if (!ok)
    {
        const DWORD error = GetLastError();
        wchar_t buffer[160]{};
        wsprintfW(
            buffer,
            L"Yami TB: RegisterRawInputDevices FAILED, error=%lu\\n",
            error
        );
        OutputDebugStringW(buffer);
    }
    else
    {
        OutputDebugStringW(L"Yami TB: RegisterRawInputDevices OK\\n");
    }

    return ok;
}

int RawKeyboardToVK(const RAWKEYBOARD& keyboard)
{
    UINT scan = keyboard.MakeCode;

    if (keyboard.Flags & RI_KEY_E0)
        scan |= 0xE000;
    else if (keyboard.Flags & RI_KEY_E1)
        scan |= 0xE100;

    UINT vk = MapVirtualKeyExW(
        scan,
        MAPVK_VSC_TO_VK_EX,
        GetKeyboardLayout(0)
    );

    if (vk == 0 && keyboard.VKey != 0)
        vk = keyboard.VKey;

    if (vk > 254)
        return 0;

    return static_cast<int>(vk);
}

extern HWND g_hwnd;

void HandleActivationEvent(int vk, bool down, bool up);

void HandleRawActivationInput(HRAWINPUT hRawInput)
{

    UINT size = 0;

    if (GetRawInputData(
        hRawInput,
        RID_INPUT,
        nullptr,
        &size,
        sizeof(RAWINPUTHEADER)
    ) == static_cast<UINT>(-1) || size == 0)
    {
        return;
    }

    std::array<BYTE, 512> stackBuffer{};

    BYTE* data = stackBuffer.data();
    std::vector<BYTE> heapBuffer;

    if (size > stackBuffer.size())
    {
        heapBuffer.resize(size);
        data = heapBuffer.data();
    }

    if (GetRawInputData(
        hRawInput,
        RID_INPUT,
        data,
        &size,
        sizeof(RAWINPUTHEADER)
    ) != size)
    {
        return;
    }

    const RAWINPUT* raw =
        reinterpret_cast<const RAWINPUT*>(data);

    int vk = 0;
    bool down = false;
    bool up = false;

    if (raw->header.dwType == RIM_TYPEKEYBOARD)
    {
        const RAWKEYBOARD& keyboard = raw->data.keyboard;

        vk = RawKeyboardToVK(keyboard);

        down = (keyboard.Flags & RI_KEY_BREAK) == 0;
        up = !down;

        // Raw input can report fake zero-VKey/zero-scan records.
        if (vk <= 0 || vk > 254)
            return;
    }
    else if (raw->header.dwType == RIM_TYPEMOUSE)
    {
        const USHORT flags = raw->data.mouse.usButtonFlags;

        if (flags & RI_MOUSE_LEFT_BUTTON_DOWN)
        {
            vk = VK_LBUTTON;
            down = true;
        }
        else if (flags & RI_MOUSE_LEFT_BUTTON_UP)
        {
            vk = VK_LBUTTON;
            up = true;
        }
        else if (flags & RI_MOUSE_RIGHT_BUTTON_DOWN)
        {
            vk = VK_RBUTTON;
            down = true;
        }
        else if (flags & RI_MOUSE_RIGHT_BUTTON_UP)
        {
            vk = VK_RBUTTON;
            up = true;
        }
        else if (flags & RI_MOUSE_MIDDLE_BUTTON_DOWN)
        {
            vk = VK_MBUTTON;
            down = true;
        }
        else if (flags & RI_MOUSE_MIDDLE_BUTTON_UP)
        {
            vk = VK_MBUTTON;
            up = true;
        }
        else if (flags & RI_MOUSE_BUTTON_4_DOWN)
        {
            vk = VK_XBUTTON1;
            down = true;
        }
        else if (flags & RI_MOUSE_BUTTON_4_UP)
        {
            vk = VK_XBUTTON1;
            up = true;
        }
        else if (flags & RI_MOUSE_BUTTON_5_DOWN)
        {
            vk = VK_XBUTTON2;
            down = true;
        }
        else if (flags & RI_MOUSE_BUTTON_5_UP)
        {
            vk = VK_XBUTTON2;
            up = true;
        }
        else
        {
            return;
        }
    }
    else
    {
        return;
    }

    if (ConsumeBindingInput(vk, down, up)) return;
    HandleActivationEvent(vk, down, up);
}

// Existing activation edge handling, shared with integration tests.
void HandleActivationEvent(int vk, bool down, bool up)
{
    if (vk < 1 || vk > 254) return;

    const int primaryVK =
        g_activationVK.load(std::memory_order_acquire);

    const int holdVK =
        g_holdActivationVK.load(std::memory_order_acquire);

    if (down)
    {
        if (vk == primaryVK)
        {
            // exchange() is the edge detector. Do not store true before it:
            // otherwise exchange(true) always reports an already-down key and
            // the toggle can never flip.
            const bool wasAlreadyDown =
                g_primaryKeyDown.exchange(true, std::memory_order_acq_rel);

            // Raw Input may repeat keyboard-down records while held, so only
            // the first DOWN edge flips the toggle.
            if (!wasAlreadyDown)
            {
                bool currentToggle = g_toggleState.load(std::memory_order_acquire);

                while (!g_toggleState.compare_exchange_weak(
                    currentToggle,
                    !currentToggle,
                    std::memory_order_acq_rel,
                    std::memory_order_acquire))
                {
                }

                if (g_hwnd)
                    PostMessageW(g_hwnd, WM_APP + 41, 0, 0);
            }

        }

        if (vk == holdVK)
            g_holdKeyDown.store(true, std::memory_order_release);

        UpdateActivationActive();
    }
    else if (up)
    {
        if (vk == primaryVK)
            g_primaryKeyDown.store(false, std::memory_order_release);

        if (vk == holdVK)
            g_holdKeyDown.store(false, std::memory_order_release);

        UpdateActivationActive();
    }
}

HWND g_hwnd = nullptr;
HANDLE g_scannerThread = nullptr;

