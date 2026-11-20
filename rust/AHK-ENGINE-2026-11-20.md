# Native AHK-style engine revision — 2026-11-20

This revision replaces the active DXGI/compute-shader engine. UI layout, attached colour picker, native HEX editor, animations, tray and installer architecture remain intact. The only UI data changes are the requested radius12 default/range and a build identifier that prevents silently restoring an older executable.

## What the supplied AHK actually does

reference/user-original.ahk contains the supplied v1 script with transport escaping/HTML spaces decoded. It uses a5ms timer, a25x25 square around the mouse, per-pixel GDI reads, an early-exit green-dominance predicate, physical E activation, one active callback and a120ms repeat throttle. It can repeatedly click a continuously green region after120ms; it does not implement a disappearance latch. It also has script-specific F8/exit logging.

The Rust translation retains small square sampling, early exit, physical Hold eligibility, immediate click after a match and one scan owner. It deliberately follows the newer requirements for arbitrary selected RGB, optional Toggle plus Hold, two scan centres, and a genuine-clear event latch. It does not copy the green-only predicate,5ms initial timer wait,120ms repeat behaviour, global F8 binding or AHK exit callback. Ordinary UI/tray shutdown and engine diagnostics remain.

## Active production path

screen.rs owns one screen DC and a reusable top-down32-bit DIB. Each pass performs one small BitBlt(SRCCOPY|CAPTUREBLT|NOMIRRORBITMAP) and GdiFlush, then scans its BGRA bytes in Rust, top-to-bottom/left-to-right, returning on the first inclusive RGB-tolerance match. There are no hundreds of GetPixel calls, full-desktop CPU downloads, per-frame Vec allocations, shader dispatches, staging Map polls, or cached positive/negative detection results. The old GPU source is retained only as reference/legacy-gpu-click-repair.rs.txt and is no longer compiled. The Direct3D/windows crate dependency is removed.

Default radius12 means a25x25 square, including its corners as in AHK. Radius is configurable1..128; the bitmap grows only when needed and is reused, including monitor-edge clipping and the one-pixel eyedropper. Only the actual copied width/height is scanned, using the DIB stride; unused padding can never produce stale matches. All24-bit RGB values are accepted, with internal per-channel tolerance (default8; valid existing settings persist). Unlike the AHK green predicate, #00FF00 with a narrow tolerance does not mean every shade of green. Use the eyedropper on the actual displayed highlight when choosing a target.

Cursor mode obtains current physical cursor coordinates immediately before capture. Fixed Point uses the saved normalized point in the current intended foreground client, resolving current window origin/size. Both call the identical capture/match/activation/click code; only centre selection differs. Negative monitor origins and nearest-pixel rounding are supported. Keep Yami outside the region or minimise it: capture reads the displayed screen, and the engine rejects samples covered by its own GUI rather than click its colour swatch/text.

Every accepted observation comes from a new completed BitBlt+flush. No old bitmap/result is replayed on a timer timeout. GDI does not expose a unique presentation-frame number: two new screen reads may see the same still-presented image. Diagnostics therefore count fresh screen samples, not claimed new display frames. Rearming uses independent successful absent captures (default2); failures interrupt the clear count rather than count as absence. No display-refresh-latency claim follows from sampling frequency.

WAITING -> DETECTED -> CLICKED -> WAITING_FOR_CLEAR -> WAITING is explicit, with an inactive state outside activation. The event is claimed before SendInput; even failed dispatch cannot create retry bursts. A process-wide worker guard and per-shared-engine claim prevent concurrent scan dispatch. The Control lock serializes generation/enable/hold checks with one native tagged left-down/up submission. Physical Hold is checked again before submission; release, disable and configuration changes invalidate pending eligibility. There is no queued output or keyboard output. Partial insertion retries only mouse-up, and an outstanding release prevents a new pair.

Toggle changes master permission once per press and ignores repeat. Unbound Toggle starts enabled; unbinding Toggle restores the enabled Hold-only default. Hold remains mandatory. Mouse4/5, normal keyboard input and WASD share the existing validated input path; unrelated keys do not alter activation. Left/right/middle mouse, Windows/system keys, chords and duplicate bindings remain rejected. No process hiding, driver input, injection or security-tool bypass was added.

## Timing and sustainable pacing

There is no fake1ns timer. The first eligible scan starts immediately. After a completed pass, a high-resolution Windows waitable timer applies backpressure only if that pass took less than1ms, giving an approximately1000-samples/s ceiling when Windows/GDI permit. Slower captures add no pacing wait. An ordinary waitable timer/sleep fallback is available if high-resolution timers are unavailable. Inactive/invalid states use wakeable waits; error recovery backs off. There is no pacing wait,5ms sleep,120ms cooldown, statistics flush or output-thread handoff between a confirmed match and its click. Routine sample statistics are recorded afterward.

