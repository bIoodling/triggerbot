# Yami

Native Windows desktop application, redesigned from the supplied Yami TB source.
The executable and its Windows product description are both **Yami**. The process
remains normally visible in Task Manager.

## Run

Run `Yami.exe` on Windows 10/11 x64 with a Direct3D 11 capable display adapter.
No separate .NET or Visual C++ runtime installation is required: the release
build links the C++ runtime statically. Direct3D, DXGI, XInput, GDI+ and the
Windows shell are operating-system components.

Fresh installations show **Not Set** for Toggle, Hold and Trigger. Existing
settings are migrated by continuing to use `%APPDATA%\YamiTB\settings.ini`.
Each binding change, applied colour, and launch preference is saved immediately.
Save explicitly flushes the current configuration again. The application always
starts disabled; enabled state is intentionally not restored.

- Click a binding and press a key. Escape clears it. Clicking outside cancels.
- Toggle and Hold accept keyboard, mouse and the source's existing XInput binds.
- Trigger accepts **keyboard output only**, preserving the supplied simulation.
  The supplied source incorrectly encoded mouse outputs as keyboard events;
  this build prevents choosing those unsupported output bindings rather than
  changing input simulation. Mouse buttons remain supported for Toggle/Hold.
- For Left Mouse capture, click the same binding again; clicking elsewhere cancels.
- Hold enables scanning while held. Toggle changes its latched activation state.
- The supplied engine sends once per detection, then requires at least three
  absent frames across 500 ms to rearm. It is not a continuous autofire loop.
- Click the colour swatch or HEX value to open the hue ring, saturation/value
  square and HEX editor. Apply commits the preview. Escape/outside dismisses it.
  The eyedropper samples the desktop until the next mouse release.
- Minimise removes the window and its taskbar button; detection remains available
  through the tray. Double-click the purple tray icon to restore. Right-click
  for Open, Enable, Settings and Exit. Closing the main window exits.
- Settings contains Launch minimised to tray. `Yami.exe --tray` also starts hidden.
- Start with Windows uses the ordinary current-user Run registry entry, named
  `Yami`, pointing to `Yami.exe --tray`. Move the executable
  before enabling startup. Disable the checkbox before deleting the application.

The application uses per-monitor DPI awareness and scales the UI to fit the
monitor work area when the nominal size would exceed it. Auxiliary popups are
kept within the available work area or dismissed when their DPI changes.

## Build

Install Visual Studio 2026 / Build Tools with **Desktop development with C++**,
MSVC v145 and a Windows 10/11 SDK. Open `Yami TB.slnx`, select Release / x64,
and build, or run from PowerShell:

```powershell
.\build.ps1
```

The script copies the result to `artifacts\Yami.exe`. The project can also be
built directly using MSBuild with `/p:Configuration=Release /p:Platform=x64`.
Earlier Visual Studio versions require retargeting the project's toolset.

## Source boundaries

- `Yami TB/engine.inc`: supplied timer, output-key preparation/simulation,
  ScreenGrabber and ScannerThread bodies, retained verbatim.
- `Yami TB/activation.inc`: supplied raw input/controller handling with binding
  names, fresh-install defaults, capture clear/release protection and UI routing.
- `Yami TB/settings.inc`: atomic settings replacement, validation and startup.
- `Yami TB/frontend.inc`: native controls, custom drawing, picker and tray.
- `Yami TB/Yami.rc`: icon and Windows file/product name metadata.

The supplied source uses DXGI desktop duplication and an exact CPU RGB scan in
a radius of 50 pixels around the cursor on the primary display. Despite stale
GPU comments in that source, it contains no GPU detection shaders. The earlier
standalone executable references shaders and is not established to be equivalent
to this supplied source. This redesign is based on the supplied source.

## Verification status and inherited limits

The original source and redesigned Release x64 source compile successfully.
**No runtime, interaction, detection, output, tray, persistence, multi-monitor or
DPI testing was performed**, at the user's explicit request. Preservation has
not been functionally verified. Manual testing is required before relying on it.

The engine's primary-monitor restriction and handling of DXGI access loss are
unchanged. In particular, the supplied engine does not rebuild its duplication
object after access loss; a display mode change or desktop switch may require
restarting Yami. UI support for multiple monitors does not expand the detector's
coverage. Existing mouse output values in older settings are reset to Not Set.

Only one instance is allowed per Windows session to prevent duplicate tray icons.
The tray icon is deleted on normal exit and re-added after Explorer restarts.
Shutdown signals the original workers and waits asynchronously without blocking
the UI message loop. No process hiding, injection or security-tool evasion is used.
