# Yami v1.0.6 — left-click repair revision

Current changes and verification limits: [CLICK-REPAIR-2026-11-20.md](CLICK-REPAIR-2026-11-20.md). The preceding version was reported nonfunctional. This build passes headless checks and compiles; live Roblox and visual operation have not been verified.

Run Yami.exe directly or Yami-Setup.exe to install per-user. Exit any old tray instance first. Windows10/11 x64 with Direct3D11 feature level11.0 is required. Only Windows system DLLs are imported; end users do not need Rust, Python, OpenCV, .NET or a separate Visual C++ runtime.

Settings contains Toggle, mandatory Hold, Radius (1..8 physical pixels, default2), Fixed Point and calibration. Output is always LEFT MOUSE CLICK. There is no Trigger/output key or visible RGB tolerance field. Fresh bindings are Not set and colour is #FFFFFF. Default internal RGB tolerance is8/channel; valid existing values persist. Binding capture commits on release. Mouse4/5 and individual keyboard keys are allowed; left/right/middle mouse, Windows/system keys, chords and duplicate Toggle/Hold bindings are rejected. Escape clears, outside click cancels. Unrelated keyboard activity does not disarm detection.

The compact picker opens as a child panel attached to Main, expanding the same draggable HWND. HEX is a native text field with caret, selection, navigation, Delete/Backspace and paste. Apply validates and saves. The eyedropper hides the application, samples under the cursor, and accepts left-click/Escape. Fixed Point calibration hides Yami, displays a small thin alpha red X, stores normalized client coordinates and restores the cursor/UI. The calibrated target must be foreground; ordinary left clicks go to the current cursor location. The detector does not move your cursor or bypass target input restrictions.

Settings automatically save to %APPDATA%/Yami/settings.json; old output fields are ignored, invalid/duplicate old bindings sanitized, and oversized legacy radius reset to2. Legacy YamiTB/settings.ini migration remains. Minimise hides to tray; double-click restores; tray Open/Exit remain. Main X asks for confirmation. Both base pages are400x420 logical pixels; the attached picker temporarily adds248 logical width. Always-on-top, launch centring, owned confirmation and the per-user installer are retained.

Build from THIS directory so .cargo/config.toml supplies static-CRT settings:

~~~powershell
cargo build --locked --release --bin Yami
cargo build --locked --release --features installer --bin Yami-Setup
~~~

Or run build.ps1. Build the app first: Setup embeds target/release/Yami.exe. Developers need Rust x64 MSVC, Visual C++ Build Tools and a Windows SDK. Setup installs to %LOCALAPPDATA%/Yami, confirms replacement, verifies staged bytes, registers Installed Apps and launches Yami. Uninstallation preserves settings. The installer has not been executed for this revision.

Noninteractive verification: cargo test --lib. Ten checks pass, including the actual HLSL on D3D11 WARP, colour/ROI/tolerance, event-to-click-pair construction, WASD isolation, invalid binding policy and JSON migration. Two desktop-interaction tests are ignored and were not run. These checks do not establish actual OS/game click reception, visual correctness or display-to-action latency. No1ms guarantee is made.

After manual use and clean exit, inspect %APPDATA%/Yami/engine-session.json and engine.log if needed. Timing covers acquisition/GPU work through accepted SendInput, not game consumption or presentation. Complete pending manual checks and detailed source findings are in the linked repair notes. HANDOFF.txt includes this update and historical context; the latest request is requirements/04-Final-polish-left-click-request.txt.
