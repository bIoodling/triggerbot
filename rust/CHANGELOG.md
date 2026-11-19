# Yami 1.0.6

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
