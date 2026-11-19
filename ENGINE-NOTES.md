# Inspection and failure analysis

The archive contains C++ source and a project, but **no HLSL shaders**. Comments
describe GPU dirty-tile detection; the implemented ScreenGrabber instead copies
the full desktop to staging, synchronously maps it and CPU-scans a radius-50 disk
around the cursor. Exact matching already reads the selected RGB, despite green-
specific method names. The earlier standalone EXE references shaders and is a
different build; its equivalence to this source is not established.

The 1.0 UI redesign isolated the supplied detector/output/scanner bodies unchanged.
The exact engine body is retained in `reference/supplied-engine.inc.txt`.

## Confirmed issues

- Full-frame staging/map synchronization was unnecessary for a small cursor ROI.
- The scanner rearmed on activation changes and did not revalidate Hold or a
  generation token immediately before input insertion. A release during frame
  processing could leave stale output eligible.
- A fixed 500 ms / three-frame absence rule delayed genuine new events.
- DXGI access/device loss returned no result without rebuilding the duplication
  resources, potentially stalling detection after display changes.
- Mouse virtual keys were incorrectly sent as keyboard input. Version 1.0 blocked
  those choices; 1.1 maps mouse down/up correctly.
- The supplied solution pointed to the obsolete Mali TB directory.
- UI checks found EN_CHANGE arriving before the picker edit handle was assigned,
  falsely invalidating its initial HEX. Dialog handling also swallowed popup
  Escape/Enter. These UI faults are corrected in 1.1.

## The reported roughly 15 ms duplicate

No trace or reproduction of that incident was supplied. The archived scanner's
normal 500 ms absence gate does not explain a second trigger 15 ms later.
Activation resets or separate instances are possible mechanisms, **not confirmed
causes**. The original lacked single-instance protection; 1.0/1.1 include it.

Version 1.1 removes these classes of ambiguity structurally: only the scanner
dispatches, a locked transition consumes the appearance before synchronous input,
and Hold/configuration epochs reject obsolete completions. Repeated positives
cannot fire again until qualified clear observations rearm the state machine.

## State and timing semantics

IDLE means no Hold or no permission. A fresh activation enters ARMED. A match
passes through FIRED into WAITING_FOR_CLEAR; two or more valid clear frames over
the configured debounce enter REARMED. Missing/failed capture cannot confirm
absence; failures also break a partial clear streak. Mode/config changes reject
all old results. Optional minimum interval consumes suppressed appearances rather
than scheduling them later.

Release and dispatch serialize through the state lock. The dispatcher also checks
Windows asynchronous Hold state immediately before insertion. Yami has no output
queue or timer. A key pair already accepted by Windows before release cannot be
recalled; no claim is made about physical-switch scheduling or game consumption.
Partial SendInput insertion gets an immediate best-effort key-up cleanup, never
another key-down. Failed attempts consume the appearance to prevent retry bursts.

The GPU checks a small circular ROI with integer RGB differences after UNORM
decoding. Every channel must meet the tolerance. Resources are reused, there are
no per-frame heap allocations/full-screen CPU reads, and only one result is in
flight. Active acquisition waits for desktop frames; nonblocking Map polling
yields periodically while the GPU is busy. Inactive scanning waits on an event.

The implementation follows Microsoft's
[desktop duplication](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api),
[AcquireNextFrame](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe)
and [nonblocking Map](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_map_flag)
documentation. Shader duration, result availability, input API duration and
observed receiver latency are different metrics.

## Validation scope

Tests use a real hardware D3D11 device and on-screen receiver. In the test build,
only Roblox process discovery is replaced by a test-owned client. Capture,
shaders, coordinates, state transitions and input insertion are production code.
Hold is emulated and fed through the activation handler, so tests do not certify
every physical keyboard/mouse/controller. Mouse pair construction is tested;
live reception uses F24.

Roblox gameplay, network response and parry success are outside these tests. No
Roblox viewport was open, and the three camera screenshots were not attached.
Calibration is not character tracking; it requires camera-specific user selection.
Occlusion, HDR conversion, overlays and camera motion can affect captured content.
