# Changelog

## 1.1 — Engine rework

- Preserved the approved main window, theme, picker, keybindings and tray.
- Added Cursorless TB, normalized Roblox client calibration and minimal settings.
- Replaced full-frame CPU staging/scanning with a D3D11 ROI compute shader and
  four-byte nonblocking result readback.
- Added tolerance, per-mode radius, clear time/frame confirmation and independent
  optional minimum output interval.
- Added a serialized single-trigger state machine, mandatory physical Hold,
  activation/configuration tokens and stale-completion rejection.
- Added capture/device-loss recovery, monitor selection and rotation transforms.
- Corrected mouse output pairing and filtered self-generated raw activation input.
- Fixed picker initialization validation, popup Escape/Enter and stale solution
  paths. Added Ctrl+, and context-menu access to Settings.
- Added deterministic A–H suites, concurrency tests, GPU colour validation, real
  desktop capture/input integration and measured latency reports.

## 1.0 — UI redesign

- Introduced the native dark/purple UI, readable binds, Not Set defaults, picker,
  atomic settings saves and tray integration.
- Retained the supplied engine; runtime tests were skipped for that release at
  the user's request. Version 1.1 supersedes that engine and includes tests.
