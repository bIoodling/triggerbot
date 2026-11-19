# Yami v1.0.6 — repaired build

## Included changes

- Native topmost main/installer windows and owned popups.
- Fresh-process centring on the cursor's monitor work area, before showing the
  final-sized window. The installer rejects dragging and resizing.
- A 200 ms elapsed-time ease-out opacity fade, skipped when Windows animation
  preferences disable it. No resizing or repositioning occurs during the fade.
- Fresh defaults: unassigned bindings, #FFFFFF, Fixed Point off. Existing JSON
  and legacy INI values remain preserved. Fresh settings are saved on first run.
- Compact owned Close Yami? confirmation for the main X, with Cancel/Exit,
  Tab/arrow navigation, Enter/Space and Escape. Tray Exit uses clean shutdown.
- Shutdown suspends processing, invalidates results, saves configuration, removes
  the tray icon and attempts cleanup of any partially inserted output release.
- Calibration and eyedropper hide the entire GUI. A small persistent alpha
  marker replaces the fullscreen paint surface. Temporary click/Escape callbacks
  are unregistered when selection finishes or is cancelled.
- The eyedropper requests the actual pixel from the existing DXGI capture worker,
  shows its colour beside the cursor, and synchronizes/persists the chosen HEX,
  HSV controls and detector parameters. There is no second capture/output worker.
- RGB tolerance is editable in Settings beside the existing instruction hint.

## Confirmed failure mechanisms

The old activation path required both a Raw Input-maintained Hold flag and the
current Windows key state. A controlled test reproduced zero output in both modes
when Windows correctly reported Hold down but the Raw Input flag was absent.
The repaired path shares one edge handler between Raw Input and a current-state
fallback, with release/configuration guards and one output owner. The same test
then produced the expected output in both modes.

Settings interactions could also leave the suspended flag latched. Ordinary
Settings navigation no longer acts as a permanent inhibition state. GPU/capture
errors are no longer silently discarded; error transitions go to engine.log and
the Disabled status rather than implying successful initialization.

The calibration source filled a fullscreen surface with its transparent key,
then Canvas::new cleared that surface to opaque near-black on every mouse redraw.
That was a concrete path capable of causing the reported black flashes. It has
been removed. The replacement uses a 32x32 premultiplied-alpha marker, with no
fullscreen canvas, colour-key clear, or fullscreen mouse invalidation.

## Verification and limits

Both release executables compiled successfully. Before desktop interaction was
stopped, Cargo's opt-in library checks verified a real captured desktop pixel and
the production capture/GPU/state/SendInput path to a controlled receiving window.
Eight received down/up pairs were checked across Cursor/Fixed Point, genuine
reappearance, release, Toggle, tolerance boundaries and red/green/blue/white.
The DXGI eyedropper pixel also matched the actual controlled screen colour.

The requested desktop UI inspection was stopped. The replacement calibration
marker, hide/restore interactions, centring, animations, exit-dialog appearance
and installer execution have NOT been visually/runtime verified. They require
your manual testing. No claim is made that every UI bug is gone or that black
flicker is empirically eliminated on your display. Gameplay was not tested.
There are no guaranteed parries or sub-1 ms end-to-end claims.

The two opt-in integration checks are standard cfg(test) library tests, not a
separate distributed tester application. No test executable is included here.
