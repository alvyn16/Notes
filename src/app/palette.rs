use super::*;
const COMMANDS: [&str; 16] = [
    "Create New File",
    "Close Current Tab",
    "Close All Tabs",
    "Open Workspace",
    "Close Workspace",
    "Toggle Dark Mode",
    "Toggle Sidebar",
    "Settings",
    "Zoom In",
    "Zoom Out",
    "Reset Zoom",
    "Create New Folder",
    "Toggle Preview",
    "Today's Daily Note",
    "Pin or Unpin Current Note",
    "Show Pinned and Recent Notes",
];

impl Notes {
    pub(super) fn palette_ui(&mut self, ctx: &egui::Context) {
        let mut selected = None;
        let commands = COMMANDS;

        let q = self.query.trim_start_matches('>').trim().to_lowercase();
        let command_only = self.query.starts_with('>');
        if self.search_query != q {
            self.search_query = q.clone();
            self.palette_cache.clear();
            if let Some(index) = &self.index {
                index.search(q.clone());
            }
        }
        let mut results: Vec<(String, String, Option<PathBuf>)> = Vec::new();
        if !command_only && q.is_empty() {
            for path in self
                .prefs
                .pinned
                .iter()
                .chain(
                    self.prefs
                        .recent
                        .iter()
                        .filter(|p| !self.prefs.pinned.contains(p)),
                )
                .take(25)
            {
                results.push((
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    format!(
                        "{} - {}",
                        if self.prefs.pinned.contains(path) {
                            "Pinned"
                        } else {
                            "Recent"
                        },
                        path.strip_prefix(&self.prefs.root)
                            .unwrap_or(path)
                            .display()
                    ),
                    Some(path.clone()),
                ));
            }
        }
        if !command_only && !q.is_empty() {
            for (path, content) in &self.palette_cache {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                results.push((
                    name,
                    format!(
                        "{}  {}",
                        path.strip_prefix(&self.prefs.root)
                            .unwrap_or(path)
                            .display(),
                        content
                    ),
                    Some(path.clone()),
                ));
            }
            results.sort_by_key(|(name, _, _)| !name.to_lowercase().contains(&q));
        }

        for command in commands {
            if q.is_empty() || command.to_lowercase().contains(&q) {
                results.push((command.into(), "Command".into(), None));
            }
        }

        self.palette_index = self.palette_index.min(results.len().saturating_sub(1));
        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
        if down && !results.is_empty() {
            self.palette_index = (self.palette_index + 1) % results.len();
        }
        if up && !results.is_empty() {
            self.palette_index = (self.palette_index + results.len() - 1) % results.len();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.palette = false;
            return;
        }

        let is_light = self.prefs.light;
        let card_bg = if is_light {
            Color32::from_rgb(250, 250, 250)
        } else {
            Color32::from_rgb(22, 22, 22)
        };
        let border_color = if is_light {
            Color32::from_rgba_unmultiplied(0, 0, 0, 25)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 20)
        };
        let line_color = if is_light {
            Color32::from_rgba_unmultiplied(0, 0, 0, 15)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 12)
        };

        let screen = ctx.content_rect();
        let top = 80.0_f32.min(screen.height() * 0.1);
        let area =
            egui::Area::new(Id::new("palette-area")).anchor(Align2::CENTER_TOP, vec2(0.0, top));
        let response = egui::Modal::new(Id::new("palette"))
            .area(area)
            .backdrop_color(Color32::from_black_alpha(100))
            .frame(
                Frame::new()
                    .fill(card_bg)
                    .stroke(Stroke::new(1.0_f32, border_color))
                    .corner_radius(16)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.set_width(568.0_f32.min(screen.width() - 32.0).max(120.0));
                let hint = RichText::new("Search...").size(14.0).color(if is_light {
                    Color32::from_gray(140)
                } else {
                    Color32::from_gray(105)
                });
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(hint)
                        .font(crate::theme::ui_font(ui, 14.0))
                        .text_color(if is_light {
                            Color32::from_gray(30)
                        } else {
                            Color32::from_gray(230)
                        })
                        .frame(false)
                        .desired_width(f32::INFINITY)
                        .margin(Margin {
                            left: 18,
                            right: 18,
                            top: 14,
                            bottom: 14,
                        }),
                );
                if self.focus_palette {
                    response.request_focus();
                    self.focus_palette = false;
                }
                if response.changed() {
                    self.palette_index = 0;
                }

                let line_y = ui.cursor().top();
                ui.painter().line_segment(
                    [
                        pos2(ui.max_rect().left(), line_y),
                        pos2(ui.max_rect().right(), line_y),
                    ],
                    Stroke::new(1.0_f32, line_color),
                );

                ui.add_space(6.0);
                egui::Frame::new()
                    .inner_margin(Margin {
                        left: 6,
                        right: 6,
                        top: 0,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                        egui::ScrollArea::vertical()
                            .id_salt("palette-results")
                            .max_height(324.0_f32.min(screen.height() - top - 80.0).max(58.0))
                            .show(ui, |ui| {
                                if results.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(RichText::new("No results found.").size(13.0).color(
                                        if is_light {
                                            Color32::from_gray(140)
                                        } else {
                                            Color32::from_gray(120)
                                        },
                                    ));
                                    ui.add_space(10.0);
                                }
                                for (index, (title, subtitle, _)) in results.iter().enumerate() {
                                    let is_selected = index == self.palette_index;
                                    let (r, item_response) = ui.allocate_exact_size(
                                        vec2(ui.available_width(), 58.0),
                                        Sense::click(),
                                    );
                                    item_response.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::Button,
                                            true,
                                            format!("{title}, {subtitle}"),
                                        )
                                    });
                                    let painter =
                                        ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
                                    if item_response.hovered()
                                        && ctx.input(|i| i.pointer.delta() != Vec2::ZERO)
                                    {
                                        self.palette_index = index;
                                    }
                                    if is_selected {
                                        let sel_fill = if is_light {
                                            Color32::from_rgba_unmultiplied(0, 0, 0, 16)
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 18)
                                        };
                                        ui.painter().rect_filled(r, 10, sel_fill);
                                    } else if item_response.hovered() {
                                        let hover_fill = if is_light {
                                            Color32::from_rgba_unmultiplied(0, 0, 0, 8)
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 9)
                                        };
                                        ui.painter().rect_filled(r, 10, hover_fill);
                                    }
                                    let title_color = if is_light {
                                        if is_selected {
                                            Color32::from_gray(10)
                                        } else {
                                            Color32::from_gray(50)
                                        }
                                    } else {
                                        if is_selected {
                                            Color32::from_gray(245)
                                        } else {
                                            Color32::from_gray(215)
                                        }
                                    };
                                    let subtitle_color = if is_light {
                                        if is_selected {
                                            Color32::from_gray(110)
                                        } else {
                                            Color32::from_gray(130)
                                        }
                                    } else {
                                        if is_selected {
                                            Color32::from_gray(140)
                                        } else {
                                            Color32::from_gray(120)
                                        }
                                    };
                                    painter.text(
                                        r.min + vec2(12.0, 10.0),
                                        Align2::LEFT_TOP,
                                        title,
                                        crate::theme::ui_font(ui, 14.0),
                                        title_color,
                                    );
                                    painter.text(
                                        r.min + vec2(12.0, 31.0),
                                        Align2::LEFT_TOP,
                                        subtitle,
                                        crate::theme::ui_font(ui, 12.0),
                                        subtitle_color,
                                    );
                                    if item_response.clicked() {
                                        selected = Some(index);
                                    }
                                    if is_selected && (down || up || response.changed()) {
                                        ui.scroll_to_rect(r, None);
                                    }
                                }
                            });
                    });
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
                    && !results.is_empty()
                {
                    selected = Some(self.palette_index);
                }
            });
        if response.should_close() {
            self.palette = false;
        }
        if let Some(index) = selected {
            self.palette = false;
            let (title, _, path) = results[index].clone();
            if let Some(path) = path {
                self.open(path, true);
            } else {
                match title.as_str() {
                    "Pin or Unpin Current Note" => {
                        if let Some(doc) = self.docs.get(self.active) {
                            self.toggle_pin(&doc.path.clone());
                        }
                    }
                    "Show Pinned and Recent Notes" => self.prefs.show_library = true,
                    "Toggle Sidebar" => self.prefs.sidebar = !self.prefs.sidebar,
                    "Create New File" => {
                        self.dialog = Some(Dialog::NewNote);
                        self.input.clear();
                    }
                    "Create New Folder" => {
                        self.dialog = Some(Dialog::NewFolder);
                        self.input.clear();
                    }
                    "Close Current Tab" => {
                        if !self.docs.is_empty() {
                            self.close_tab(self.active);
                        }
                    }
                    "Close All Tabs" => self.close_all(false),
                    "Open Workspace" => self.open_workspace(),
                    "Close Workspace" => self.close_all(true),
                    "Toggle Dark Mode" => {
                        self.prefs.light = !self.prefs.light;
                    }
                    "Settings" => self.settings = true,
                    "Zoom In" => {
                        self.prefs.zoom = (self.prefs.zoom + 0.1).min(3.0);
                    }
                    "Zoom Out" => {
                        self.prefs.zoom = (self.prefs.zoom - 0.1).max(0.5);
                    }
                    "Reset Zoom" => {
                        self.prefs.zoom = 1.0;
                    }
                    "Toggle Preview" => {
                        self.toggle_editor_mode();
                    }
                    "Today's Daily Note" => {
                        let folder = self.prefs.root.join("Daily Notes");
                        match fs::create_dir_all(&folder) {
                            Ok(()) => {
                                let name =
                                    chrono::Local::now().format("%A, %B %-d, %Y.md").to_string();
                                let path = folder.join(&name);
                                if !path.exists()
                                    && let Err(e) = workspace::create(
                                        &self.prefs.root,
                                        &format!("Daily Notes/{name}"),
                                        false,
                                    )
                                {
                                    self.error = Some(e.to_string());
                                    return;
                                }
                                self.expanded.insert(folder);
                                self.refresh_entries();
                                self.open(path, true);
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::tests::notes;

    fn frame(ctx: &egui::Context, notes: &mut Notes, key: Option<Key>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(620.0, 420.0))),
                events: key
                    .map(|key| egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            |ctx| notes.palette_ui(ctx),
        )
    }

    #[test]
    fn keyboard_selection_scrolls_into_view_and_wraps() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
        let mut notes = notes();
        frame(&ctx, &mut notes, None);
        frame(&ctx, &mut notes, None);
        frame(&ctx, &mut notes, Some(Key::ArrowUp));
        assert_eq!(notes.palette_index, COMMANDS.len() - 1);
        // A subsequent frame applies the scroll target.
        frame(&ctx, &mut notes, None);
        let output = frame(&ctx, &mut notes, None);
        assert!(
            output.shapes.iter().any(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    text.galley.text() == *COMMANDS.last().unwrap()
                        && shape.clip_rect.contains(text.pos + vec2(1.0, 7.0))
                } else {
                    false
                }
            }),
            "keyboard-selected last row must be visible"
        );
        frame(&ctx, &mut notes, Some(Key::ArrowDown));
        assert_eq!(notes.palette_index, 0);
    }

    #[test]
    fn filtered_command_executes_and_escape_dismisses() {
        let ctx = egui::Context::default();
        let mut notes = notes();
        notes.query = ">Toggle Sidebar".into();
        frame(&ctx, &mut notes, None);
        frame(&ctx, &mut notes, Some(Key::Enter));
        assert!(!notes.prefs.sidebar);
        assert!(!notes.palette);
        notes.palette = true;
        notes.query = "no matching command".into();
        frame(&ctx, &mut notes, Some(Key::Enter));
        assert!(notes.palette);
        frame(&ctx, &mut notes, Some(Key::Escape));
        assert!(!notes.palette);
    }
}
