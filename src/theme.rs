use eframe::egui::{Color32, FontFamily};
pub fn ui_font(ui: &eframe::egui::Ui, size: f32) -> eframe::egui::FontId {
    eframe::egui::FontId::new(
        size,
        eframe::egui::TextStyle::Body.resolve(ui.style()).family,
    )
}
pub fn mix(bg: Color32, fg: Color32, amount: f32) -> Color32 {
    let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    Color32::from_rgb(c(bg.r(), fg.r()), c(bg.g(), fg.g()), c(bg.b(), fg.b()))
}
pub fn code_background(bg: Color32, fg: Color32) -> Color32 {
    mix(bg, fg, 0.065)
}
pub fn bold(family: &FontFamily) -> FontFamily {
    match family {
        FontFamily::Name(name)
            if ["Segoe UI", "Arial", "Calibri", "Consolas"].contains(&name.as_ref()) =>
        {
            FontFamily::Name(format!("bold:{name}").into())
        }
        _ => FontFamily::Name("bold".into()),
    }
}
