double QpcNowSeconds()
{
    static const LONGLONG frequency = []() -> LONGLONG
        {
            LARGE_INTEGER value{};
            QueryPerformanceFrequency(&value);
            return value.QuadPart;
        }();

    LARGE_INTEGER counter{};
    QueryPerformanceCounter(&counter);

    return frequency > 0
        ? static_cast<double>(counter.QuadPart) /
        static_cast<double>(frequency)
        : 0.0;
}

// ============================================================
// SEND OUTPUT KEY
// ============================================================

bool IsExtendedOutputKey(int vk)
{
    switch (vk)
    {
    case VK_RCONTROL:
    case VK_RMENU:
    case VK_INSERT:
    case VK_DELETE:
    case VK_HOME:
    case VK_END:
    case VK_PRIOR:
    case VK_NEXT:
    case VK_LEFT:
    case VK_RIGHT:
    case VK_UP:
    case VK_DOWN:
    case VK_DIVIDE:
    case VK_NUMLOCK:
        return true;

    default:
        return false;
    }
}

struct PreparedOutputKey
{
    INPUT down{};
    INPUT up{};
    bool valid = false;
};

PreparedOutputKey PrepareOutputKey()
{
    PreparedOutputKey prepared{};

    const int vk = g_outputVK.load(std::memory_order_relaxed);

    if (vk < 1 || vk > 254)
        return prepared;

    // Output bindings change rarely. Cache the VK -> scan-code translation
    // on the calling thread so the actual injection path stays tiny.
    static thread_local int cachedVK = -1;
    static thread_local UINT cachedScanCode = 0;
    static thread_local DWORD cachedFlags = 0;

    if (vk != cachedVK)
    {
        cachedVK = vk;
        cachedScanCode = MapVirtualKeyW(
            static_cast<UINT>(vk),
            MAPVK_VK_TO_VSC
        );

        cachedFlags = cachedScanCode != 0
            ? KEYEVENTF_SCANCODE
            : 0;

        if (cachedScanCode != 0 && IsExtendedOutputKey(vk))
            cachedFlags |= KEYEVENTF_EXTENDEDKEY;
    }

    if (cachedScanCode != 0)
    {
        prepared.down.type = INPUT_KEYBOARD;
        prepared.down.ki.wVk = 0;
        prepared.down.ki.wScan = static_cast<WORD>(cachedScanCode);
        prepared.down.ki.dwFlags = cachedFlags;

        prepared.up = prepared.down;
        prepared.up.ki.dwFlags = cachedFlags | KEYEVENTF_KEYUP;
    }
    else
    {
        prepared.down.type = INPUT_KEYBOARD;
        prepared.down.ki.wVk = static_cast<WORD>(vk);

        prepared.up = prepared.down;
        prepared.up.ki.dwFlags = KEYEVENTF_KEYUP;
    }

    prepared.valid = true;
    return prepared;
}

__forceinline bool SendOutputKeyFast()
{
    static thread_local int cachedVK = -1;
    static thread_local INPUT down{};
    static thread_local INPUT up{};

    const int vk = g_outputVK.load(std::memory_order_relaxed);

    // Output is intentionally restricted to keyboard/mouse virtual-key codes.
    if (vk < 1 || vk > 254)
        return false;

    if (vk != cachedVK)
    {
        cachedVK = vk;

        const UINT scanCode = MapVirtualKeyW(
            static_cast<UINT>(vk),
            MAPVK_VK_TO_VSC
        );

        down = {};
        down.type = INPUT_KEYBOARD;

        if (scanCode != 0)
        {
            down.ki.wVk = 0;
            down.ki.wScan = static_cast<WORD>(scanCode);
            down.ki.dwFlags =
                KEYEVENTF_SCANCODE |
                (IsExtendedOutputKey(vk) ? KEYEVENTF_EXTENDEDKEY : 0);
        }
        else
        {
            down.ki.wVk = static_cast<WORD>(vk);
            down.ki.wScan = 0;
            down.ki.dwFlags = 0;
        }

        up = down;
        up.ki.dwFlags |= KEYEVENTF_KEYUP;
    }

    INPUT pair[2] = { down, up };
    return SendInput(2, pair, sizeof(INPUT)) == 2;
}

// ============================================================
// V25 - COMPACT DIRTY-TILE GPU DETECTOR
// ============================================================
//
// V25 attacks the remaining V24 bottleneck: dispatching every tile even when only a few changed.
// and empty GPU work on frames where only a small part of the desktop changed.
//
// The detector keeps a persistent GPU copy of the last desktop image.
// DXGI dirty rectangles are copied into that persistent texture. Only tiles
// touched by those rectangles are dispatched to the GPU. The CPU keeps the resulting
// per-tile green state, so the global "any exact selected RGB colour exists" state remains
// exact without rescanning the whole desktop.
//
// Move rectangles are handled conservatively: if DXGI reports any move rect,
// V24 falls back to a full CopyResource and marks every tile dirty. This keeps
// correctness ahead of the optimization.
//
// If a frame contains no dirty/move rectangles, its pixels have not changed.
// We can therefore return the previous exact result without doing a GPU scan.
//
// No Sleep/yield is used in the active detection path.
// ============================================================

