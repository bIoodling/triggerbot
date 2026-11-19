// Settings are UI-thread owned. Detection reads only atomic configuration.
#include <shlobj.h>
#include <shellapi.h>
#include <gdiplus.h>
#include <dwmapi.h>
#include <fstream>
#include <filesystem>
#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "advapi32.lib")
#pragma comment(lib, "gdiplus.lib")
#pragma comment(lib, "dwmapi.lib")
#pragma comment(lib, "ole32.lib")
bool g_launchMinimized = false;
bool g_startWindows = false;
bool g_saveFailed = false;
std::wstring g_settingsOverride;

std::wstring GetSettingsDirectory()
{
    if (!g_settingsOverride.empty()) return g_settingsOverride;
    PWSTR path = nullptr;
    if (FAILED(SHGetKnownFolderPath(FOLDERID_RoamingAppData, 0, nullptr, &path))) return L"";
    std::wstring result = std::wstring(path) + L"\\YamiTB";
    CoTaskMemFree(path);
    CreateDirectoryW(result.c_str(), nullptr);
    return result;
}
std::wstring GetSettingsPath()
{
    auto directory = GetSettingsDirectory();
    return directory.empty() ? L"" : directory + L"\\settings.ini";
}
bool ParseHex(const std::wstring& input, uint32_t& value)
{
    const size_t first = !input.empty() && input[0] == L'#' ? 1 : 0;
    if (input.size() - first != 6) return false;
    uint32_t result = 0;
    for (size_t i = first; i < input.size(); ++i)
    {
        wchar_t c = input[i];
        int digit = c >= L'0' && c <= L'9' ? c - L'0' :
            c >= L'a' && c <= L'f' ? c - L'a' + 10 :
            c >= L'A' && c <= L'F' ? c - L'A' + 10 : -1;
        if (digit < 0) return false;
        result = (result << 4) | digit;
    }
    value = result;
    return true;
}
std::wstring HexColor(uint32_t color)
{
    wchar_t buffer[12];
    swprintf_s(buffer, L"#%06X", color & 0xFFFFFF);
    return buffer;
}
bool StartupRegistered()
{
    HKEY key{};
    if (RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run", 0, KEY_READ, &key)) return false;
    DWORD bytes = 0;
    bool exists = RegQueryValueExW(key, L"Yami", nullptr, nullptr, nullptr, &bytes) == ERROR_SUCCESS;
    RegCloseKey(key);
    return exists;
}
bool SetStartup(bool enabled)
{
    HKEY key{};
    if (RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Run", 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr)) return false;
    LSTATUS result;
    if (enabled)
    {
        wchar_t exe[32768]{};
        GetModuleFileNameW(nullptr, exe, ARRAYSIZE(exe));
        std::wstring command = L"\"" + std::wstring(exe) + L"\" --tray";
        result = RegSetValueExW(key, L"Yami", 0, REG_SZ, reinterpret_cast<const BYTE*>(command.c_str()), DWORD((command.size()+1)*sizeof(wchar_t)));
    }
    else result = RegDeleteValueW(key, L"Yami");
    RegCloseKey(key);
    return result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND);
}
void SaveSettings()
{
    const auto path = GetSettingsPath();
    if (path.empty()) { g_saveFailed = true; return; }
    // Replace one complete UTF-16 INI atomically; a partial write cannot
    // leave a mixture of old/new bindings after a power loss.
    std::wostringstream out;
    out << L"[Activation]\r\nPrimaryVK=" << g_activationVK.load()
        << L"\r\nHoldVK=" << g_holdActivationVK.load()
        << L"\r\n[Output]\r\nVK=" << g_outputVK.load()
        << L"\r\n[Detector]\r\nTargetRGB=" << HexColor(g_targetColor.load()).substr(1)
        << L"\r\n[Window]\r\nLaunchMinimized=" << int(g_launchMinimized) << L"\r\n";
    const auto options=ReadDetectionOptions();
    out.precision(12);
    out << L"[Detection]\r\nCursorless=" << int(options.cursorless)
        << L"\r\nCalibrated=" << int(options.calibrated)
        << L"\r\nNormalizedX=" << options.normalizedX << L"\r\nNormalizedY=" << options.normalizedY
        << L"\r\nTolerance=" << options.tolerance << L"\r\nCursorRadius=" << options.cursorRadius
        << L"\r\nFixedRadius=" << options.fixedRadius << L"\r\nClearMs=" << options.clearMs
        << L"\r\nClearFrames=" << options.clearFrames << L"\r\nMinimumIntervalMs=" << options.minimumIntervalMs << L"\r\n";
    std::wstring content = L"\xFEFF" + out.str();
    std::wstring temp = path + L".tmp";
    HANDLE file = CreateFileW(temp.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) { g_saveFailed = true; return; }
    DWORD written = 0, bytes = DWORD(content.size() * sizeof(wchar_t));
    bool ok = WriteFile(file, content.data(), bytes, &written, nullptr) && written == bytes;
    ok = FlushFileBuffers(file) && ok;
    CloseHandle(file);
    ok = ok && MoveFileExW(temp.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
    g_saveFailed = !ok;
}
void LoadSettings()
{
    const auto path = GetSettingsPath();
    auto loadBind = [&](const wchar_t* section, const wchar_t* name, bool controller) {
        int vk = int(GetPrivateProfileIntW(section, name, 0, path.c_str()));
        int index = 0, button = 0;
        return (vk >= 0 && vk <= 254) || (controller && DecodeControllerBind(vk, index, button)) ? vk : 0;
    };
    g_activationVK = loadBind(L"Activation", L"PrimaryVK", true);
    g_holdActivationVK = loadBind(L"Activation", L"HoldVK", true);
    g_outputVK = loadBind(L"Output", L"VK", false);
    wchar_t hex[64]{};
    GetPrivateProfileStringW(L"Detector", L"TargetRGB", L"00FF00", hex, ARRAYSIZE(hex), path.c_str());
    uint32_t rgb = 0x00FF00;
    if (!ParseHex(hex, rgb)) rgb = 0x00FF00;
    g_targetColor = rgb;
    g_launchMinimized = GetPrivateProfileIntW(L"Window", L"LaunchMinimized", 0, path.c_str()) != 0;
    g_startWindows = StartupRegistered();
    auto number=[&](const wchar_t* key,double fallback,double minimum,double maximum){
        wchar_t value[64]{};GetPrivateProfileStringW(L"Detection",key,L"",value,ARRAYSIZE(value),path.c_str());
        wchar_t* end=nullptr;double loaded=wcstod(value,&end);
        return end!=value&&*end==0&&std::isfinite(loaded)&&loaded>=minimum&&loaded<=maximum?loaded:fallback;
    };
    {std::lock_guard lock(g_optionsMutex);
        g_options.cursorless=number(L"Cursorless",0,0,1)!=0;g_options.calibrated=number(L"Calibrated",0,0,1)!=0;
        g_options.normalizedX=number(L"NormalizedX",.5,0,1);g_options.normalizedY=number(L"NormalizedY",.5,0,1);
        g_options.tolerance=unsigned(number(L"Tolerance",2,0,16));g_options.cursorRadius=unsigned(number(L"CursorRadius",50,1,128));g_options.fixedRadius=unsigned(number(L"FixedRadius",12,1,128));
        g_options.clearMs=number(L"ClearMs",8,0,100);g_options.clearFrames=unsigned(number(L"ClearFrames",2,2,16));g_options.minimumIntervalMs=number(L"MinimumIntervalMs",0,0,1000);
    }
    g_toggleState = true;
    g_primaryKeyDown = false;
    g_holdKeyDown = false;
    g_activationActive = false;
}
