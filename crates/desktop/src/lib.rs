pub const THIRD_PARTY_NOTICES: &str = include_str!("../../../THIRD_PARTY_NOTICES.md");
pub const CANDLE_LICENSE: &str = include_str!("../../../third_party/CANDLE-LICENSE-APACHE");

mod app;
mod brain_view;
mod campaign_view;
mod coordinator;
mod display;
mod music;
mod trailer;
mod trailer_scene;
mod trailer_view;
mod tutorial;
mod tutorial_demo;
mod tutorial_view;
mod worker_client;
use app::Desktop;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
pub(crate) fn workspace_root() -> std::path::PathBuf {
    // Normal launches never probe the development checkout or protected Documents.
    // An explicit override keeps local journals convenient while assets stay embedded.
    std::env::var_os("FLY_AND_YOU_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").expect("macOS home directory"))
                .join("Library/Application Support/FlyAndYou")
        })
}
pub fn game_main() {
    let root = workspace_root();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "fly & you".into(),
                        resolution: (1280, 800).into(),
                        resize_constraints: WindowResizeConstraints {
                            min_width: 1100.,
                            min_height: 720.,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(EguiPlugin {
            bindless_mode_array_size: None,
            ..default()
        })
        .insert_non_send(Desktop::new(root))
        .add_systems(Startup, setup)
        .add_systems(Update, tick)
        .add_systems(EguiPrimaryContextPass, draw)
        .run();
}
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
fn tick(mut app: NonSendMut<Desktop>) {
    app.update();
}
fn draw(
    mut contexts: EguiContexts,
    mut app: NonSendMut<Desktop>,
    mut configured: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*configured {
        display::configure(ctx);
        *configured = true;
    }
    app.ui(ctx);
    Ok(())
}

pub fn trailer_main() {
    trailer::run(workspace_root());
}
