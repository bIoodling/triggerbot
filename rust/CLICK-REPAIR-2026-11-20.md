# Yami 1.0.6 — left-click engine and final polish revision

This revision implements requirements/04-Final-polish-left-click-request.txt. The preceding build was reported nonfunctional. This build has not been launched or tested in Roblox and is not described as gameplay verified. The user's restriction against desktop interaction/manual testing was retained.

## Engine changes and source findings

The previous code discarded cursor-mode results whenever the pointer moved during GPU processing. At normal pointer speeds this can repeatedly discard valid matches. Cursor position is now sampled after the bounded acquisition wait, directly before the ROI copy; ordinary subsequent motion does not invalidate that observation. Fixed Point still validates its normalized client point and intended foreground window before output. Both modes reject output over Yami's own interface.

The old output-key field and its activation gate are removed. The action is always one tagged MOUSEEVENTF_LEFTDOWN/MOUSEEVENTF_LEFTUP pair. A previously saved output field is ignored on load and omitted on save. Default radius is 2 physical pixels (a 5x5 bounding square, circular mask); valid radius is 1..8, with older oversized values migrated to 2. GPU ROI storage is 17x17 instead of 257x257. Default per-channel RGB tolerance is 8; an existing valid internal tolerance is preserved, but the tolerance editor is gone. Any 24-bit RGB target remains valid, including black and white.

The same actual D3D11 compute matcher serves both modes. DXGI acquisition uses a nonzero 2ms bounded wait during detection. This is not a promised reaction time; presentation cadence, scheduling and GPU/readback costs remain. GPU resources are reused; only a four-byte match result is downloaded. Capture failures discard invalid resources and interrupt clear evidence; subsequent work recreates them. Confirmed unchanged observations at the identical point/radius can complete the absence interval without counting as new captured frames. The explicit event latch still consumes an appearance before dispatch, never retries a failed click down, and rearms only after qualified absence. A partial input insertion only retries the release. Hold/enable/config generation checks remain immediately before dispatch. An outstanding failed release prevents a new click pair.

Ordinary keyboard input is registered using Raw Input without NOLEGACY or a global keyboard suppression hook. Only the configured Toggle/Hold keys affect activation. WASD and other unrelated keys do not change Hold/master state. Temporary selection hooks remain limited to calibration/eyedropper left selection and Escape; unrelated input passes through.

No evidence proves the exact cause of the user's failure inside Roblox, or that a target game consumes the ordinary Windows input pair. These source fixes address demonstrable failure paths and the user's new left-click-only behaviour. No driver, DLL injection, process hiding or input restriction bypass was added.

## Binding and configuration changes

Only Toggle and Hold are shown. Key capture commits on release so another simultaneous key can reject a chord. Mouse 4 and Mouse 5 remain supported and named clearly. Left/Right/Middle mouse, Windows/App/system keys and unsupported VKs are rejected. Toggle/Hold cannot duplicate one another. Generic legacy Shift/Ctrl/Alt values normalize to their left-side variants before duplicate validation. Escape clears; outside click/focus loss cancels without replacing the prior binding. All mouse-button flags in one Raw Input packet are processed, including simultaneous thumb-button edges. Configuration sanitization rejects invalid old bindings. Normal keyboard events continue reaching their normal target.

Settings remain at %APPDATA%/Yami/settings.json, atomically saved. Existing valid colour, tolerance and calibration persist. Fresh defaults are white, no bindings, Cursor mode and radius2. Install paths and the existing per-user installer are retained. The new build identity warns if an obsolete Yami instance would otherwise be silently restored.

## UI changes

The base Main and Settings panels are both 400x420 logical pixels. The picker is a WS_CHILD panel inside the same main HWND, 76% of its old size (228x295 logical). Its parent expands horizontally by248 logical pixels over170ms; both panels move together. It can dock on either side when space requires, uses the same DPI scaling, and clamps to the monitor work area. There is no independent floating picker window. DPI changes rebuild the native text font as needed.

