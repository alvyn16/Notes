use eframe::egui::{self, Id, RichText, Ui, Vec2};
use std::{path::Path, sync::Arc};

#[derive(Clone)]
struct Viewer {
    uri: String,
    size: Vec2,
    zoom: f32,
    pan: Vec2,
}
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Viewer>(Id::new("media-viewer")).is_some())
}
#[cfg(debug_assertions)]
pub fn capture_viewer(ctx: &egui::Context) {
    ctx.include_bytes(
        "bytes://capture-notes.png",
        include_bytes!("../assets/notes.png").as_slice(),
    );
    ctx.data_mut(|d| {
        d.insert_temp(
            Id::new("media-viewer"),
            Viewer {
                uri: "bytes://capture-notes.png".into(),
                size: egui::vec2(512.0, 512.0),
                zoom: 1.0,
                pan: Vec2::ZERO,
            },
        )
    });
}
pub fn show(ui: &mut Ui, target: &str, location: (&Path, &Path)) -> bool {
    let id = Id::new(("image-resolution", location, target));
    let uri = if target.starts_with("https://") || target.starts_with("http://") {
        Some(target.to_owned())
    } else {
        let cached = ui
            .ctx()
            .data(|d| d.get_temp::<(std::time::Instant, Option<String>)>(id));
        if let Some((_, uri)) = cached.filter(|(time, _)| time.elapsed().as_secs_f32() < 2.0) {
            uri
        } else {
            let paths = ui
                .ctx()
                .data(|d| d.get_temp::<Arc<Vec<std::path::PathBuf>>>(Id::new("workspace-files")))
                .unwrap_or_default();
            let uri = crate::links::resolve_indexed(location.0, location.1, target, false, &paths)
                .map(|p| format!("file://{}", p.display()));
            ui.ctx()
                .data_mut(|d| d.insert_temp(id, (std::time::Instant::now(), uri.clone())));
            uri
        }
    };
    let mut edit = false;
    let limit = egui::vec2(ui.available_width().max(1.0), 480.0);
    let result = uri
        .as_ref()
        .map(|uri| egui::Image::new(uri).load_for_size(ui.ctx(), limit));
    match result {
        Some(Ok(egui::load::TexturePoll::Ready { texture })) => {
            let size = texture.size
                * (limit.x / texture.size.x)
                    .min(limit.y / texture.size.y)
                    .min(1.0);
            let response = ui
                .add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(size)
                        .sense(egui::Sense::click()),
                )
                .on_hover_text("Click to enlarge · Right-click to edit source");
            if response.clicked() {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(
                        Id::new("media-viewer"),
                        Viewer {
                            uri: uri.clone().unwrap(),
                            size: texture.size,
                            zoom: 1.0,
                            pan: Vec2::ZERO,
                        },
                    )
                });
            }
            response.context_menu(|ui| {
                if ui.button("Edit image source").clicked() {
                    edit = true;
                    ui.close();
                }
                if ui.button("Reload image").clicked() {
                    if let Some(uri) = &uri {
                        ui.ctx().forget_image(uri);
                    }
                    ui.close();
                }
            });
        }
        Some(Ok(egui::load::TexturePoll::Pending { .. })) => {
            egui::Frame::new()
                .fill(ui.visuals().code_bg_color)
                .corner_radius(8)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2((limit.x - 32.0).max(1.0), 100.0));
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading image…");
                    });
                    ui.label(RichText::new(target).small().weak());
                });
        }
        error => {
            let message = match error {
                Some(Err(error)) => error.to_string(),
                _ => "The image could not be found in this workspace.".into(),
            };
            egui::Frame::new()
                .fill(ui.visuals().code_bg_color)
                .stroke(ui.visuals().window_stroke)
                .corner_radius(8)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.label(RichText::new("Image unavailable").strong());
                    ui.label(RichText::new(target).small().weak());
                    ui.label(RichText::new(message).small().weak());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Retry").clicked() {
                            if let Some(uri) = &uri {
                                ui.ctx().forget_image(uri);
                            }
                            ui.ctx()
                                .data_mut(|d| d.remove::<(std::time::Instant, Option<String>)>(id));
                            ui.ctx().request_repaint();
                        }
                        if ui.button("Edit source").clicked() {
                            edit = true;
                        }
                    });
                });
        }
    }
    edit
}
pub fn viewer(ctx: &egui::Context) {
    let id = Id::new("media-viewer");
    let Some(mut state) = ctx.data(|d| d.get_temp::<Viewer>(id)) else {
        return;
    };
    let screen = ctx.content_rect().shrink(12.0);
    let mut close = false;
    let response = egui::Modal::new(Id::new("image-fullscreen"))
        .area(egui::Area::new(Id::new("image-fullscreen-area")).fixed_pos(screen.min))
        .frame(
            egui::Frame::new()
                .fill(ctx.style().visuals.panel_fill)
                .corner_radius(10)
                .inner_margin(12),
        )
        .show(ctx, |ui| {
            ui.set_width((screen.width() - 24.0).max(1.0));
            ui.horizontal(|ui| {
                ui.label("Image");
                if ui.button("−").on_hover_text("Zoom out").clicked() {
                    state.zoom = (state.zoom / 1.25).max(0.25);
                }
                if ui.button("+").on_hover_text("Zoom in").clicked() {
                    state.zoom = (state.zoom * 1.25).min(8.0);
                }
                if ui.button("Fit").clicked() {
                    state.zoom = 1.0;
                    state.pan = Vec2::ZERO;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close · Esc").clicked() {
                        close = true;
                    }
                });
            });
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), (screen.height() - 80.0).max(1.0)),
                egui::Sense::click_and_drag(),
            );
            if response.dragged() {
                state.pan += ui.input(|i| i.pointer.delta());
            }
            if response.hovered() {
                state.zoom = (state.zoom * ui.input(|i| i.zoom_delta())).clamp(0.25, 8.0);
                response.on_hover_cursor(egui::CursorIcon::Grab);
            }
            let fit = (rect.width() / state.size.x)
                .min(rect.height() / state.size.y)
                .min(1.0);
            let mut painter = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            painter.set_clip_rect(rect);
            egui::Image::new(&state.uri).paint_at(
                &painter,
                egui::Rect::from_center_size(
                    rect.center() + state.pan,
                    state.size * fit * state.zoom,
                ),
            );
        });
    if close || response.should_close() {
        ctx.data_mut(|d| d.remove::<Viewer>(id));
    } else {
        ctx.data_mut(|d| d.insert_temp(id, state));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        }
    }
    fn click(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    fn text_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn unavailable_image_has_an_edit_source_action() {
        let ctx = egui::Context::default();
        let mut edit = false;
        let mut frame = |events| {
            ctx.run(input(events), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    edit |= show(ui, "missing.png", (Path::new("."), Path::new("note.md")));
                });
            })
        };
        frame(vec![]);
        let output = frame(vec![]);
        let pos = text_position(&output, "Edit source");
        frame(click(pos, true));
        frame(click(pos, false));
        assert!(edit);
    }
    #[test]
    fn fullscreen_image_zooms_and_escape_closes_it() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.include_bytes(
            "bytes://viewer.png",
            include_bytes!("../assets/notes.png").as_slice(),
        );
        ctx.data_mut(|d| {
            d.insert_temp(
                Id::new("media-viewer"),
                Viewer {
                    uri: "bytes://viewer.png".into(),
                    size: egui::vec2(512.0, 512.0),
                    zoom: 1.0,
                    pan: Vec2::ZERO,
                },
            )
        });
        let frame = |events| ctx.run(input(events), viewer);
        frame(vec![]);
        let output = frame(vec![]);
        let pos = text_position(&output, "+");
        frame(click(pos, true));
        frame(click(pos, false));
        assert!(ctx.data(|d| d.get_temp::<Viewer>(Id::new("media-viewer")).unwrap().zoom) > 1.0);
        frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(!is_open(&ctx));
    }
}
