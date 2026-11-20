# Yami v1.0.6 — native AHK-style engine

Current source findings, benchmark and verification limits: [AHK-ENGINE-2026-11-20.md](AHK-ENGINE-2026-11-20.md). The active engine is screen.rs, using a reusable Windows GDI bitmap and early-exit Rust RGB scanning. The old DXGI/shader code is historical reference only.

Run Yami.exe directly, or Yami-Setup.exe for the existing per-user installation. Windows10/11 x64 is required. Only Windows system DLLs are imported; no Rust, AHK, Python, OpenCV, .NET or separate VC runtime is needed by end users.

Exit the older Yami tray instance first. Set Hold (keyboard or Mouse4/5); Toggle is optional and defaults enabled when unbound. Output is always one left-down/up pair. Hold must be physically pressed. Unrelated WASD keys do not change activation. The existing binding validation, attached compact picker, native HEX field, animations, tray and installer are retained.

Default radius12 scans a25x25 square, matching the AHK region. Radius supports1..128. Any selected24-bit RGB is accepted with internal tolerance (default8/channel). This is selected-colour matching, not the AHK script's broad green classifier: choose the actual highlight with the eyedropper. Cursor mode follows the current physical cursor; Fixed Point resolves saved normalized coordinates in the intended foreground client's current bounds. Both use the same scan/click logic. Move or minimise Yami outside the scan area so it does not cover the pixels you want to detect.

The first eligible sample runs immediately. Independent successful captures drive an explicit one-click/qualified-clear latch. There is no120ms repeat cooldown or5ms initial timer delay. A1ms between-pass backpressure budget avoids an unbounded polling loop; it is not a reaction-time promise or nanosecond scheduler. No cached match is processed as a new capture. GDI samples current displayed pixels but exposes no unique presentation-frame ID.

Settings save atomically to %APPDATA%/Yami/settings.json. Valid existing bindings/colour/calibration persist. The earlier default radius2 migrates to12 once, and default clear_ms8 to0; clear_frames defaults2. Output-key fields remain removed. Fresh bindings are unset, fresh colour is white. Existing per-user install/uninstall behaviour is retained.

Build from this directory (Rust x64 MSVC, Visual C++ Build Tools, Windows SDK):

~~~powershell
cargo build --locked --release --bin Yami
cargo build --locked --release --features installer --bin Yami-Setup
~~~

Or run build.ps1. Build the app FIRST because Setup embeds target/release/Yami.exe. .cargo/config.toml provides static-CRT settings.

Verification: cargo test --release --lib passes19 noninteractive checks, including real off-screen GDI copies and the production Rust scanner, event/click-pair recording, activation logic, coordinates and persistence serialization. The controlled actual desktop/click test is ignored and was not run, preserving the user's manual-testing restriction. No app, game or installer was launched. Actual desktop pixels, real OS click reception and Roblox functionality remain unverified. No sub-1ms game/screen-to-click claim is made.

After a clean exit, %APPDATA%/Yami/engine-session.json separately reports capture, matching, confirmed-match-to-SendInput entry and API-call timing. Unmeasured stages are null. Windows accepting a pair is not proof of game consumption. Errors go to engine.log. Detailed coverage, off-screen benchmark values, remaining checks and the supplied AHK reference are in the linked notes and HANDOFF.txt.