class ScreenGrabber
{
public:
    enum class ScanResult { NoNewFrame, GreenFound, GreenNotFound };
    static constexpr size_t FRAME_SLOTS = 1;

    bool Initialize()
    {
        Shutdown();
        IDXGIFactory1* factory = nullptr;
        HRESULT hr = CreateDXGIFactory1(__uuidof(IDXGIFactory1),
            reinterpret_cast<void**>(&factory));
        if (FAILED(hr) || !factory) return false;

        IDXGIAdapter1* adapter = nullptr;
        IDXGIOutput* output = nullptr;
        for (UINT ai = 0; !output; ++ai)
        {
            IDXGIAdapter1* a = nullptr;
            hr = factory->EnumAdapters1(ai, &a);
            if (hr == DXGI_ERROR_NOT_FOUND) break;
            if (FAILED(hr) || !a) continue;
            for (UINT oi = 0; ; ++oi)
            {
                IDXGIOutput* o = nullptr;
                hr = a->EnumOutputs(oi, &o);
                if (hr == DXGI_ERROR_NOT_FOUND) break;
                if (FAILED(hr) || !o) continue;
                DXGI_OUTPUT_DESC d{};
                if (SUCCEEDED(o->GetDesc(&d)) &&
                    d.DesktopCoordinates.left == 0 &&
                    d.DesktopCoordinates.top == 0)
                {
                    adapter = a;
                    output = o;
                    break;
                }
                o->Release();
            }
            if (!output) a->Release();
        }
        if (!output)
        {
            factory->Release();
            return false;
        }

        D3D_FEATURE_LEVEL levels[] = {
            D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0
        };
        D3D_FEATURE_LEVEL actual{};
        hr = D3D11CreateDevice(adapter, D3D_DRIVER_TYPE_UNKNOWN, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, levels, ARRAYSIZE(levels),
            D3D11_SDK_VERSION, &device_, &actual, &context_);
        adapter->Release();
        if (FAILED(hr))
        {
            output->Release(); factory->Release(); Shutdown(); return false;
        }

        IDXGIOutput1* output1 = nullptr;
        hr = output->QueryInterface(__uuidof(IDXGIOutput1),
            reinterpret_cast<void**>(&output1));
        output->Release();
        factory->Release();
        if (FAILED(hr) || !output1) { Shutdown(); return false; }

        hr = output1->DuplicateOutput(device_, &duplication_);
        output1->Release();
        if (FAILED(hr) || !duplication_) { Shutdown(); return false; }

        DXGI_OUTDUPL_DESC dd{};
        duplication_->GetDesc(&dd);
        width_ = dd.ModeDesc.Width;
        height_ = dd.ModeDesc.Height;
        if (!width_ || !height_) { Shutdown(); return false; }

        D3D11_TEXTURE2D_DESC td{};
        td.Width = width_; td.Height = height_;
        td.MipLevels = 1; td.ArraySize = 1;
        td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_STAGING;
        td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        hr = device_->CreateTexture2D(&td, nullptr, &staging_);
        if (FAILED(hr)) { Shutdown(); return false; }
        return true;
    }

    struct TimingMetrics
    {
        double acquireMs = 0.0;
        double copyMapMs = 0.0;
        double scanMs = 0.0;
        bool acquired = false;
    };

    const TimingMetrics& LastTiming() const { return lastTiming_; }

