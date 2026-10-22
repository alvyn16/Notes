//! Debug-only native render capture for repeatable local visual verification.
use eframe::egui;
pub fn scene(ctx: &egui::Context) -> Option<String> {
    let scene = std::env::var("NOTES_CAPTURE_SCENE").ok()?;
    let id = egui::Id::new("capture-scene-prepared");
    if ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
        return None;
    }
    ctx.data_mut(|d| d.insert_temp(id, true));
    Some(scene)
}
pub fn prepare(ctx: &egui::Context, path: &std::path::Path, text: &str) -> bool {
    if std::env::var_os("NOTES_CAPTURE_PATH").is_none() {
        return false;
    }
    let id = egui::Id::new("capture-prepared");
    if ctx.data(|data| data.get_temp::<bool>(id).unwrap_or(false)) {
        return false;
    }
    ctx.data_mut(|data| data.insert_temp(id, true));
    if let Ok(anchor) = std::env::var("NOTES_CAPTURE_ANCHOR")
        && let Some(offset) = crate::links::heading_offset(text, &anchor)
    {
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("heading-target"), (path.to_owned(), offset))
        });
    }
    std::env::var("NOTES_CAPTURE_MODE").as_deref() == Ok("live")
}
pub fn capture(ctx: &egui::Context) {
    let Some(path) = std::env::var_os("NOTES_CAPTURE_PATH") else {
        return;
    };
    let id = egui::Id::new("native-capture-start");
    let start = ctx.data_mut(|data| *data.get_temp_mut_or_insert_with(id, std::time::Instant::now));
    let requested = id.with("requested");
    if start.elapsed() > std::time::Duration::from_secs(3)
        && !ctx.data(|data| data.get_temp::<bool>(requested).unwrap_or(false))
    {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        ctx.data_mut(|data| data.insert_temp(requested, true));
    }
    for event in ctx.input(|i| i.events.clone()) {
        if let egui::Event::Screenshot { image, .. } = event {
            let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            if let Some(image) =
                image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, pixels)
            {
                match image.save(&path) {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => eprintln!("Native capture failed: {error}"),
                }
            }
        }
    }
}