Rust Instant/high-resolution timestamps separately measure the copy+GdiFlush, pixel search, confirmed-match-to-SendInput entry, and time inside SendInput. The entry timestamp is taken immediately before the actual API call, not before a worker queue. On clean exit, %APPDATA%/Yami/engine-session.json contains count/last/min/max/mean microseconds for each stage, accepted-pair counts and errors. Unmeasured stages have null values, not a fake0ms result. Display/presentation and game-consumption latency are explicitly not measured.

## Verification actually run

cargo test --offline --locked --release --lib:19 passed,0 failed,1 ignored. These include actual off-screen Windows GDI BitBlt+GdiFlush and DIB reads through the production copy/matcher, arbitrary red/green/blue/white/black/purple/magenta/custom targets, BGRA ordering, tolerance boundaries, square corners, early exit, clipped stride, capture failure, fresh successive samples, both scan-centre choices through the same latch and recording input sink, genuine clear/reappearance, flicker suppression, failed-dispatch consumption, Toggle/Hold/thumb/WASD activation logic, stale generations, worker claims, legacy migration and normalized-coordinate calculations at100/125/150/200% scales. These coordinate tests are arithmetic checks, not visual Windows scaling tests.

An optimized5000-sample benchmark used an off-screen25x25 source bitmap, with624 near-colour misses requiring all three channel comparisons before the final matching pixel:

| Off-screen workload | Median µs | p95 µs | p99 µs | Maximum µs |
|---|---:|---:|---:|---:|
| BitBlt + GdiFlush, memory DC to DIB |2.3|3.6|4.6|87.0|
| Rust625-pixel search |0.7|1.0|1.2|20.5|

These are synthetic memory-DC and CPU measurements, NOT desktop capture or SendInput timings. No real click was emitted by these tests. Actual confirmed-match-to-SendInput latency remains unmeasured until a live run. Neither this benchmark nor a1ms pacing period establishes sub-1ms screen-to-click or game latency. OS scheduling spikes remain visible in the maximum values.

Both locked Windows release builds pass. The installer is built after the app and its embedded app bytes are checked. Imports are Windows system DLLs only; no Rust, AHK, Python, OpenCV, Direct3D11 runtime package, .NET or VC Redistributable installation is required by end users.

## Exactly what remains unverified

The user previously requested no desktop interaction and manual program testing. That restriction was retained. The controlled-window test remains opt-in/ignored and was NOT run. Actual screen-DC capture under the real cursor, actual calibrated-window capture, real keyboard/thumb-button activation, live WASD, actual SendInput reception by a Windows window, real match-to-SendInput timings, display-mode recovery, GUI/DPI operation and Roblox game consumption remain unverified in this build. No Yami/Roblox window or installer was launched. This is an engine repair candidate with tested native components, not a claim of verified Roblox operation.

The opt-in receiver test in verification.rs uses its own foreground receiver, restores cursor/focus when it still owns focus, stops the worker and releases only its injected held keys on normal completion or panic. It prints timings instead of overwriting the user's session report. It must only be run with explicit desktop-test authorization and no other Yami instance running. Recording-sink tests validate the native mouse-pair construction and decision path but do not substitute for actual Windows delivery.

For manual acceptance: exit the older tray instance; launch CURRENT/Yami.exe; assign Hold (Toggle is optional); choose a real displayed colour; move/minimise the GUI outside the scan area. Verify one click for sustained appearance, none after Hold release/Toggle disable, and rearming after absence in both modes. Repeat with WASD, Mouse4/5 and custom colours. Fixed Point must be calibrated inside the intended foreground client. After quitting, inspect engine-session.json and engine.log if anything fails. GDI can only sample content Windows makes capturable; no protected-capture restriction is bypassed.

## Source and build

Active modules: screen.rs (bulk capture/scanning), trigger.rs (activation/worker), event.rs (event latch), input.rs (bindings and measured native click), pacing.rs (between-pass backpressure), metrics.rs (timing summaries), capture.rs (centre/client mapping), config.rs (settings). engine_revision2 migrates the former default radius2 to12 and old default clear_ms8 to0 once; later explicit preferences persist. Default clear_frames remains2. UI appearance/animations are retained.

Build from rust/ so .cargo/config.toml supplies static CRT flags:

~~~powershell
cargo build --locked --release --bin Yami
cargo build --locked --release --features installer --bin Yami-Setup
~~~

Source requirements are in requirements/05-Native-AHK-engine-request.txt. Earlier repair documents describe superseded implementations.

Primary Windows API references:
- https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createdibsection
- https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-bitblt
- https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw
- https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps
