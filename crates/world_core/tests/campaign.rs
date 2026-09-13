use world_core::{Action, Outcome, Stroke, Tool, World};
fn brush(tool: Tool, x0: f64, x1: f64, y: f64, radius: f64) -> Stroke {
    Stroke {
        tool,
        points: vec![[x0, y], [x1, y]],
        radius,
        color: Some([232, 186, 60]),
    }
}
#[test]
fn shared_ground_is_recycled_and_undo_cannot_duplicate_it() {
    let mut w = World::level(1);
    let first = brush(Tool::Solid, 136., 216., 268., 8.);
    let second = brush(Tool::Solid, 392., 472., 268., 8.);
    w.edit(&first).unwrap();
    assert!(w.edit(&second).is_err());
    w.edit(&brush(Tool::EraseSolid, 132., 220., 268., 24.))
        .unwrap();
    w.edit(&second).unwrap();
    w.undo().unwrap();
    w.undo().unwrap();
    assert!(w.redo().is_ok());
    w.redo().unwrap();
    assert_eq!(World::restore(&w.snapshot()).unwrap().level, 1);
    let mut forged = w.clone();
    forged.solid[67 * 160 + 40] = 1;
    assert!(World::restore(&forged.snapshot()).is_err());
}
#[test]
fn button_requires_body_contact_and_opens_gate_permanently() {
    let mut w = World::level(2);
    assert!(w.occupied(36, 60));
    w.edit(&brush(Tool::Ink, 434., 450., 257., 8.)).unwrap();
    assert!(!w.button_pressed);
    w.actor.x = 440.;
    w.actor.y = 248.;
    w.step(Action {
        steer: 0.,
        jump: false,
    });
    assert!(w.button_pressed);
    assert!(!w.occupied(36, 60));
    w.actor.x = 300.;
    w.step(Action {
        steer: 0.,
        jump: false,
    });
    assert!(w.button_pressed);
    assert!(World::restore(&w.snapshot()).unwrap().button_pressed);
    assert!(!World::level(2).button_pressed);
}
#[test]
fn floor_plug_is_erasable_and_reset_restores_it() {
    let mut w = World::level(3);
    assert!(w.occupied(84, 33));
    w.edit(&brush(Tool::EraseSolid, 300., 372., 142., 24.))
        .unwrap();
    assert!(!w.occupied(84, 33));
    assert!(World::level(3).occupied(84, 33));
    assert!(w.edit(&brush(Tool::Solid, 200., 220., 220., 8.)).unwrap() == 0);
}
#[test]
fn swatter_uses_physics_time_and_waiting_is_not_failure() {
    let mut w = World::level(4);
    let initial = w.swatter_phase();
    let _ = w.observe();
    let _ = w.compose();
    assert_eq!(w.tick, 0);
    assert_eq!(initial, w.swatter_phase());
    for _ in 0..5000 {
        w.step(Action {
            steer: 0.,
            jump: false,
        });
    }
    assert_eq!(w.outcome, Outcome::Running);
    w.actor.x = 400.;
    w.tick = 0;
    w.step(Action {
        steer: 0.,
        jump: false,
    });
    assert_eq!(w.outcome, Outcome::Running);
    w.tick = 310;
    w.step(Action {
        steer: 0.,
        jump: false,
    });
    assert_eq!(w.outcome, Outcome::Failed);
}
#[test]
fn each_level_has_its_own_checked_snapshot_and_grayscale_eye() {
    for level in 0..6 {
        let w = World::level(level);
        assert_eq!(World::restore(&w.snapshot()).unwrap().level, level);
        assert!(
            w.observe()
                .chunks_exact(3)
                .all(|p| p[0] == p[1] && p[1] == p[2])
        );
        let mut bad = w.clone();
        bad.goal[0] += 4.;
        assert!(World::restore(&bad.snapshot()).is_err());
    }
}