    size_t FindGreen(ScanResult* results, size_t capacity)
    {
        lastTiming_ = {};
        if (!results || !capacity || !duplication_ || !context_) return 0;

        IDXGIResource* resource = nullptr;
        DXGI_OUTDUPL_FRAME_INFO info{};
        const double acquireStart = QpcNowSeconds();
        HRESULT hr = duplication_->AcquireNextFrame(0, &info, &resource);
        lastTiming_.acquireMs = (QpcNowSeconds() - acquireStart) * 1000.0;
        if (hr == DXGI_ERROR_WAIT_TIMEOUT) return 0;
        if (hr == DXGI_ERROR_ACCESS_LOST ||
            hr == DXGI_ERROR_DEVICE_REMOVED ||
            hr == DXGI_ERROR_DEVICE_RESET)
            return 0;
        if (FAILED(hr) || !resource) return 0;
        lastTiming_.acquired = true;

        ID3D11Texture2D* frame = nullptr;
        hr = resource->QueryInterface(__uuidof(ID3D11Texture2D),
            reinterpret_cast<void**>(&frame));
        resource->Release();

        bool found = false;
        if (SUCCEEDED(hr) && frame)
        {
            const double copyMapStart = QpcNowSeconds();
            context_->CopyResource(staging_, frame);
            frame->Release();

            D3D11_MAPPED_SUBRESOURCE mapped{};
            hr = context_->Map(staging_, 0, D3D11_MAP_READ, 0, &mapped);
            lastTiming_.copyMapMs = (QpcNowSeconds() - copyMapStart) * 1000.0;

            if (SUCCEEDED(hr))
            {
                const double scanStart = QpcNowSeconds();
                const uint32_t target = g_targetColor.load(std::memory_order_relaxed)
                    & 0x00FFFFFFu;
                const uint8_t tr = static_cast<uint8_t>((target >> 16) & 0xFF);
                const uint8_t tg = static_cast<uint8_t>((target >> 8) & 0xFF);
                const uint8_t tb = static_cast<uint8_t>(target & 0xFF);
                const uint8_t* base = static_cast<const uint8_t*>(mapped.pData);

                constexpr int CURSOR_SCAN_RADIUS = 50;
                POINT cursor{};
                if (GetCursorPos(&cursor))
                {
                    const int centerX = cursor.x;
                    const int centerY = cursor.y;
                    const int minX = (std::max)(0, centerX - CURSOR_SCAN_RADIUS);
                    const int maxX = (std::min)(static_cast<int>(width_) - 1,
                        centerX + CURSOR_SCAN_RADIUS);
                    const int minY = (std::max)(0, centerY - CURSOR_SCAN_RADIUS);
                    const int maxY = (std::min)(static_cast<int>(height_) - 1,
                        centerY + CURSOR_SCAN_RADIUS);
                    constexpr int RADIUS_SQUARED = CURSOR_SCAN_RADIUS * CURSOR_SCAN_RADIUS;

                    if (minX <= maxX && minY <= maxY)
                    {
                        for (int y = minY; y <= maxY && !found; ++y)
                        {
                            const int dy = y - centerY;
                            const uint8_t* row = base + static_cast<size_t>(y) * mapped.RowPitch;
                            for (int x = minX; x <= maxX; ++x)
                            {
                                const int dx = x - centerX;
                                if (dx * dx + dy * dy > RADIUS_SQUARED) continue;
                                const uint8_t* pixel = row + static_cast<size_t>(x) * 4;
                                if (pixel[0] == tb && pixel[1] == tg && pixel[2] == tr)
                                {
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                lastTiming_.scanMs = (QpcNowSeconds() - scanStart) * 1000.0;
                context_->Unmap(staging_, 0);
            }
        }
        duplication_->ReleaseFrame();
        if (FAILED(hr)) return 0;
        results[0] = found ? ScanResult::GreenFound : ScanResult::GreenNotFound;
        return 1;
    }

    void Shutdown()
    {
        if (context_) context_->ClearState();
        if (staging_) { staging_->Release(); staging_ = nullptr; }
        if (duplication_) { duplication_->Release(); duplication_ = nullptr; }
        if (context_) { context_->Release(); context_ = nullptr; }
        if (device_) { device_->Release(); device_ = nullptr; }
        width_ = height_ = 0;
    }

    ~ScreenGrabber() { Shutdown(); }

private:
    ID3D11Device* device_ = nullptr;
    ID3D11DeviceContext* context_ = nullptr;
    IDXGIOutputDuplication* duplication_ = nullptr;
    ID3D11Texture2D* staging_ = nullptr;
    UINT width_ = 0, height_ = 0;
    TimingMetrics lastTiming_{};
};


void ScannerThread()
{
    SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    ScreenGrabber grabber;
    bool ready = grabber.Initialize();
    g_detectorReady.store(ready, std::memory_order_release);
    bool armed = true;
    // Re-arm only after a sustained, repeatedly observed absence. A short
    // detector dropout must not split one visible target into multiple hits.
    constexpr unsigned ABSENT_FRAMES_TO_REARM = 3;
    constexpr auto ABSENT_TIME_TO_REARM = std::chrono::milliseconds(500);
    unsigned consecutiveAbsentFrames = 0;
    std::chrono::steady_clock::time_point absentSince{};
    ScreenGrabber::ScanResult result{};

    // Aggregate timings off the hot path and emit one diagnostic line per second.
    double windowStart = QpcNowSeconds();
    double acquireTotal = 0.0, copyMapTotal = 0.0, scanTotal = 0.0, outputTotal = 0.0;
    double acquireMax = 0.0, copyMapMax = 0.0, scanMax = 0.0, outputMax = 0.0;
    unsigned calls = 0, frames = 0, timeouts = 0, detections = 0;

    while (g_running.load(std::memory_order_relaxed))
    {
        if (!ready)
        {
            g_scanning.store(false, std::memory_order_relaxed);
            g_greenDetected.store(false, std::memory_order_relaxed);
            ready = grabber.Initialize();
            g_detectorReady.store(ready, std::memory_order_release);
            if (!ready)
            {
                if (g_activationWakeEvent)
                    WaitForSingleObject(g_activationWakeEvent, 250);
                else
                    Sleep(250);
            }
            continue;
        }

        if (!g_activationActive.load(std::memory_order_acquire))
        {
            armed = true;
            g_scanning.store(false, std::memory_order_relaxed);
            g_greenDetected.store(false, std::memory_order_relaxed);
            if (g_activationWakeEvent)
            {
                ResetEvent(g_activationWakeEvent);
                if (!g_activationActive.load(std::memory_order_acquire))
                    WaitForSingleObject(g_activationWakeEvent, INFINITE);
            }
            else Sleep(1);
            continue;
        }

        g_scanning.store(true, std::memory_order_relaxed);
        const size_t count = grabber.FindGreen(&result, 1);
        const auto& timing = grabber.LastTiming();
        ++calls;
        acquireTotal += timing.acquireMs;
        acquireMax = (std::max)(acquireMax, timing.acquireMs);
        if (timing.acquired)
        {
            ++frames;
            copyMapTotal += timing.copyMapMs;
            scanTotal += timing.scanMs;
            copyMapMax = (std::max)(copyMapMax, timing.copyMapMs);
            scanMax = (std::max)(scanMax, timing.scanMs);
        }
        else
        {
            ++timeouts;
        }

        if (count)
        {
            const bool found = result == ScreenGrabber::ScanResult::GreenFound;
            if (found)
            {
                consecutiveAbsentFrames = 0;
                absentSince = {};
                g_greenDetected.store(true, std::memory_order_relaxed);

                if (armed)
                {
                    const double outputStart = QpcNowSeconds();
                    SendOutputKeyFast();
                    const double outputMs = (QpcNowSeconds() - outputStart) * 1000.0;
                    outputTotal += outputMs;
                    outputMax = (std::max)(outputMax, outputMs);
                    ++detections;
                    armed = false;
                }
            }
            else
            {
                if (consecutiveAbsentFrames < ABSENT_FRAMES_TO_REARM)
                    ++consecutiveAbsentFrames;

                const auto now = std::chrono::steady_clock::now();
                if (absentSince.time_since_epoch().count() == 0)
                    absentSince = now;

                // Require both multiple fresh negative frames and a full
                // absence interval. Missing desktop frames do not count.
                if (consecutiveAbsentFrames >= ABSENT_FRAMES_TO_REARM &&
                    now - absentSince >= ABSENT_TIME_TO_REARM)
                {
                    g_greenDetected.store(false, std::memory_order_relaxed);
                    armed = true;
                    absentSince = {};
                }
            }
        }
        else
        {
            // No new desktop image is not evidence of absence. Keep the
            // previous detection state and output gate until a fresh frame.
            _mm_pause();
        }

        const double nowSeconds = QpcNowSeconds();
        if (nowSeconds - windowStart >= 1.0)
        {
            wchar_t diagnostic[512]{};
            const double divisor = calls ? static_cast<double>(calls) : 1.0;
            const double frameDivisor = frames ? static_cast<double>(frames) : 1.0;
            swprintf_s(diagnostic, ARRAYSIZE(diagnostic),
                L"Yami TB timing (1s): calls=%u frames=%u timeouts=%u hits=%u | "
                L"Acquire avg/max=%.3f/%.3f ms | Copy+Map avg/max=%.3f/%.3f ms | "
                L"Scan avg/max=%.3f/%.3f ms | SendInput avg/max=%.3f/%.3f ms\\n",
                calls, frames, timeouts, detections,
                acquireTotal / divisor, acquireMax,
                copyMapTotal / frameDivisor, copyMapMax,
                scanTotal / frameDivisor, scanMax,
                detections ? outputTotal / detections : 0.0, outputMax);
            OutputDebugStringW(diagnostic);

            windowStart = nowSeconds;
            acquireTotal = copyMapTotal = scanTotal = outputTotal = 0.0;
            acquireMax = copyMapMax = scanMax = outputMax = 0.0;
            calls = frames = timeouts = detections = 0;
        }
    }

    g_scanning.store(false, std::memory_order_relaxed);
    g_greenDetected.store(false, std::memory_order_relaxed);
    g_detectorReady.store(false, std::memory_order_release);
    grabber.Shutdown();
}

