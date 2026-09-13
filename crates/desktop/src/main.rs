mod app;
mod brain_view;
mod coordinator;
mod display;
mod tutorial;
mod tutorial_demo;
mod tutorial_view;
mod worker_client;
use app::Desktop;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
fn main() {
    let root = std::env::var_os("FLY_AND_YOU_ROOT")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let root = exe.ancestors().nth(4)?;
            root.join("Cargo.toml")
                .is_file()
                .then(|| root.to_path_buf())
        })
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap()
        });
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
