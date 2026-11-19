# Yami v1.0.6 — current review status

This branch is **not a final release** and **must not be advertised as a working triggerbot** without validation on Windows and the actual target player.

## Confirmed source problems

1. The older repository Rust worker required an internal `held` bit in addition to `GetAsyncKeyState`; missing Raw Input edges could leave monitoring permanently idle. The branch relies on physical Hold state to enable capture.
2. Earlier defaults selected exact/nearly exact RGB colours with ±2 per-channel tolerance, easily missing antialiasing; fresh defaults now use ±14 and old settings are clamped as configured.
3. Failed calibration/foreground lookup and D3D initialization were previously silent; this branch now sets a non-active status.
4. `Yami-v1.0.6-Source-Handoff.zip` from the user's conversation is newer than this repository's Rust tree. The separate UI source-revision ZIP contains work not yet ported to GitHub.

## Build and runtime validation

A Windows Actions workflow is included and builds Rust source plus installer to downloadable artifacts. Passing compilation does not verify gameplay timing, input acceptance, screenshot capture, calibration overlay behaviour, or one-trigger-per-highlight under actual gameplay conditions.

## Requested UI refinements to port from newer source

- Compact shared Main/Settings shell and restrained violet accent
- Small title and minimal settings icon
- Keybind hint above input rows and no visible RGB tolerance control
- Compact colour hue ring with central screen-pick action and blinking caret in HEX field
- Glowing red-X calibration pointer with cleanup of the previous pointer
- Smaller exit confirmation, consistent borders and transition alignment

Do not merge this branch as a complete release until these changes have been reconciled with the newer source and Windows runtime testing has passed.
