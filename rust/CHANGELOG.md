# 2026-11-20 — left-click engine and attached picker

- Fixed left-click output; removed output binding/configuration and obsolete activation gate.
- Fresh cursor position at capture, removed movement-induced result starvation, small5x5 default ROI and17x17 reusable storage.
- Added binding allowlist, release-based chord rejection, duplicate prevention and legacy cleanup; preserved normal WASD input.
- Compact attached picker, native HEX editor, aligned two-bind Settings, hidden tolerance editor, thin red X and simpler icons.
- Added page/hover/pressed/toggle/picker/close transitions and DPI text sizing.
- Ten windowless checks pass and both release binaries compile. Live desktop/game and visual checks remain unverified.

--- Earlier changes ---

# Latest revision — 2026-11-19

- Purple palette; Main/Settings fixed at 400x420, with radius/tolerance controls and hover feedback.
- Re-evaluate identical GPU ROI on confirmed unchanged DXGI timeout; qualified clear can now rearm without requiring unrelated screen animation. Count fresh frames separately.
- Explicit IDLE/ARMED/FIRED/WAITING_FOR_CLEAR latch; retain generation/Hold guards and add point/foreground validation before dispatch.
- Red X alpha calibration marker, balanced cursor cleanup, overlay error cleanup; retain hidden-GUI eyedropper and normalized calibration.
- Warn about old running versions rather than silently restore an obsolete build.
- Save bounded session counters and iteration-to-SendInput timing at clean exit; no gameplay latency claims.
- Studied actual FenixAT/BladeBall code and licence; documented differences and verification limits.
- Release app/installer builds and three noninteractive latch checks pass. Live capture-to-game, UI, calibration, installer and DPI acceptance remain unverified for this revision.

--- Historical changelog ---

# Yami 1.0.6

## Repair build

- Repaired Hold activation for valid Windows input without Raw Input edges;
  unified Toggle edges and removed sticky Settings suspension.
- Verified real capture-to-received-input in both modes, tolerance boundaries,
  reappearance, Hold release and DXGI pixel sampling.
- Added topmost/centred windows, reduced-motion-aware launch fade, #FFFFFF fresh
  defaults, exit confirmation and shutdown cleanup.
- Replaced fullscreen calibration painting with a tiny alpha marker and added
  whole-GUI-hidden calibration/eyedropper selection.
- UI and installer runtime verification remain for manual testing. See
  REPAIR-NOTES.md for confirmed causes and verification limits.

## Original Rust build

- Rewrote the application and separate installer in Rust with native Windows APIs.
- Added the compact near-black/crimson main, Settings, picker, installer and
  replacement-confirmation layouts from the supplied reference.
- Implemented persistent keyboard/mouse capture, RGB picker and prior-INI migration.
- Added Fixed Point calibration with normalized client coordinates.
- Implemented reusable DXGI/D3D11 ROI processing and single-owner transition-based
  input dispatch, Hold cancellation and clear-state confirmation.
- Added minimise-to-tray and Open/Exit tray actions.
- Added per-user installation, SHA-256 verification, owned-file rollback,
  Installed Apps registration and settings-preserving uninstallation.
- Release app/installer builds and OS-only dependency inspection succeeded.
- Runtime and gameplay validation are reserved for the user's manual testing.
