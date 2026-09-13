# First Flight tutorial — approved design

Scope: implement the approved Level 1 introduction and remove elapsed-time losses. Later puzzle designs remain pending implementation: Borrowed Ground, Touch and Go, Pull the Rug, Swat Team, Long Way Home. That order introduces one new mechanic at a time; later pressure comes from the moving fly and physical hazards.

Use the existing native Bevy 0.19.1 / bevy-egui 0.42.0 game, actual fly artwork, actual anatomical scan, and exact eye-level observation. The intro is presentation state; it must not step the neural model or physics. Preserve the single outstanding request and edit → observation → neural response → physics contract.

The terrain is visible from the first frame (the user's later refinement of the empty opening). Reveal Fly, then “This is Fly” with a rough arrow. Pan across the existing terrain to the goal location, reveal the actual flag, then “That is the Goal” with an arrow. Zoom out to the full map; terrain never pops in. Continue with these captions, one at a time:

1. Fly needs to get to Goal
2. But Fly does not know that
3. Because Fly is the simulated nervous system of a male fruit fly (drosophila melanogaster) receiving visual stimulation and desiring left/right movement
4. You can help Fly!
5. Use Paint to guide Fly to Goal
6. Fly is scared of Red
7. Yellow looks like juicy fruit
8. Fly is a fruit fly, so the other colors make Fly do fruit fly things
9. Fly will begin moving when you start painting

Reveal the brain and FlyCam™ during the simulated nervous system caption, at the words “simulated nervous system.” Give that entire overprecise line a longer hold. Use stable, scratchy hand lettering inspired by the user's Baba Is You / XKCD references. Color only “Red” red and “Yellow” dark yellow. Keep the provided copy verbatim; technical documentation continues to distinguish the approximate model and engineered color channels from biological claims.

The final caption waits indefinitely for the first valid canvas painting gesture. That gesture removes the caption and requests play once, after its first edit fragment reaches the normal decision boundary. Keep the map framing unchanged during this handoff. Subsequent painting preserves the player's explicit pause state. Narration pauses when unfocused or loading/restoring. Space cannot start play during narration. A small Skip intro control goes to the waiting-for-paint beat; Replay intro in help resets world and brain before replaying. Ordinary reset after completion remains the existing paused reset.

Remove the 4,500-tick loss from world physics and the descriptive 45-second limit from the level fixture. Preserve physical hazards and neural worker fault timeouts. No controller, renderer semantics, or scheduling changes.

Approved follow-up: show a painted red post just ahead of Fly during the red caption, then a yellow post during the yellow caption, with matching FlyCam images and brain activity. These are disposable copies of the opening world. Prepare each response from the actual full graph, starting from the same initial model checkpoint, with the observer fixed. Replay the resulting anatomy samples for 4.2 seconds per color. Do not inject demonstration state into the live game. Remove the post and return the brain and eye displays to the untouched initial state before the remaining narration. Validate recordings against the profile, model code, atlas, world geometry, and exact rendered image; never synthesize activity if a recording is missing or stale.

Validation: focused state transition and timeout regression tests, workspace Rust checks, then a separate native instance for visual timing, frozen narration, and first-stroke edit/observation ordering. Do not stop the user's existing game.
