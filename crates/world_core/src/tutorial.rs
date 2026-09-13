use crate::*;

/// A presentation-only copy. Never install this as the player's world.
pub fn tutorial_cue(color: [u8; 3]) -> World {
    let mut world = World::bridge();
    world
        .edit(&Stroke {
            tool: Tool::Solid,
            points: vec![[100., 248.], [100., 284.]],
            radius: 8.,
            color: None,
        })
        .expect("tutorial post is outside Fly and protected terrain");
    world
        .edit(&Stroke {
            tool: Tool::Ink,
            points: vec![[100., 248.], [100., 284.]],
            radius: 16.,
            color: Some(color),
        })
        .expect("tutorial surface accepts ink");
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demonstration_cues_reach_the_eye_without_moving_fly() {
        let initial = World::bridge();
        for (color, red) in [([195, 80, 57], true), ([232, 186, 60], false)] {
            let cue = tutorial_cue(color);
            assert_eq!(cue.actor, initial.actor);
            assert_eq!(cue.tick, 0);
            assert_eq!(cue.goal, initial.goal);
            let colored = cue
                .observe()
                .chunks_exact(3)
                .filter(|p| {
                    if red {
                        p[0] > p[1].saturating_add(48) && p[0] > p[2]
                    } else {
                        p[0].min(p[1]) > p[2].saturating_add(24)
                    }
                })
                .count();
            assert!(
                colored > 128 * 96 / 10,
                "cue must occupy a visible part of FlyCam"
            );
        }
        assert!(
            initial
                .observe()
                .chunks_exact(3)
                .all(|p| p[0] == p[1] && p[1] == p[2])
        );
    }
}
