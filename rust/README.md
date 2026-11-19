# Yami v1.0.6 — Rust repair candidate

Current source is in this directory. The latest changes and evidence are in [ENGINE-REPAIR-2026-11-19.md](ENGINE-REPAIR-2026-11-19.md). The user reported the preceding build still failed; this build has not been run in Roblox or visually checked. Compilation and state-machine checks do not prove the game pipeline works.

## Run and configure

Run Yami.exe directly or use Yami-Setup.exe for a per-user installation. Windows 10/11 x64 and a Direct3D 11 feature-level 11.0 GPU are required. The executables use Windows system libraries with the CRT linked statically; end users do not need Rust, .NET or a separate Visual C++ runtime.

Exit an older Yami tray instance first. The new build warns if a different version is already running. The application and visible process are named Yami.

Main and Settings use the same 400x420 logical outer dimensions and the latest purple palette. Settings contains Toggle, mandatory Hold, Trigger, RGB tolerance, ROI radius and Fixed Point/calibration. Set distinct Hold and Trigger keys. Toggle changes master permission; it does not replace Hold. Fresh bindings are Not set; a fresh colour is #FFFFFF. Existing settings are preserved. Escape clears a captured binding; an outside click cancels it. Unassigned bindings cannot activate.

Click the swatch/HEX button for the hue wheel, saturation/value square and HEX editor. HEX supports Ctrl+A/Ctrl+V/Backspace and rejects invalid values. Apply commits the selected value. Eyedropper temporarily hides all Yami windows; left-click selects and Escape cancels. The preview remains offset from the sampled pixel.

Fixed Point calibration hides the application and uses a small red X, left-click confirm/Escape cancel, then restores Yami. Select within the intended application's client area. The saved normalized position follows that application's current foreground client bounds. Cursor mode uses physical mouse coordinates. Both modes require Hold. Neither mode tracks a ball or moves the cursor. Radius is 1..128 physical pixels, tolerance 0..16 per channel.

Settings save automatically to %APPDATA%/Yami/settings.json. When no JSON exists, legacy %APPDATA%/YamiTB/settings.ini bindings/colour/calibration are imported. Minimise hides to the notification area; double-click restores; the tray offers Open and Exit. The main X requests exit confirmation. Clean exit saves settings and removes the tray icon. The process remains visible to Windows and security tools.

## Detection and diagnostic limits

One DXGI/D3D11 worker copies only a bounded ROI, performs arbitrary RGB comparison on the GPU and reads a four-byte result. The explicit event latch consumes a continuous highlight once and rearms after qualified clear observations. Successful timeouts can confirm an identical cached ROI is unchanged; capture errors cannot. Hold release, configuration generations and point/foreground changes reject stale output.

Ordinary tagged SendInput sends one keyboard/mouse down/up pair. A Windows accepted pair is not proof of game consumption. No process hiding, injection, drivers or input restriction bypass is included. No measured gameplay latency or working-game claim is made.

After a clean exit, inspect %APPDATA%/Yami/engine-session.json for captured frames, matching observations, accepted input pairs, iteration-to-input timing and last error. Capture/input errors also go to engine.log. Timing excludes game processing and display delay. Zero timing means no accepted pair was measured. The complete manual acceptance list is in the repair notes.

## Install and remove

Setup installs to %LOCALAPPDATA%/Yami, confirms replacement, verifies staged bytes with SHA-256, backs up owned files, rolls back on replacement/registration failure, registers Windows Installed Apps and launches the installed EXE. Run Setup outside the destination directory. Existing instances must close cleanly; they are never killed. Installed Apps/Uninstall.exe removes only owned binaries and registration, preserving configuration. A temporary uninstaller helper can remain until Windows Temp cleanup. Installation and removal were not executed for this revision.

## Build

Developers need Rust x64 MSVC, Visual C++ Build Tools and a Windows SDK. Run build.ps1 from this directory or:

~~~powershell
cargo build --locked --release --bin Yami
cargo build --locked --release --features installer --bin Yami-Setup
~~~

Build the app FIRST: Setup embeds target/release/Yami.exe. Run Cargo from this directory so .cargo/config.toml supplies static-CRT flags. Cargo.lock is included. GDI+/Win32 draw the UI; HLSL is embedded. No Python, OpenCV or external engine runtime is required.

Noninteractive event-state checks: cargo test --lib event::tests. Desktop integration checks are ignored/opt-in and were not run for this revision. Do not run those on the user's desktop without authorization.

The full reference review, current limits and manual checks are in ENGINE-REPAIR-2026-11-19.md. Historical REPAIR-NOTES.md and HANDOFF.txt describe earlier work; the newest purple specification overrides the older red screenshot. The latest supplied request ends abruptly at Window positioning / Fix; earlier positioning requirements were retained.