fn travel(w: &mut World, steer: f64, until: impl Fn(&World) -> bool) {
    for _ in 0..1600 {
        if until(w) {
            return;
        }
        assert_eq!(
            w.outcome,
            Outcome::Running,
            "level {} at {:?}",
            w.level,
            w.actor
        );
        w.step(Action { steer, jump: false });
    }
    panic!("route stuck in level {} at {:?}", w.level, w.actor);
}
#[test]
fn complete_routes_use_real_live_brushes_and_collision_physics() {
    let mut w = World::level(1);
    w.edit_live(&brush(Tool::Solid, 132., 220., 268., 8.), false)
        .unwrap();
    travel(&mut w, 0.52, |w| w.actor.x > 250.);
    w.edit_live(&brush(Tool::EraseSolid, 132., 220., 272., 24.), false)
        .unwrap();
    w.edit_live(&brush(Tool::Solid, 388., 476., 268., 8.), false)
        .unwrap();
    travel(&mut w, 0.52, |w| w.outcome == Outcome::Won);

    let mut w = World::level(2);
    travel(&mut w, 0.52, |w| w.button_pressed);
    travel(&mut w, -0.85, |w| w.outcome == Outcome::Won);

    let mut w = World::level(3);
    travel(&mut w, 0.52, |w| w.button_pressed);
    travel(&mut w, -0.85, |w| w.actor.x < 340.);
    w.edit_live(&brush(Tool::EraseSolid, 296., 376., 142., 24.), false)
        .unwrap();
    travel(&mut w, 0., |w| w.actor.y > 280.);
    travel(&mut w, -0.52, |w| w.outcome == Outcome::Won);

    let mut w = World::level(4);
    let wall = Stroke {
        tool: Tool::Solid,
        points: vec![[288., 180.], [288., 264.]],
        radius: 12.,
        color: Some([0; 3]),
    };
    w.edit_live(&wall, false).unwrap();
    travel(&mut w, 0.52, |w| w.actor.x > 265. && w.actor.vx == 0.);
    // Wait as long as needed behind the wall, then release at a safe cycle.
    travel(&mut w, 0., |w| w.tick >= 400 && w.tick % 400 == 0);
    w.edit_live(
        &Stroke {
            tool: Tool::EraseSolid,
            radius: 24.,
            ..wall
        },
        false,
    )
    .unwrap();
    travel(&mut w, 0.52, |w| w.outcome == Outcome::Won);

    let mut w = World::level(5);
    w.edit_live(&brush(Tool::Solid, 188., 284., 136., 8.), false)
        .unwrap();
    travel(&mut w, 0.52, |w| w.button_pressed);
    travel(&mut w, 0.52, |w| w.actor.x > 560.);
    w.edit_live(&brush(Tool::EraseSolid, 528., 596., 142., 24.), false)
        .unwrap();
    travel(&mut w, 0., |w| w.actor.y > 280.);
    w.edit_live(&brush(Tool::EraseSolid, 188., 284., 140., 24.), false)
        .unwrap();
    w.edit_live(&brush(Tool::Solid, 308., 404., 308., 8.), false)
        .unwrap();
    travel(&mut w, -0.85, |w| w.outcome == Outcome::Won);
}
#[test]
fn shared_ground_rejects_live_fragments_and_inverse_edits() {
    let mut w = World::level(1);
    let first = brush(Tool::Solid, 136., 216., 268., 8.);
    let second = brush(Tool::Solid, 392., 472., 268., 8.);
    w.edit_live(&first, false).unwrap();
    assert!(w.edit_live(&second, true).is_err());
    w.edit_live(&brush(Tool::EraseSolid, 132., 220., 272., 24.), false)
        .unwrap();
    let erased = w.history.last().unwrap().clone();
    w.edit_live(&second, false).unwrap();
    let mut inverse = erased;
    inverse.based_on_revision = w.revision;
    for c in &mut inverse.changes {
        std::mem::swap(&mut c.before, &mut c.after);
    }
    assert!(w.apply(&inverse).is_err());
}
#[test]
fn relocated_flag_and_gate_are_real_paintable_eye_surfaces() {
    let mut w = World::level(2);
    w.actor.x = 170.;
    w.actor.y = 248.;
    w.facing = -1;
    let gray = w.observe();
    w.edit(&brush(Tool::Ink, 146., 150., 248., 12.)).unwrap();
    let colored = w.observe();
    assert_ne!(gray, colored);
    assert!(colored.chunks_exact(3).any(|p| p[0] > p[2] + 20));
    w.button_pressed = true;
    w.actor.x = 96.;
    w.edit(&brush(Tool::Ink, 64., 84., 230., 20.)).unwrap();
    assert!(w.observe().chunks_exact(3).any(|p| p[0] > p[2] + 20));
}
