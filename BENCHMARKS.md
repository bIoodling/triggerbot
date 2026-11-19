# Measured validation — 19 November 2026

Hardware: **NVIDIA GeForce RTX 4060**, Direct3D 11 hardware device. Release x64 build.
Raw final-run results are in `validation/results.json` and `validation/results.txt`.
These are measured values from this machine, not performance guarantees.

**22 test groups passed, 0 failed.** The groups include the
requested A–H sequences, capture failure, stale configuration, output pairing,
settings persistence, actual HLSL execution and live capture/input integration.

| Measurement | Samples | Median (ms) | P95 (ms) | P99 (ms) |
|---|---:|---:|---:|---:|
| Desktop acquisition API call | 70 | 0.057000 | 0.086300 | 0.574100 |
| ROI submission CPU work | 785 | 0.029000 | 0.055800 | 0.105100 |
| GPU shader execution | 696 | 0.004096 | 0.004096 | 0.005120 |
| GPU submit to CPU result available | 785 | 0.052300 | 0.123400 | 0.618700 |
| SendInput API call | 70 | 0.753700 | 1.494800 | 3.288700 |
| Ready result to input dispatch completion | 70 | 0.753900 | 1.495100 | 3.289000 |
| Controlled paint to received key (observed) | 33 | 16.264400 | 16.739300 | 16.803700 |

## Interpretation

The ready-result-to-input metric starts after a GPU match result is available,
passes it through SingleTrigger and ends after the synchronous SendInput call.
The benchmark permission callback returns true; the separate live scanner tests
exercise the production physical Hold checks. It does not measure when a game
consumes the input. Its P95/P99 exceeded 1 ms on this run, so the under-1 ms
processing target is **not met at those percentiles**. The median is under 1 ms.

GPU submit-to-CPU availability includes submission, ROI copy, shader, result copy
and nonblocking polling. It is not an isolated GPU-to-CPU transfer measurement.
GPU timestamp queries independently measure shader execution; queries not ready
at readback are excluded, explaining their lower sample count. No missing values
are filled in. Acquisition timing is an API-call duration on a controlled,
already-updating desktop, not complete game-render-to-capture delay.

Observed end-to-end timing starts before GDI paints the target patch and ends in
the receiving window's WM_KEYDOWN handler. It includes desktop composition,
capture, processing, input insertion and message reception. The harness's 1 ms
message-pump interval contributes scheduling granularity. This is a controlled
desktop observation, not a game/network/parry latency measurement. Results from
separate metric populations must not be summed as percentile estimates.

## Accuracy and duplicates

- GPU classification cases: 785 (521
  expected positives, 264 expected negatives).
- False positives: 0/264 (0%).
- False negatives: 0/521 (0%).
- Live qualified appearances: 33; received outputs:
  33; extra/duplicate outputs: 0.
- Continuous 500 ms target, brief flicker, qualified reappearance, Hold release,
  100 rapid synthetic cycles and concurrent completions passed their assertions.

The rates above apply only to the constructed test frames, not to Blade Ball.
Live output used F24. Mouse pair construction was checked separately. A real
Roblox viewport and the referenced camera screenshots were unavailable, so
interactive game calibration and game-specific error rates remain unmeasured.

Normalized placement was checked for 1440×1080, 1920×1080, 2560×1440, 3840×2160
and other window sizes, negative origins, and 100/125/150/200% scaling inputs.
Those are deterministic physical-coordinate tests, not actual OS DPI changes.
Rotated/HDR monitors and every hardware/controller device were not tested.

The final binary also received a native UI layout check. Main-window dimensions,
Cursorless toggle placement, relaunch persistence, settings layout and Escape
handling were observed. No claim of guaranteed perfect parries or guaranteed
sub-1 ms end-to-end latency is made.
