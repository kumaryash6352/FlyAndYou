use world_core::{Action, Actor, Outcome, Stroke, Tool, World, capsule_cells};
fn stroke(tool: Tool, pts: &[[f64; 2]]) -> Stroke {
    Stroke {
        tool,
        points: pts.to_vec(),
        radius: 4.0,
        color: Some([227, 178, 61]),
    }
}
fn walk(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step(Action {
            steer: 1.0,
            jump: false,
        });
    }
}
#[test]
fn waiting_on_safe_ground_has_no_time_limit() {
    let mut world = World::bridge();
    for _ in 0..5000 {
        world.step(Action {
            steer: 0.,
            jump: false,
        });
    }
    assert_eq!(world.outcome, Outcome::Running);
    assert_eq!(world.tick, 5000);
}
#[test]
fn introductory_goal_reveal_preserves_all_terrain_pixels() {
    let world = World::bridge();
    let before = world.compose_without_goal();
    let after = world.compose();
    let changed: Vec<_> = before
        .chunks_exact(3)
        .zip(after.chunks_exact(3))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert!(!changed.is_empty(), "the Goal must have its own reveal");
    assert!(
        changed
            .into_iter()
            .all(|i| (560..580).contains(&(i % 640)) && (242..280).contains(&(i / 640)))
    );
}
#[test]
fn solid_bridge_can_be_crossed_but_ink_cannot() {
    let mut w = World::bridge();
    w.edit(&stroke(Tool::Solid, &[[188., 282.], [292., 282.]]))
        .unwrap();
    walk(&mut w, 1100);
    assert_eq!(w.outcome, Outcome::Won);
    let mut w = World::bridge();
    w.edit(&stroke(Tool::Ink, &[[188., 282.], [292., 282.]]))
        .unwrap();
    walk(&mut w, 1100);
    assert_eq!(w.outcome, Outcome::Failed);
}
#[test]
fn fast_stroke_is_continuous_and_resampling_invariant() {
    let a = capsule_cells(&[[20., 20.], [180., 20.]], 4., 4).unwrap();
    let b = capsule_cells(&[[20., 20.], [80., 20.], [180., 20.]], 4., 4).unwrap();
    assert_eq!(a, b);
    assert!(a.contains(&(4 * 160 + 20)));
}
#[test]
fn illegal_part_rejects_whole_stroke_and_pose() {
    let mut w = World::bridge();
    let before = w.state_hash();
    assert!(
        w.edit(&stroke(Tool::Solid, &[[100., 272.], [64., 272.]]))
            .is_err()
    );
    assert_eq!(before, w.state_hash());
}
#[test]
fn solid_cannot_cover_flag_but_ink_can_color_it() {
    let mut w = World::bridge();
    let before = w.state_hash();
    assert!(
        w.edit(&stroke(Tool::Solid, &[[510., 248.], [560., 248.]]))
            .is_err()
    );
    assert_eq!(before, w.state_hash());
    assert!(
        w.edit(&stroke(Tool::Ink, &[[561., 248.], [573., 248.]]))
            .unwrap()
            > 0
    );
}
#[test]
fn removing_support_causes_fall() {
    let mut w = World::bridge();
    w.edit(&stroke(Tool::Solid, &[[188., 282.], [292., 282.]]))
        .unwrap();
    w.actor = Actor {
        x: 240.,
        y: 272.,
        vx: 0.,
        vy: 0.,
        grounded: true,
    };
    w.edit(&stroke(Tool::EraseSolid, &[[225., 282.], [255., 282.]]))
        .unwrap();
    walk(&mut w, 25);
    assert!(w.actor.y > 280.);
}
#[test]
fn undo_cannot_restore_solid_inside_actor() {
    let mut w = World::bridge();
    w.edit(&stroke(Tool::Solid, &[[230., 240.], [250., 240.]]))
        .unwrap();
    w.edit(&stroke(Tool::EraseSolid, &[[230., 240.], [250., 240.]]))
        .unwrap();
    w.actor.x = 240.;
    w.actor.y = 240.;
    let before = w.state_hash();
    assert!(w.undo().is_err());
    assert_eq!(before, w.state_hash());
}
#[test]
fn undo_redo_preserves_exact_pixels_and_does_not_rewind_actor() {
    let mut w = World::bridge();
    let original = w.compose();
    w.edit(&stroke(Tool::Ink, &[[120., 285.], [170., 305.]]))
        .unwrap();
    let painted = w.compose();
    assert_ne!(painted, original);
    w.actor.x = 90.;
    w.undo().unwrap();
    assert_eq!(w.compose(), original);
    assert_eq!(w.actor.x, 90.);
    w.redo().unwrap();
    assert_eq!(w.compose(), painted);
}
#[test]
fn wall_and_low_ceiling_stop_swept_actor() {
    let mut w = World::bridge();
    for y in 60..70 {
        w.solid[y * 160 + 30] = 1;
    }
    walk(&mut w, 200);
    assert!(w.actor.x <= 114.000001);
}
#[test]
fn four_pixel_step_is_climbed() {
    let mut w = World::bridge();
    for x in 25..40 {
        w.solid[69 * 160 + x] = 1;
    }
    walk(&mut w, 50);
    assert!(w.actor.x > 120.);
    assert_eq!(w.actor.y, 268.);
}
#[test]
fn low_ceiling_blocks_step_up() {
    let mut w = World::bridge();
    for x in 25..40 {
        w.solid[69 * 160 + x] = 1;
    }
    for x in 22..40 {
        w.solid[65 * 160 + x] = 1;
    }
    walk(&mut w, 200);
    assert!(w.actor.x <= 94.000001);
}
#[test]
fn snapshot_roundtrip_replays_world_and_observations() {
    let mut w = World::bridge();
    w.edit(&stroke(Tool::Ink, &[[140., 220.]])).unwrap();
    walk(&mut w, 20);
    let mut copy = World::restore(&w.snapshot()).unwrap();
    walk(&mut w, 30);
    walk(&mut copy, 30);
    assert_eq!(w.state_hash(), copy.state_hash());
    assert_eq!(w.observe(), copy.observe());
}
#[test]
fn observation_is_actor_free_and_exact_size() {
    let w = World::bridge();
    assert_eq!(w.observe().len(), 128 * 96 * 3);
    let mut v = w.clone();
    v.actor.vx = -48.;
    assert_eq!(w.observe(), v.observe());
    v.facing = -1;
    assert_ne!(w.observe(), v.observe());
}
#[test]
fn perspective_cue_is_occluded_by_terrain() {
    let mut w = World::bridge();
    w.edit(&Stroke {
        tool: Tool::Ink,
        points: vec![[132., 260.]],
        radius: 24.,
        color: Some([232, 186, 60]),
    })
    .unwrap();
    let warm_pixels = |rgb: Vec<u8>| {
        rgb.chunks_exact(3)
            .filter(|c| c[0] > 160 && c[0] > c[2].saturating_add(60))
            .count()
    };
    assert!(warm_pixels(w.observe()) > 0);
    for y in 0..90 {
        w.solid[y * 160 + 24] = 1;
    }
    assert_eq!(warm_pixels(w.observe()), 0);
}
#[test]
fn noop_does_not_increment_revision_and_base_cannot_be_erased() {
    let mut w = World::bridge();
    w.edit(&stroke(Tool::EraseSolid, &[[100., 300.]])).unwrap();
    assert_eq!(w.revision, 0);
    assert_eq!(w.base[75 * 160 + 25], 1);
}
#[test]
fn nan_action_is_rejected_without_mutation() {
    let mut w = World::bridge();
    let h = w.state_hash();
    assert!(!w.step(Action {
        steer: f64::NAN,
        jump: false
    }));
    assert_eq!(h, w.state_hash());
}

