use super::*;

pub(super) fn blend(bg: Color32, fg: Color32, amount: f32) -> Color32 {
    let c = |b: u8, f: u8| (b as f32 + (f as f32 - b as f32) * amount).round() as u8;
    Color32::from_rgb(c(bg.r(), fg.r()), c(bg.g(), fg.g()), c(bg.b(), fg.b()))
}

/// Local action styling shared by the welcome screen and file dialogs.
pub(super) fn action(
    ui: &mut Ui,
    label: &str,
    primary: bool,
    enabled: bool,
    width: f32,
) -> egui::Response {
    ui.scope(|ui| {
        let bg = ui.visuals().panel_fill;
        let fg = ui.visuals().text_color();
        let visuals = &mut ui.style_mut().visuals;
        for (widget, strength) in [
            (&mut visuals.widgets.inactive, 0.06),
            (&mut visuals.widgets.hovered, 0.11),
            (&mut visuals.widgets.active, 0.16),
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.bg_fill = if primary {
                blend(fg, bg, strength)
            } else {
                blend(bg, fg, strength)
            };
            widget.weak_bg_fill = widget.bg_fill;
            widget.bg_stroke = Stroke::NONE;
            widget.fg_stroke.color = if primary { bg } else { fg };
            widget.expansion = 0.0;
        }
        ui.add_enabled(
            enabled,
            egui::Button::new(
                RichText::new(label)
                    .size(13.0)
                    .color(if primary { bg } else { fg }),
            )
            .min_size(vec2(width, 36.0)),
        )
    })
    .inner
}
