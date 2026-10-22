use super::{Rendered, mix};
use eframe::egui::{self, Ui, Vec2};

#[derive(Clone, Default)]
pub(super) struct Canvas {
    scale: Option<f32>,
    pan: Vec2,
    pub fullscreen: bool,
}
fn fit(size: Vec2, viewport: Vec2) -> f32 {
    ((viewport.x - 48.0).max(1.0) / size.x)
        .min((viewport.y - 48.0).max(1.0) / size.y)
        .min(1.0)
}
impl Canvas {
    pub(super) fn reset(&mut self) {
        self.scale = None;
        self.pan = Vec2::ZERO;
    }
    fn zoom(&mut self, factor: f32, fit: f32, anchor: Vec2) {
        let old = self.scale.unwrap_or(fit);
        let new = (old * factor).clamp(fit.min(0.25), 4.0);
        self.pan = anchor - (anchor - self.pan) * (new / old);
        self.scale = Some(new);
    }
}
// Separate paint and hit targets keep controls small without relying on font glyphs.
fn button(
    ui: &mut Ui,
    canvas: egui::Rect,
    offset: Vec2,
    icon: usize,
    label: &str,
    alpha: f32,
) -> egui::Response {
    let rect = egui::Rect::from_min_size(canvas.right_top() + offset, egui::vec2(28.0, 28.0));
    let response = ui.interact(rect, ui.id().with(label), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let fg = ui.visuals().text_color().gamma_multiply(alpha);
    let bg = mix(
        ui.visuals().panel_fill,
        ui.visuals().text_color(),
        if response.hovered() { 0.12 } else { 0.04 },
    )
    .gamma_multiply(alpha);
    let p = ui.painter();
    p.rect_filled(rect, 4, bg);
    let c = rect.center();
    let s = egui::Stroke::new(1.4_f32, fg);
    let line = |a: Vec2, b: Vec2| {
        p.line_segment([c + a, c + b], s);
    };
    match icon {
        0 => {
            line(egui::vec2(-3.0, -5.0), egui::vec2(-7.0, 0.0));
            line(egui::vec2(-7.0, 0.0), egui::vec2(-3.0, 5.0));
            line(egui::vec2(3.0, -5.0), egui::vec2(7.0, 0.0));
            line(egui::vec2(7.0, 0.0), egui::vec2(3.0, 5.0));
        }
        1 => {
            for sign in [-1.0, 1.0] {
                line(egui::vec2(sign * 2.0, -6.0), egui::vec2(sign * 6.0, -6.0));
                line(egui::vec2(sign * 6.0, -6.0), egui::vec2(sign * 6.0, -2.0));
                line(egui::vec2(sign * 2.0, 6.0), egui::vec2(sign * 6.0, 6.0));
                line(egui::vec2(sign * 6.0, 6.0), egui::vec2(sign * 6.0, 2.0));
            }
        }
        2 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(12.0, 10.0)),
                1,
                s,
                egui::StrokeKind::Inside,
            );
        }
        3 => {
            line(egui::vec2(-5.0, 0.0), egui::vec2(5.0, 0.0));
            line(egui::vec2(0.0, -5.0), egui::vec2(0.0, 5.0));
        }
        _ => line(egui::vec2(-5.0, 0.0), egui::vec2(5.0, 0.0)),
    }
    response.on_hover_text(label)
}
pub(super) fn show(
    ui: &mut Ui,
    result: &Rendered,
    key: u64,
    state: &mut Canvas,
    height: f32,
    fullscreen: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    let control_focused = ui.memory(|m| {
        [
            "Edit code (Enter)",
            "Fullscreen",
            "Close fullscreen",
            "Reset to fit (0)",
            "Zoom in (+)",
            "Zoom out (−)",
        ]
        .iter()
        .any(|label| m.has_focus(ui.id().with(label)))
    });
    let visible = ui.rect_contains_pointer(rect)
        || response.has_focus()
        || response.dragged()
        || control_focused;
    let alpha = ui
        .ctx()
        .animate_bool_with_time(response.id.with("controls"), visible, 0.12);
    let bg = ui.visuals().panel_fill;
    let border = mix(
        bg,
        ui.visuals().text_color(),
        if response.has_focus() { 0.35 } else { 0.12 },
    );
    ui.painter().rect(
        rect,
        4,
        bg,
        egui::Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let size = result
        .as_ref()
        .map(|d| d.size)
        .unwrap_or(egui::vec2(400.0, 200.0));
    let fitted = fit(size, rect.size());
    if response.clicked() {
        response.request_focus();
    }
    if response.dragged() {
        state.pan += ui.input(|i| i.pointer.delta());
    }
    if response.hovered() {
        let factor = ui.input(|i| i.zoom_delta());
        if factor != 1.0 {
            let anchor =
                ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center()) - rect.center();
            state.zoom(factor, fitted, anchor);
        }
        ui.output_mut(|o| {
            o.cursor_icon = if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            }
        });
    }
    let mut edit = false;
    if response.has_focus() {
        for (key, delta) in [
            (egui::Key::ArrowLeft, egui::vec2(24.0, 0.0)),
            (egui::Key::ArrowRight, egui::vec2(-24.0, 0.0)),
            (egui::Key::ArrowUp, egui::vec2(0.0, 24.0)),
            (egui::Key::ArrowDown, egui::vec2(0.0, -24.0)),
        ] {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                state.pan += delta;
            }
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Num0)) {
            state.reset();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Plus)) {
            state.zoom(1.25, fitted, Vec2::ZERO);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Minus)) {
            state.zoom(0.8, fitted, Vec2::ZERO);
        }
        edit |= ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(1.0)), |ui| {
        ui.set_clip_rect(rect.shrink(1.0).intersect(ui.clip_rect()));
        match result {
            Ok(diagram) => {
                let image = egui::Image::from_bytes(
                    format!("bytes://diagram-{key}.svg"),
                    diagram.bytes.clone(),
                );
                image.paint_at(
                    ui,
                    egui::Rect::from_center_size(
                        rect.center() + state.pan,
                        size * state.scale.unwrap_or(fitted),
                    ),
                );
            }
            Err(error) => {
                let area = egui::Rect::from_min_size(
                    rect.min + egui::vec2(24.0, 52.0),
                    egui::vec2((rect.width() - 48.0).max(1.0), rect.height() - 76.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                    ui.label(
                        egui::RichText::new("Can’t draw this diagram")
                            .color(ui.visuals().error_fg_color)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(error).monospace().size(13.0));
                });
            }
        }
    });
    // Invisible buttons remain in the tab order, so focusing them reveals controls.
    let code = button(
        ui,
        rect,
        egui::vec2(-36.0, 8.0),
        0,
        "Edit code (Enter)",
        alpha,
    );
    let expand = button(
        ui,
        rect,
        egui::vec2(-70.0, 8.0),
        1,
        if fullscreen {
            "Close fullscreen"
        } else {
            "Fullscreen"
        },
        alpha,
    );
    edit |= code.clicked();
    if expand.clicked() {
        state.fullscreen = !state.fullscreen;
    }
    for (index, label) in ["Reset to fit (0)", "Zoom in (+)", "Zoom out (−)"]
        .iter()
        .enumerate()
    {
        let control = button(
            ui,
            rect,
            egui::vec2(-36.0, rect.height() - 104.0 + index as f32 * 32.0),
            index + 2,
            label,
            alpha,
        );
        if control.clicked() {
            match index {
                0 => state.reset(),
                1 => state.zoom(1.25, fitted, Vec2::ZERO),
                _ => state.zoom(0.8, fitted, Vec2::ZERO),
            }
        }
    }
    edit
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_preserves_cursor_anchor_and_fit_is_always_reachable() {
        let mut state = Canvas::default();
        let anchor = egui::vec2(80.0, -20.0);
        let point = anchor / 0.1;
        state.zoom(2.0, 0.1, anchor);
        assert!((point * state.scale.unwrap() + state.pan - anchor).length() < 0.001);
        state.zoom(0.001, 0.1, Vec2::ZERO);
        assert_eq!(state.scale, Some(0.1));
        state.reset();
        assert_eq!(state.pan, Vec2::ZERO);
        assert!(state.scale.is_none());
        assert_eq!(fit(egui::vec2(100.0, 50.0), egui::vec2(700.0, 480.0)), 1.0);
    }
    #[test]
    fn error_canvas_retains_height_and_source_button_works() {
        let ctx = egui::Context::default();
        let mut state = Canvas::default();
        let mut edit = false;
        let rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let start = ui.cursor().top();
                        let canvas = ui.scope(|ui| {
                            edit |=
                                show(ui, &Err("syntax error".into()), 0, &mut state, 480.0, false);
                        });
                        rect.set(canvas.response.rect);
                        assert_eq!(
                            ui.cursor().top() - start,
                            480.0 + ui.spacing().item_spacing.y
                        );
                    });
                },
            );
        };
        frame(vec![]);
        let pos = rect.get().right_top() + egui::vec2(-22.0, 22.0);
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        assert!(edit);
    }
}
