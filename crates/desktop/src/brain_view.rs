use bevy_egui::egui::*;
use serde::Deserialize;
use std::collections::BTreeMap;

const W: usize = 384;
const H: usize = 384;
#[derive(Deserialize)]
struct Atlas {
    neurons: Vec<Neuron>,
}
#[derive(Deserialize)]
struct Neuron {
    segments: Vec<[[f32; 3]; 2]>,
}

pub struct BrainView {
    neurons: Vec<Vec<(usize, f32)>>,
    texture: Option<TextureHandle>,
    tick: u64,
    error: String,
    baseline: Vec<f32>,
    glow: Vec<f32>,
}
impl BrainView {
    pub fn load() -> Self {
        let mut view = Self {
            neurons: vec![],
            texture: None,
            tick: u64::MAX,
            error: String::new(),
            baseline: vec![],
            glow: vec![],
        };
        let result =
            serde_json::from_str::<Atlas>(include_str!("../../../assets/brain_atlas.json"))
                .map_err(|e| e.to_string());
        let atlas = match result {
            Ok(a) => a,
            Err(e) => {
                view.error = e;
                return view;
            }
        };
        // Two registered anatomical projections, like paired tomography views.
        // These are sampled real SWC branches, never invented neural wiring.
        for neuron in atlas.neurons {
            let mut pixels = BTreeMap::<usize, f32>::new();
            for [a, b] in neuron.segments {
                for projection in 0..2 {
                    let project = |p: [f32; 3]| -> [f32; 3] {
                        if projection == 0 {
                            [W as f32 * (0.5 + p[0] * 0.47), 109. + p[1] * 178., p[2]]
                        } else {
                            [W as f32 * (0.5 + p[0] * 0.43), 294. + p[2] * 178., p[1]]
                        }
                    };
                    let a = project(a);
                    let b = project(b);
                    let n =
                        ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as usize).clamp(1, 64);
                    for i in 0..=n {
                        let t = i as f32 / n as f32;
                        let x = (a[0] + (b[0] - a[0]) * t).round() as i32;
                        let y = (a[1] + (b[1] - a[1]) * t).round() as i32;
                        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
                            let depth = (0.6 + (a[2] + (b[2] - a[2]) * t) * 0.35).clamp(0.15, 0.9);
                            pixels
                                .entry(y as usize * W + x as usize)
                                .and_modify(|v| *v = v.max(depth))
                                .or_insert(depth);
                        }
                    }
                }
            }
            view.neurons.push(pixels.into_iter().collect());
        }
        view
    }
    pub fn reset(&mut self) {
        self.tick = u64::MAX;
        self.texture = None;
        self.baseline.clear();
        self.glow.clear();
    }
    pub fn draw(&mut self, ui: &mut Ui, rect: Rect, tick: u64, rates: &[f32]) {
        if self.texture.is_none() || self.tick != tick {
            if self.baseline.len() != self.neurons.len() || tick < self.tick {
                self.baseline = vec![0.0002; self.neurons.len()];
                self.glow = vec![0.; self.neurons.len()];
            }
            let mut density = vec![[0f32; 2]; W * H];
            let changes: Vec<f32> = self
                .baseline
                .iter()
                .enumerate()
                .map(|(i, baseline)| {
                    (rates.get(i).copied().unwrap_or(0.) - baseline).max(0.) / (baseline + 0.0001)
                })
                .collect();
            let mut ranked = changes.clone();
            ranked.sort_by(f32::total_cmp);
            // A common rise across the graph must not turn the whole scan orange.
            // Highlight the strongest local increases, with a fixed minimum
            // contrast so tiny numerical changes cannot create a bright flash.
            let threshold = ranked.get(ranked.len() * 9 / 10).copied().unwrap_or(0.);
            let high = ranked.get(ranked.len() * 99 / 100).copied().unwrap_or(0.);
            let contrast = (high - threshold).max(0.25);
            for (i, pixels) in self.neurons.iter().enumerate() {
                let rate = rates.get(i).copied().unwrap_or(0.);
                // Display real activity above each neuron's recent baseline.
                // This contrast scale reveals local changes without inventing
                // motion-driven pulses; the display holds still when paused.
                let activation = ((changes[i] - threshold) / contrast).clamp(0., 1.);
                self.glow[i] = activation.max(self.glow[i] * 0.45);
                self.baseline[i] = self.baseline[i] * 0.96 + rate * 0.04;
                let glow = self.glow[i];
                for &(p, depth) in pixels {
                    density[p][0] += depth * 0.32;
                    density[p][1] += depth * glow * 0.7;
                }
            }
            let mut colors = vec![Color32::BLACK; W * H];
            for y in 1..H - 1 {
                for x in 1..W - 1 {
                    let i = y * W + x;
                    let d = density[i][0];
                    let halo = (density[i - 1][1]
                        + density[i + 1][1]
                        + density[i - W][1]
                        + density[i + W][1])
                        * 0.1;
                    let g = 1. - (-(density[i][1] + halo)).exp();
                    let d = 1. - (-d).exp();
                    colors[i] = Color32::from_rgb(
                        (d * 65. + g * 220.).min(255.) as u8,
                        (d * 155. + g * 132.).min(255.) as u8,
                        (d * 205. + g * 30.).min(255.) as u8,
                    );
                }
            }
            let image = ColorImage::new([W, H], colors);
            if let Some(t) = self.texture.as_mut() {
                t.set(image, TextureOptions::LINEAR);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "anatomical-brain",
                    image,
                    TextureOptions::LINEAR,
                ));
            }
            self.tick = tick;
        }
        ui.painter().rect_filled(rect, 0, Color32::BLACK);
        if let Some(t) = &self.texture {
            ui.painter().image(
                t.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        let response = ui.interact(rect, Id::new("brain-volume"), Sense::hover());
        response.on_hover_text(if self.error.is_empty() {
            "MaleCNS anatomy · two projections\n336 sampled neurons; orange highlights the strongest local activity increases above their recent baseline."
        } else { &self.error });
    }
}
