# Latest revision — 2026-11-20

The active [Rust application](rust/README.md) now uses fixed left-click output and a compact attached picker. Read the [current findings and verification limits](rust/CLICK-REPAIR-2026-11-20.md). The app and installer compile and ten headless checks pass; live Roblox and UI behaviour remain unverified. Older descriptions below are historical.

---

# Yami v1.0.6 — current Rust source

The active application is in [rust/](rust/README.md). See [latest repair findings and verification limits](rust/ENGINE-REPAIR-2026-11-19.md) before treating the build as gameplay verified. The latest UI uses a fixed 400 × 420 purple design.

The C++ implementation below is retained as historical source; its build instructions and results do not describe the current Rust release.

---

# Yami 1.1

Native Windows application with a small-region GPU detector and one synchronous
output owner. The approved dark/purple interface remains 520 × 630 design units.
Cursorless TB is the only new main-window control.

## Run

Run `Yami.exe` on Windows 10/11 x64 with a Direct3D 11 feature-level 11.0 GPU.
The C++ runtime is linked statically: no separate .NET or Visual C++ runtime is
needed. D3D11, DXGI, D3DCompiler_47, GDI+, XInput and the shell are OS components.
The executable/product name is Yami; its process remains visible in Task Manager.

**Hold is mandatory in both modes.** The status switch, tray Enable and Toggle
binding control master permission. They never substitute for Hold. Permission
starts enabled, but no output is allowed until the assigned Hold is physically
down. Releasing it invalidates in-flight results. Unassigned binds cannot activate.

- Click a binding to capture. Escape clears it; outside-click cancels. The new
  input must be released before it can activate monitoring.
- Toggle/Hold support keyboard, mouse and existing XInput bindings. Trigger
  supports keyboard/mouse down-up pairs. Prefer different Hold/output keys:
  synthetic key-up affects asynchronous key state if both use the same key.
- Fresh bindings are Not Set. Existing settings remain compatible at
  `%APPDATA%\YamiTB\settings.ini`.
- The existing swatch/HEX picker, hue ring, saturation/value square and eyedropper
  remain. Apply changes real shader parameters immediately; Escape cancels preview.
- Binding changes, applied colours and options save automatically. Save flushes
  again. The entire INI is replaced atomically.

## Modes and calibration

Cursorless TB off retains a circular ROI around the mouse, radius 50 px by
default. Cursorless TB on uses a calibrated point in the actual Roblox client
area, default radius 12 px. Production Yami never moves the mouse cursor.

Open Settings from the tray, the main-window context menu or **Ctrl+,**. Select
**Calibrate detection point**, then click the intended point inside a visible
Roblox viewport. The temporary crosshair disappears when finished; Escape cancels.
The point is saved as normalized client coordinates and resolved against current
physical client dimensions/position, including windowed and stretched modes.
Cursorless monitoring requires that Roblox client to be foreground and visible.

The calibration is not character tracking. Recalibrate after camera/zoom changes
as needed; no fixed point is guaranteed to intersect every character highlight.
No permanent gameplay overlay is created. Roblox identity is checked through
read-only process queries; no code is injected into it.

## Options

Numeric fields save when focus leaves them. Invalid values show a red outline
and do not replace the previous valid configuration. Radius edits the active mode.

| Setting | Default | Range |
|---|---:|---:|
| RGB tolerance, per channel | 2 | 0–16 |
| Cursor radius | 50 px | 1–128 px |
| Cursorless radius | 12 px | 1–128 px |
| Clear debounce | 8 ms | 0–100 ms |
| Clear confirmation | 2 frames | 2–16 frames |
| Minimum output interval | 0 ms | 0–1000 ms |

Both clear-frame confirmation and debounce must be satisfied before rearming.
Capture failures/timeouts cannot count as clear. Minimum interval is independent;
an appearance suppressed by it is consumed, never queued for delayed firing.

## Tray and startup

Minimise hides the main window/taskbar button while retaining the notification
icon. Double-click restores. Its menu has Open, Enable, Settings and Exit. Closing
the main window exits. One instance is permitted, Explorer restart restores the
icon, and normal shutdown removes it and signals workers asynchronously.

Settings offers Launch minimised to tray; `Yami.exe --tray` also starts hidden.
Start with Windows uses the ordinary current-user Run entry named Yami. Move the
executable before enabling startup; disable the checkbox before deleting it.

## Build and tests

Install Visual Studio 2026 / Build Tools, Desktop development with C++, MSVC v145
and a Windows 10/11 SDK. Open `Yami TB.slnx` and build Release/x64, or run:

```powershell
.\build.ps1
.\build.ps1 -Test
```

Outputs: `artifacts\Yami.exe`, `artifacts\YamiTests.exe`, and
`artifacts\test-results`. The test project is `tests\Tests.vcxproj`. Older Visual
Studio versions require retargeting the toolset. The HLSL is embedded in the EXE
and compiled with D3DCompile, so no external shader file is needed to run Yami.

The test harness opens a controlled test receiver, emulates F23 Hold and F24
output, and temporarily moves/restores the cursor for its cursor-mode case. Let
it finish without changing focus. It does not connect to a game.

## Source and verification

- `detection_core.h`: serialized states, tokens, debounce and stale-result rejection.
- `gpu_detector.h` / `ColorDetect.hlsl`: DXGI capture, GPU ROI matching, reused
  resources, nonblocking four-byte readback and timestamp queries.
- `engine.inc`: Hold validation, client coordinates, capture recovery and sole
  dispatcher. Background initialization avoids first-Hold device creation.
- `activation.inc`, `settings.inc`, `frontend.inc`: inputs, persistence and native UI.
- `reference/supplied-engine.inc.txt`: the supplied engine body for comparison.
- `tests/`, `validation/`: harness and recorded results.

See [BENCHMARKS.md](BENCHMARKS.md) and [ENGINE-NOTES.md](ENGINE-NOTES.md). Controlled
hardware tests passed; gameplay accuracy, perfect parries and sub-1 ms end-to-end
latency are not guaranteed. Roblox gameplay/interactive calibration were not
tested because no game viewport was open, and the referenced camera screenshots
were not attached. Resolution/DPI coordinate tests were synthetic; actual OS DPI
changes and rotated/HDR monitors were not manually exercised.