#[test]
fn live_scribble_clips_actor_and_undoes_the_whole_gesture() {
    let mut w = World::bridge();
    let start = w.compose();
    let actor = w.actor;
    let mut s = stroke(Tool::Solid, &[[64., 272.], [170., 250.]]);
    assert!(w.edit_live(&s, false).unwrap() > 0);
    s.points = vec![[170., 250.], [210., 282.]];
    assert!(w.edit_live(&s, true).unwrap() > 0);
    assert_eq!(w.actor, actor);
    assert_eq!(w.history.len(), 1);
    assert!(!w.occupied(16, 68));
    w.undo().unwrap();
    assert_eq!(w.compose(), start);
}

#[test]
fn only_painted_surfaces_bring_color_into_the_eye_view() {
    let mut w = World::bridge();
    w.actor.x = 500.;
    let initial = w.observe();
    assert!(
        initial
            .chunks_exact(3)
            .all(|p| p[0] == p[1] && p[1] == p[2])
    );
    let mut s = stroke(Tool::Ink, &[[571., 248.]]);
    s.radius = 10.;
    assert!(w.edit(&s).unwrap() > 0);
    // With no flag pasted on the backdrop, these colored pixels come from the
    // modeled cloth and pole ray intersections.
    assert!(
        w.observe()
            .chunks_exact(3)
            .filter(|p| p[0] > p[2].saturating_add(35))
            .count()
            > 8
    );
    s.tool = Tool::EraseInk;
    w.edit(&s).unwrap();
    assert_eq!(w.observe(), initial);

    // The end walls are physical surfaces too, including protected border pixels.
    w.actor.x = 64.;
    w.facing = -1;
    s.tool = Tool::Ink;
    s.radius = 24.;
    s.points = vec![[0., 260.], [24., 280.]];
    assert!(w.edit_live(&s, false).unwrap() > 0);
    assert_eq!(w.paint[(272 * 640) * 4 + 3], 255);
    assert!(
        w.observe()
            .chunks_exact(3)
            .any(|p| p[0] > p[2].saturating_add(35))
    );
    s.points = vec![[128., 0.], [180., 16.]];
    assert!(w.edit_live(&s, false).unwrap() > 0);
    // The dry brush can leave individual edge pixels blank.
    assert!((128..180).any(|x| w.paint[x * 4 + 3] == 255));
}

