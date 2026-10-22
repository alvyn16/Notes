#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod demo;
mod diagrams;
mod distribution;
mod edge_blur;
mod editor;
mod index;
mod links;
mod markdown;
mod math;
mod media;
#[cfg(debug_assertions)]
mod qa;
mod theme;
mod updates;
mod windows;
mod workspace;

fn main() -> eframe::Result {
    if distribution::uninstall_requested() {
        if let Err(error) = distribution::uninstall() {
            rfd::MessageDialog::new()
                .set_title("Notes uninstall")
                .set_description(error.to_string())
                .show();
        }
        return Ok(());
    }
    if distribution::setup_requested() {
        return eframe::run_native(
            "Install Notes",
            eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([540.0, 390.0])
                    .with_resizable(false),
                ..Default::default()
            },
            Box::new(|_| Ok(Box::new(distribution::Setup::default()))),
        );
    }
    if windows::broker::forward_cli() {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([620.0, 420.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/notes.png"))
                    .expect("valid embedded Notes icon"),
            )
            .with_title("Notes"),
        renderer: eframe::Renderer::Glow,
        persistence_path: windows::profile_path(),
        window_builder: Some(Box::new(windows::validate_viewport)),
        ..Default::default()
    };
    eframe::run_native(
        "Notes",
        options,
        Box::new(|cc| Ok(Box::new(app::Notes::new(cc)))),
    )
}
