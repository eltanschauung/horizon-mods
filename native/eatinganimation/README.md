# Eating Animation

This SDK port uses the installed resource pack's bitten food and drinking frames.
It keeps the existing animation thresholds at 11, 21 and 27 ticks. If a frame
model or its texture is missing, the renderer keeps the item's regular model.

Build the project in `sdk/` with `cargo horizon-mod build` against the pinned
fork SDK.
