# Yami v1.0.6 — Rust

Run **Yami-Setup.exe** to install, or **Yami.exe** directly for a portable launch.
Windows 10/11 x64 and a Direct3D 11 feature-level 11.0 GPU are required. Both EXEs
are standalone. Dependency inspection found only Windows system DLLs: no Rust,
Cargo, .NET or Visual C++ Redistributable installation is needed.

The compact main window contains status, colour and Settings. Settings contains
the three bindings and Fixed Point. Green Ready means waiting for Hold; red Active
means monitoring; grey Disabled means master permission was toggled off. Assign
Hold and Trigger before use. Toggle controls permission; it never replaces Hold.
Fresh bindings are Not set. Keyboard and mouse bindings are supported. Escape
clears a binding during capture. Clicking outside cancels capture.

The hue wheel, saturation/value square, HEX editor, eyedropper and Apply are
functional. Click HEX to type; Ctrl+A, Ctrl+V and Backspace are supported. Invalid
HEX cannot apply. Settings save automatically to `%APPDATA%\Yami\settings.json`.
Existing `%APPDATA%\YamiTB\settings.ini` keyboard/mouse binds, colour and calibration
are imported when no new JSON exists. Existing files are not deleted.

Fixed Point offers Calibrate when enabled. Select a point in the intended
application's client area; Escape cancels. Coordinates are saved normalized to
the viewport. Its process name and current client bounds are used on later runs.
The calibrated application must be foreground and visible. There is no permanent
overlay, cursor movement or character tracking. Recalibrate when camera position
changes. Minimise hides to the tray; double-click restores. The tray menu has
Open and Exit. Normal close exits and removes the icon. The process remains visible.

Advanced detection settings are deliberately kept out of the minimalist UI.
Edit the JSON while Yami is closed: `radius` (1–128 physical px, default 12),
`tolerance` (0–16 per RGB channel, default 2), `clear_ms` (0–100, default 8),
and `clear_frames` (2–16, default 2). Each continuous match is consumed once.
Rearming needs confirmed absence, not a large fixed cooldown. Hold release,
disable and configuration changes invalidate pending results. Output has one
worker owner and synchronous down/up insertion. Prefer different Hold/output keys.

## Installation and removal

The separate installer targets `%LOCALAPPDATA%\Yami`, asks before replacement,
verifies staged application/uninstaller bytes with SHA-256, backs up owned files
before replacement and rolls them back on replacement/registration failure.
It registers Yami 1.0.6 in Windows Installed Apps, then launches the installed EXE.
Run the installer outside the destination directory. No elevation is requested.
An active installation must close cleanly; it is never forcibly terminated.

Installed Apps or `Uninstall.exe` removes only Yami.exe, Uninstall.exe and Yami's
uninstall registration. Configuration is preserved. Unrelated files are not
recursively deleted. A temporary uninstall helper can remain in Windows Temp
until normal Temp cleanup; it does not run persistently.

## Build from source

Developers need Rust x64 MSVC, Visual C++ Build Tools and a Windows SDK. End users
do not. From this directory, use `build.ps1`, or:

```powershell
cargo build --release --bin Yami
cargo build --release --features installer --bin Yami-Setup
```

Build the app first: Setup embeds `target/release/Yami.exe`. Outputs are in
`target/release`. Cargo.lock is included. GDI+ and native Win32 provide the UI;
DXGI/D3D11 and the embedded HLSL provide GPU-only circular-ROI matching. The only
readback is a four-byte result; resources are reused, and UI work is separate.

Modules: `ui`/`render`, `config`, `capture`, `gpu`, `input`, `trigger`, `tray`,
`install`. No C++ engine, external engine DLL or AutoHotkey runtime is used.

## Verification scope

Both release executables compiled successfully, and DLL imports were checked.
Static review corrected class-name ownership, installer self-overwrite prevention,
binding capture, state invalidation, settings migration and popup DPI bounds.
No runtime, gameplay, installer execution, pixel-comparison or OS scaling tests
were run, as requested. No claim of zero defects, guaranteed duplicate-free
gameplay, perfect parries or sub-1 ms latency is made. The referenced original
AHK file was unavailable; its behaviour was taken from the supplied description.