HEX now uses a real Win32 EDIT control: system blinking caret, selection highlight, mouse positioning, Home/End/arrows/Shift selection, Backspace/Delete, Ctrl+A and clipboard paste. Invalid HEX cannot apply; programmatic wheel updates synchronize the editor without losing selected hue. The key-unbind hint sits directly below Keybinds and above the two aligned binding rows. The Back background strip and visible RGB tolerance field are removed. Radius is aligned to the binding inputs.

Calibration is one 18x18 alpha window containing a thin red X, not a solid fullscreen surface. Its bitmap is transparent outside the strokes. Marker position uses physical cursor coordinates. Cursor display-counter changes are balanced on confirm/cancel/error/shutdown. Yami and its child panel hide before selecting; the hide is flushed through DWM before sampling, and initial foreground settling does not immediately cancel selection. The existing click/Escape cleanup and normalized point storage are retained. A stylus-shaped vector icon replaces the pickaxe shape, and the gear uses a thinner continuous outline.

Launch fade remains200ms; page transitions160ms; hover/pressed and toggle transitions150ms; picker expansion/fade170ms; visible-app exit fade150ms. UI timers run only while these transitions are active, apart from the existing low-frequency status timer. Panel geometry changes only during expansion or DPI updates. Windows reduced-motion settings are respected. All animation and rendering remain separate from detection.

## Verification actually performed

cargo test --offline --locked --lib: 10 passed, 0 failed, 2 ignored.

The windowless D3D11 WARP check uses the same production Matcher and HLSL, with actual BGRA textures and staging-buffer readback. It verifies red, green, blue, white, black, purple, magenta and custom HEX; channel ordering; tolerance inclusivity; circular ROI exclusion; and repeated appearance/clear sequences through the real event latch into the production mouse-pair construction/dispatch function with a recording sink. WARP is software D3D11, not a live desktop capture or a hardware latency benchmark.

Other checks cover exactly one tagged left-button pair, partial insertion release recovery, failed dispatch consumption, continuous highlight suppression, qualifying clear, interrupted absence, disarm, invalid/duplicate/chord policy, WASD isolation, legacy output removal, binding normalization and RGB JSON/HEX roundtrips.

Both locked release binaries build without warnings. Setup was built after Yami.exe; its embedded app bytes are checked against that exact app. Import inspection finds Windows system DLLs only, including the new dwmapi.dll for hide-composition flushing. No Rust, Python, OpenCV, .NET or separate VC runtime is needed by end users.

Two interactive tests remain opt-in/ignored: actual desktop capture and actual receiver input in both modes. The latter now observes left mouse down/up and holds WASD during the sequence. Neither was run on the user's desktop. No native HEX editing, drag/animation, calibration, eyedropper, install/uninstall, tray, DPI visual or Roblox gameplay run occurred. Actual OS click reception and game consumption remain unverified for this revision. Do not report a guaranteed 1ms or sub-1ms result.

## Manual checks still needed

Exit the older tray instance first. Run this build or its installer; set distinct allowed Toggle/Hold keys. Check cursor and calibrated foreground Fixed Point with real red/green/blue/white/custom pixels, continuous appearance, absence/reappearance and immediate Hold release. Verify an actual left click and continue while WASD are held. Check Mouse4/5, invalid keys/chords/duplicate rejection, Escape and outside cancellation. Verify field caret, mouse selection, arrows/Home/End/Delete/Backspace/Ctrl+V, attached-panel dragging, every animation, left/right docking, DPI100/125/150/200%, calibration confirm/cancel and cursor cleanup, eyedropper, persistence, tray restore and clean exit.

After a clean exit, %APPDATA%/Yami/engine-session.json contains current configuration, fresh-frame/observation/match counts, Windows-accepted mouse-pair count, last/max iteration-to-SendInput microseconds and last error; engine.log records capture/input errors. Windows acceptance is not proof of a Roblox action. A zero timing is no measured accepted pair, not zero latency.

The seven supplied reference images are in reference/final-polish/1.png through7.png. The repeated explicit request for a small red X takes precedence over the plus-shaped image4. Earlier red/purple repair documents are historical and do not describe the new fixed output or current picker architecture.
