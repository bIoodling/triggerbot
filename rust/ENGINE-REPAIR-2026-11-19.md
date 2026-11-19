# Yami 1.0.6: latest repair and verification boundary

The user still reports that the preceding build does not work. This release is a repair candidate for manual verification, not a claim that Blade Ball now works. No game or desktop application was launched during this revision.

## Findings from the actual source

- Earlier activation required both an async Hold state and a Raw Input-held flag; this blocked valid key-state paths. The previous local repair removed the extra flag gate. That repair had not reached public main; this update includes it.
- The Rust worker required two absent results and an elapsed clear interval but skipped every DXGI timeout. A scene that disappears once and remains static could therefore leave the latch waiting indefinitely. The current capture path retains only its copied GPU ROI, distinguishes new frames from confirmed unchanged observations, and re-evaluates the exact same point/radius on a successful timeout. Current colour/tolerance are uploaded on every evaluation. A different point/radius cannot reuse incompatible data. Capture errors discard resources and invalidate clear evidence, rather than count as absence.
- The original opaque fullscreen calibration paint was already removed locally. It cleared the keyed surface back to the main background on redraw. This release retains one small alpha window and changes its image to a premultiplied red X with halo, with balanced cursor hide/restore and cleanup on overlay failure.
- A previously running old executable could silently be restored by single-instance handling when a new portable build was started. The current build identifies itself with a window property and warns if the existing instance is a different build. It never kills a process.
- No evidence establishes the exact cause of the user's live Roblox failure. Target colour/ROI, calibration/foreground gating, output configuration, target integrity level and game input consumption remain possible boundaries. Do not represent these as confirmed root causes.

## Pipeline and changes

Capture uses DXGI Desktop Duplication on the physical point's monitor. The API exposes a desktop surface; Yami copies only the circular region's bounding rectangle to a reusable maximum 257x257 GPU texture. It does not download a full desktop to CPU. Rotation and monitor offsets are translated before the ROI copy. One compute dispatch compares all three RGB channels against the configured 24-bit value with an inclusive per-channel tolerance. BGRA_UNORM texture loads supply logical RGB to HLSL. The readback is four bytes. The same Detector serves Cursor, normalized client-coordinate Fixed Point and one-pixel eyedropper requests.

The duplication lease is released before the next acquisition, including transitions to eyedropper work, and by Drop. Lost duplication/device resources are rebuilt by the worker. No retry or repeated-click workaround was added. IDLE -> ARMED -> FIRED -> WAITING_FOR_CLEAR -> ARMED is explicit in event.rs. Consuming an appearance precedes output, even if dispatch fails. Generation, Hold, enable/suspension and physical-point/foreground validation discard pending results before output. Brief absence and errors do not qualify as a clear event.

The app keeps ordinary Windows SendInput with a tagged keyboard or mouse down/up pair. It does not implement injection, driver input, anti-cheat bypass, process hiding or security-tool evasion. SendInput's count proves insertion into the Windows input stream, not acceptance by Roblox. Hold and Trigger should be different keys because injecting an up event for a physically held key can interfere with key state.

Main and Settings share exactly 400x420 logical outer dimensions, including when Fixed Point is enabled. Purple palette follows the latest complete specification. Radius (1..128 physical pixels) and RGB tolerance (0..16/channel) are editable and automatically saved in Settings. All earlier user binding, colour, calibration and configuration migration are retained. A fresh configuration is white (#FFFFFF) and three unassigned bindings, per the later defaults instruction. Existing settings are never reset to defaults.

## Reference actually studied

FenixAT/BladeBall main: BladeBallAutoNoGUI.py (blob 5c89848d37f0b0b672df2225bf683461ece78bea), BladeballAutoWithGUI.py (d84145cf27f34e21617c0b021503fa06d95711fe), README and MIT LICENSE, copyright 2025 Fenix.

The no-GUI version grabs monitor 1 with mss, converts BGR/BGRA to HSV, thresholds two red hue bands, filters contours by bounding-box area >=500 and circularity >=0.7, and checks contour centres against a hardcoded 1920x1080 centre/radius250. It sends F down/up on every matching loop then sleeps 1ms. The GUI version grabs a 500x500 centre ROI, adds 3x3 morphology, filters area >=300 and circularity >=0.6, sends F each match and sleeps 5ms; inactive detection sleeps100ms. Both reuse an mss session but allocate image/mask/contour data per frame. Neither tracks a unique target across frames, waits for a genuine clear, or prevents repeat output during a sustained match. It is not evidence of exactly-once behaviour or measured latency. Yami adapts the small-region/current-frame idea, uses arbitrary RGB instead of red HSV/ball shape, and preserves its own event latch. No source was copied or translated; no Python/OpenCV dependencies were added. Attribution and the reference licence are included for clarity.

Primary sources:
- https://github.com/FenixAT/BladeBall
- https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe
- https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-releaseframe
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput

## Verification performed in this revision

- cargo check --offline --lib: passed, no warnings.
- cargo test --offline --lib event::tests: 3 passed, no desktop access or input dispatch. These check continuous highlights, qualified clear/reappearance, flicker, interrupted absence, disarm and failed-dispatch consumption.
- Release app and installer builds passed. Installer embeds the freshly built app.
- Interactive Cargo checks remain opt-in/ignored. They were not run for this revision. Earlier repair notes document controlled capture-to-input checks on an older build; those results do not validate this changed release or Roblox.
- No launch, gameplay, installation/uninstallation, eyedropper, calibration or 100/125/150/200% visual scaling check was performed in this revision, following the user's manual-check instruction.

No actual latency numbers are asserted. On clean exit, %APPDATA%/Yami/engine-session.json records fresh_frames, valid_observations, matching_observations, windows_accepted_pairs, last/max iteration-to-SendInput microseconds, selected parameters and last error. The interval begins before acquisition and ends after Windows accepts a pair; it includes acquisition waiting/GPU work and excludes game consumption/display delay. engine.log contains capture/input errors. Zero timing means no accepted pair was measured. A live manual run is still needed to establish complete capture-to-game behaviour.

## Remaining acceptance checks for the owner

Exit the older tray instance first, then run this build or its installer. Set distinct Hold/Trigger keys and a real screen colour, select Cursor or recalibrate Fixed Point inside the intended foreground client. Hold through one highlight, clear it, and show it again; expect one action for each appearance and none after release. Repeat both modes, arbitrary colours, tolerance bounds, display changes, restart persistence, tray restore, eyedropper/Escape, calibration cleanup, close confirmation and DPI scales. Check the session report after a clean exit if anything fails. Do not solve game rejection by evasion or repeated input bursts.

The newest pasted request terminates after Window positioning / Fix. No missing continuation was available; earlier centring, DPI/work-area clamping, topmost ownership, installer positioning and fade requirements were retained.