#[test]
fn wall_ink_reaches_both_eyes_without_support_and_restores_exactly() {
    let mut w = World::bridge();
    let initial = w.observe();
    let solids = w.solid.clone();
    let mut s = stroke(Tool::Ink, &[[90., 260.], [150., 260.]]);
    s.radius = 20.;
    assert!(w.edit(&s).unwrap() > 0);
    assert_eq!(w.solid, solids);
    assert!(!w.occupied(26, 65));
    let eye = w.observe();
    for half in [0..64, 64..128] {
        assert!((0..96).any(|y| half.clone().any(|x| {
            let p = &eye[(y * 128 + x) * 3..][..3];
            p[0].min(p[1]) > p[2].saturating_add(24)
        })));
    }
    let saved = World::restore(&w.snapshot()).unwrap();
    assert_eq!(saved.observe(), eye);
    let mut occluded = saved.clone();
    let mut post = stroke(Tool::Solid, &[[84., 200.], [84., 284.]]);
    post.color = None;
    occluded.edit(&post).unwrap();
    assert!(
        occluded
            .observe()
            .chunks_exact(3)
            .all(|p| p[0] == p[1] && p[1] == p[2])
    );
    w.undo().unwrap();
    assert_eq!(w.observe(), initial);
    w.redo().unwrap();
    assert_eq!(w.observe(), eye);
    s.tool = Tool::EraseInk;
    w.edit(&s).unwrap();
    assert_eq!(w.observe(), initial);
}

#[test]
fn colored_ground_and_its_ink_undo_as_one_live_gesture() {
    let mut w = World::bridge();
    let initial = w.compose();
    let mut s = stroke(Tool::Solid, &[[100., 248.], [100., 270.]]);
    s.radius = 8.;
    w.edit_live(&s, false).unwrap();
    s.points = vec![[100., 270.], [100., 282.]];
    w.edit_live(&s, true).unwrap();
    let eye = w.observe();
    assert!(
        eye.chunks_exact(3)
            .any(|p| p[0].min(p[1]) > p[2].saturating_add(24))
    );
    assert_eq!(w.history.len(), 1);
    let painted = w.compose();
    w.undo().unwrap();
    assert_eq!(w.compose(), initial);
    w.redo().unwrap();
    assert_eq!(w.compose(), painted);
    assert_eq!(World::restore(&w.snapshot()).unwrap().observe(), eye);
    s.tool = Tool::EraseSolid;
    s.points = vec![[100., 248.], [100., 282.]];
    w.edit(&s).unwrap();
    assert_eq!(w.compose(), initial);
    s.tool = Tool::Solid;
    s.color = None;
    w.edit(&s).unwrap();
    assert!(
        w.observe()
            .chunks_exact(3)
            .all(|p| p[0] == p[1] && p[1] == p[2])
    );
}
