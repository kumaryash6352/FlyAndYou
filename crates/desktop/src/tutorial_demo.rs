use crate::{brain_view::BrainView, worker_client::Telemetry};
use bevy_egui::egui::{self, TextureHandle};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};
use wire_types::hash;
use world_core::{SENSOR_PROFILE, tutorial_cue};

#[derive(Deserialize)]
struct Recording {
    schema: u32,
    sensor: String,
    profile_sha256: String,
    sources: BTreeMap<String, String>,
    seed: u64,
    seconds_per_sample: f32,
    scenes: Vec<RecordedScene>,
}
#[derive(Deserialize)]
struct RecordedScene {
    color: [u8; 3],
    world_sha256: String,
    rgb_sha256: String,
    samples: Vec<Telemetry>,
}

pub struct Clip {
    world_rgb: Vec<u8>,
    eye_rgb: Vec<u8>,
    samples: Vec<Telemetry>,
    pub world_texture: Option<TextureHandle>,
    pub eye_texture: Option<TextureHandle>,
}
pub struct ColorDemos {
    pub clips: Vec<Clip>,
    brains: [BrainView; 2],
}

impl ColorDemos {
    pub fn load(root: &Path, profile: &str, anatomy_count: usize) -> Result<Self, String> {
        let data =
            std::fs::read(root.join("data/cache/tutorial.json")).map_err(|e| e.to_string())?;
        let recording: Recording = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
        if recording.schema != 1
            || recording.sensor != SENSOR_PROFILE
            || recording.profile_sha256 != profile
            || recording.seed != 7
            || recording.seconds_per_sample != 0.2
            || recording.scenes.len() != 2
        {
            return Err("Color demonstration profile changed; run scripts/setup.sh.".into());
        }
        for name in [
            "controller/brainworker/model.py",
            "controller/brainworker/chromatic.py",
            "controller/brainworker/motor.py",
            "controller/brainworker/tutorial.py",
            "assets/brain_atlas.json",
        ] {
            let bytes = std::fs::read(root.join(name)).map_err(|e| e.to_string())?;
            if recording.sources.get(name) != Some(&hash(&bytes)) {
                return Err("Color demonstration model changed; run scripts/setup.sh.".into());
            }
        }
        let mut clips = Vec::new();
        for (scene, color) in recording
            .scenes
            .into_iter()
            .zip([[195, 80, 57], [232, 186, 60]])
        {
            let world = tutorial_cue(color);
            let eye_rgb = world.observe();
            if scene.color != color
                || scene.world_sha256 != world.state_hash()
                || scene.rgb_sha256 != hash(&eye_rgb)
                || scene.samples.len() != 21
                || scene.samples.iter().enumerate().any(|(i, s)| {
                    s.ticks != i as u64 * 5
                        || s.anatomy_activity.len() != anatomy_count
                        || s.anatomy_activity
                            .iter()
                            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                })
            {
                return Err(
                    "Color demonstration image or activity changed; run scripts/setup.sh.".into(),
                );
            }
            clips.push(Clip {
                world_rgb: world.compose(),
                eye_rgb,
                samples: scene.samples,
                world_texture: None,
                eye_texture: None,
            });
        }
        Ok(Self {
            clips,
            brains: [BrainView::load(), BrainView::load()],
        })
    }
    pub fn reset(&mut self) {
        self.brains = [BrainView::load(), BrainView::load()];
    }
    pub fn textures(&mut self, ctx: &egui::Context) {
        for (i, clip) in self.clips.iter_mut().enumerate() {
            if clip.world_texture.is_none() {
                clip.world_texture = Some(ctx.load_texture(
                    format!("demo-world-{i}"),
                    egui::ColorImage::from_rgb([640, 360], &clip.world_rgb),
                    egui::TextureOptions::NEAREST,
                ));
                clip.eye_texture = Some(ctx.load_texture(
                    format!("demo-eye-{i}"),
                    egui::ColorImage::from_rgb([128, 96], &clip.eye_rgb),
                    egui::TextureOptions::NEAREST,
                ));
            }
        }
    }
    pub fn draw_brain(&mut self, ui: &mut egui::Ui, rect: egui::Rect, index: usize, elapsed: f32) {
        let samples = &self.clips[index].samples;
        let sample = &samples[((elapsed / 0.2) as usize).min(samples.len() - 1)];
        self.brains[index].draw(ui, rect, sample.ticks, &sample.anatomy_activity);
    }
}
