# Changelog

- Renamed executable, window and Windows product metadata to Yami.
- Replaced the original interface with a compact near-black and purple layout
  based on the supplied reference, using rounded panels and native controls.
- Added readable binding names, Not Set defaults, capture prompts, Escape to
  clear, outside-click cancellation and release protection while rebinding.
- Added a hue-ring colour picker, saturation/value square, validated HEX entry,
  live swatch preview, Apply and a desktop eyedropper.
- Added a Windows tray icon and custom Open / Enable / Settings / Exit menu,
  minimise-to-tray, restoration, single-instance handling and Explorer recovery.
- Added automatic atomic settings saves, corrected colour reload validation,
  current-user Windows startup and optional launch to tray.
- Added per-monitor DPI layout, bounded popup positioning and asynchronous exit.
- Isolated the supplied detection and output bodies without algorithm changes.
- Preserved keyboard simulation and disallowed the source's broken mouse-output
  selections. Existing mouse outputs migrate to Not Set.
- Release x64 compilation succeeded. Runtime testing was skipped by request.
