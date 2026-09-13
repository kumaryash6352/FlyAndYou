use world_core::{Outcome, Stroke, Tool, World};

pub fn bridge_stroke() -> Stroke {
    Stroke {
        tool: Tool::Solid,
        points: vec![[184., 290.], [300., 290.]],
        radius: 10.,
        color: YELLOW,
    }
}

pub fn payoff_ready(outcome: Outcome, saw_approach: bool) -> bool {
    outcome == Outcome::Won && saw_approach
}

pub const YELLOW: [u8; 3] = [232, 186, 60];
pub const RED: [u8; 3] = [195, 80, 57];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Story,
    Red,
    Rug,
    Wall,
}

pub fn world(scene: Scene) -> World {
    let mut w = World::level(if scene == Scene::Rug { 3 } else { 0 });
    match scene {
        Scene::Story => {}
        Scene::Red => {
            w.edit_live(&bridge_stroke(), false)
                .expect("trailer bridge");
            w.actor.x = 350.;
        }
        Scene::Rug => w.actor.x = 324.,
        Scene::Wall => {
            w.actor.x = 340.;
            w.edit_live(&tower(Tool::Solid), false)
                .expect("trailer tower");
        }
    }
    w
}

pub fn tower(tool: Tool) -> Stroke {
    Stroke {
        tool,
        points: vec![[396., 120.], [396., 296.]],
        radius: if tool.erase() { 24. } else { 20. },
        color: YELLOW,
    }
}

pub fn yellow_stroke() -> Stroke {
    Stroke {
        tool: Tool::Ink,
        points: vec![[140., 283.], [550., 283.]],
        radius: 24.,
        color: YELLOW,
    }
}

pub fn red_stroke() -> Stroke {
    Stroke {
        tool: Tool::Ink,
        points: vec![[400., 283.], [536., 283.]],
        radius: 24.,
        color: RED,
    }
}

pub fn rug_stroke() -> Stroke {
    Stroke {
        tool: Tool::EraseSolid,
        points: vec![[292., 142.], [380., 142.]],
        radius: 24.,
        color: YELLOW,
    }
}

/// Every fragment uses the same capsule brush and one undo transaction.
pub fn fragment(stroke: &Stroke, from: f64, to: f64) -> Stroke {
    let a = stroke.points[0];
    let b = stroke.points[1];
    let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    Stroke {
        points: vec![at(from.clamp(0., 1.)), at(to.clamp(0., 1.))],
        ..stroke.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use world_core::Action;

    #[test]
    fn drawn_bridge_supports_a_real_crossing() {
        let mut world = World::bridge();
        world.edit_live(&bridge_stroke(), false).unwrap();
        for _ in 0..900 {
            world.step(Action {
                steer: 0.52,
                jump: false,
            });
            if world.outcome != Outcome::Running {
                break;
            }
        }
        assert_eq!(world.outcome, Outcome::Won);
    }

    #[test]
    fn payoff_requires_a_real_win_and_observed_persuasion() {
        assert!(!payoff_ready(Outcome::Running, true));
        assert!(!payoff_ready(Outcome::Failed, true));
        assert!(!payoff_ready(Outcome::Won, false));
        assert!(payoff_ready(Outcome::Won, true));
    }
}
