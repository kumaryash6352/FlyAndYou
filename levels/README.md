# Campaign

The runtime definitions live in `crates/world_core/src/levels.rs`; `World::bridge()` keeps the original tutorial geometry. The JSON file describes Level 1 and is not a runtime loader. All six maps are 640 × 360 pixels with four-pixel collision cells.

| Level | Puzzle | New idea |
| --- | --- | --- |
| 1 | First Flight | The animated introduction teaches Ground and the color rules. |
| 2 | Borrowed Ground | Two gaps share one Ground patch. Fully erase one bridge before building the other. |
| 3 | Touch and Go | Fly's feet press A; gate A stays open. Turn back toward the left-hand Goal. |
| 4 | Pull the Rug | Cross the stitched floor to A, then erase that floor to reach the lower route. |
| 5 | Swat Team | A cycling swatter; the Ground box supports a removable stopping wall. |
| 6 | The Long Way Home | Combine the shared bridge, button, erasable floor, drop, and return route. |

Only Level 1 plays the full tutorial. Later levels begin paused and start on the first painting gesture. A normal reset rearms that start; later strokes do not override an explicit pause. Bookmark restoration remains paused. Puzzle rules are conveyed by labels on the map and the objects' behavior; the toolbox contains controls and optional neural diagnostics, not solution hints. The toolbox chooser can replay any level; after winning, Next level or N advances. R resets the current level and neural state. F9 restores the level and mechanics captured by F5 along with its brain checkpoint.

Dashed boxes labeled Ground mark where it can be added or erased in Levels 2–6. Shared boxes say “one at a time,” and an unavailable box says “in use elsewhere.” Floor plugs are labeled “erasable.” Rejected edits are labeled at the edit location. Ink remains available on physical surfaces. Shared Ground is exclusive across the first two zones, including live fragments and undo/redo. Floor plugs are seeded into the erasable player layer and restored by Reset; the final level's plug is independent of its shared bridge.

A is activated only by Fly touching the plate. Neither paint nor building presses it. The gate opens permanently for that attempt; the host also requires A before accepting the Goal. The closed gate and button are physical, paintable surfaces in Fly's eye view. Letters and editing guides belong only to the spectator view.

The swatter has 2.5 simulated seconds clear, 0.6 seconds of warning, and a 0.9-second strike. It stays parked by the ceiling while clear or warning; only contact during a strike kills. Space and neural waits hold its simulation clock. There is no elapsed-time failure.

World checkpoints identify their baked layout and validate geometry, Ground restrictions, and mechanism state. Old First Flight world snapshots remain compatible; changed campaign layouts reject incompatible snapshots. Bookmarks are still within-session joint world/brain saves.

`crates/world_core/tests/campaign.rs` exercises complete routes using live brushes and the actual collision physics at the controller's exploration/retreat speeds. Those scripted routes prove geometry and mechanics; they are not autonomous neural playthroughs or a usability study.
