use super::*;

impl Notes {
    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let rect = ui.max_rect();
        sidebar_background(ui, rect, self.prefs.light, self.prefs.translucency);
        ui.add_space(13.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            if icon_button(ui, Icon::Sidebar, "Toggle sidebar · Ctrl+\\").clicked() {
                self.prefs.sidebar = false;
            }
            let drag = ui.allocate_response(vec2(ui.available_width(), 22.0), Sense::drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
        });
        ui.add_space(27.0);
        ui.horizontal(|ui| {
            ui.add_space(11.0);
            let w = ui.available_width() - 12.0;
            let (r, response) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    "Search notes and commands",
                )
            });
            ui.painter().rect_filled(
                r,
                8,
                if response.hovered() {
                    crate::theme::mix(self.background(), ui.visuals().text_color(), 0.13)
                } else {
                    crate::theme::mix(self.background(), ui.visuals().text_color(), 0.08)
                },
            );
            icon(
                ui,
                Icon::Search,
                pos2(r.left() + 18.0, r.center().y),
                MUTED,
                15.0,
            );
            ui.painter().text(
                pos2(r.left() + 36.0, r.center().y),
                Align2::LEFT_CENTER,
                "Search",
                crate::theme::ui_font(ui, 16.0),
                MUTED,
            );
            ui.painter().text(
                pos2(r.right() - 11.0, r.center().y),
                Align2::RIGHT_CENTER,
                "Ctrl P",
                crate::theme::ui_font(ui, 12.0),
                MUTED,
            );
            if response.on_hover_text("Find notes and commands").clicked() {
                self.show_palette();
            }
        });
        ui.add_space(19.0);
        let tree_height = (ui.available_height() - 46.0).max(20.0);
        egui::ScrollArea::vertical()
            .id_salt("file-tree")
            .max_height(tree_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.prefs.show_library {
                    self.library_notes(ui);
                }
                self.tree(ui, self.prefs.root.clone(), 0);
            });
        let library = Rect::from_min_size(
            pos2(rect.left() + 12.0, rect.bottom() - 38.0),
            vec2(rect.width() - 24.0, 29.0),
        );
        let settings_rect = Rect::from_center_size(
            pos2(library.right() - 14.0, library.center().y),
            vec2(28.0, 28.0),
        );
        let library_rect = Rect::from_min_max(
            library.min,
            pos2(settings_rect.left() - 8.0, library.bottom()),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(library_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                let response =
                    ui.add(egui::Button::new(RichText::new("Library").color(MUTED)).frame(false));
                response.context_menu(|ui| {
                    self.library_menu(ui);
                });
                if response.clicked() {
                    self.show_palette();
                    self.query = ">".into();
                }
            },
        );
        let response = ui.interact(settings_rect, Id::new("sidebar-settings"), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, self.settings, "Settings")
        });
        if self.settings || response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(settings_rect, 6, Color32::from_white_alpha(18));
        }
        icon(
            ui,
            Icon::Settings,
            settings_rect.center(),
            if self.settings {
                ui.visuals().text_color()
            } else {
                MUTED
            },
            18.0,
        );
        if response.on_hover_text("Settings · Ctrl+,").clicked() {
            self.settings = !self.settings;
        }
    }

    pub(super) fn library_menu(&mut self, ui: &mut Ui) {
        ui.label(self.prefs.root.display().to_string());
        ui.separator();
        if ui.button("Open workspace…").clicked() {
            self.open_workspace();
            ui.close();
        }
        if ui.button("Open note in new window…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown"])
                .pick_file()
                && let Err(e) = crate::windows::open(&path)
            {
                self.error = Some(e.to_string());
            }
            ui.close();
        }
        if ui.button("New note…").clicked() {
            self.dialog = Some(Dialog::NewNote);
            self.input.clear();
            ui.close();
        }
        if ui.button("New folder…").clicked() {
            self.dialog = Some(Dialog::NewFolder);
            self.input.clear();
            ui.close();
        }
        if ui.button("Open in File Explorer").clicked() {
            let _ = std::process::Command::new("explorer.exe")
                .arg(&self.prefs.root)
                .spawn();
            ui.close();
        }
        if ui.button("Settings").clicked() {
            self.settings = true;
            ui.close();
        }
    }

    fn library_notes(&mut self, ui: &mut Ui) {
        for (title, paths) in [
            ("Pinned", self.prefs.pinned.clone()),
            (
                "Recent",
                self.prefs
                    .recent
                    .iter()
                    .filter(|p| !self.prefs.pinned.contains(p))
                    .take(5)
                    .cloned()
                    .collect(),
            ),
        ] {
            if paths.is_empty() {
                continue;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(title).size(12.0).color(MUTED));
            });
            for path in paths {
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                let selected = self.docs.get(self.active).is_some_and(|d| d.path == path);
                let (rect, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        name.as_ref(),
                    )
                });
                if selected || response.hovered() {
                    ui.painter().rect_filled(
                        rect.shrink2(vec2(11.0, 0.5)),
                        8,
                        crate::theme::mix(
                            self.background(),
                            ui.visuals().text_color(),
                            if selected { 0.10 } else { 0.05 },
                        ),
                    );
                }
                let color = if selected {
                    ui.visuals().text_color()
                } else {
                    MUTED
                };
                icon(
                    ui,
                    Icon::File,
                    pos2(rect.left() + 30.0, rect.center().y),
                    color,
                    15.0,
                );
                ui.painter()
                    .with_clip_rect(rect.shrink2(vec2(12.0, 0.0)))
                    .text(
                        pos2(rect.left() + 46.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        name.as_ref(),
                        crate::theme::ui_font(ui, 14.0),
                        color,
                    );
                let response = response.on_hover_text(path.display().to_string());
                if response.clicked() {
                    self.open(path.clone(), true);
                }
                response.context_menu(|ui| {
                    if ui
                        .button(if self.prefs.pinned.contains(&path) {
                            "Unpin note"
                        } else {
                            "Pin note"
                        })
                        .clicked()
                    {
                        self.toggle_pin(&path);
                        ui.close();
                    }
                    if title == "Recent" && ui.button("Remove from recent").clicked() {
                        self.prefs.recent.retain(|p| p != &path);
                        ui.close();
                    }
                });
            }
        }
        ui.add_space(12.0);
    }

    pub(super) fn tree(&mut self, ui: &mut Ui, parent: PathBuf, depth: usize) {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.path.parent() == Some(parent.as_path()))
            .cloned()
            .collect();
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.path.cmp(&b.path))
        });
        for entry in entries {
            let selected = self.selected_paths.contains(&entry.path)
                || !self.settings
                    && self
                        .docs
                        .get(self.active)
                        .is_some_and(|d| d.path == entry.path);
            let open = self.expanded.contains(&entry.path);
            let label = if entry.directory {
                entry.path.file_name()
            } else {
                entry.path.file_stem()
            }
            .unwrap_or_default()
            .to_string_lossy();
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 33.0), Sense::click_and_drag());
            let payload: Vec<_> = if self.selected_paths.contains(&entry.path) {
                self.selected_paths.iter().cloned().collect()
            } else {
                vec![entry.path.clone()]
            };
            response.dnd_set_drag_payload(payload);
            let destination = if entry.directory {
                entry.path.clone()
            } else {
                entry.path.parent().unwrap_or(&self.prefs.root).to_owned()
            };
            if response.dnd_hover_payload::<Vec<PathBuf>>().is_some() {
                ui.painter().rect_stroke(
                    rect.shrink(2.0),
                    5,
                    Stroke::new(1.0_f32, self.accent()),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(paths) = response.dnd_release_payload::<Vec<PathBuf>>()
                && let Err(e) = self.move_selected(&paths, &destination)
            {
                self.error = Some(e.to_string());
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    selected,
                    &label,
                )
            });
            let row = rect.shrink2(vec2(11.0, 0.5));
            if selected || response.hovered() {
                ui.painter().rect_filled(
                    row,
                    8,
                    if selected {
                        Color32::from_rgba_unmultiplied(82, 87, 88, 70)
                    } else {
                        Color32::from_rgba_unmultiplied(80, 80, 80, 35)
                    },
                );
            }
            let color = if selected {
                ui.visuals().text_color()
            } else {
                MUTED
            };
            let x = rect.left() + 30.0 + depth as f32 * 8.0;
            icon(
                ui,
                if entry.directory {
                    if open { Icon::FolderOpen } else { Icon::Folder }
                } else {
                    Icon::File
                },
                pos2(x, rect.center().y),
                color,
                15.0,
            );
            let text_rect = Rect::from_min_max(
                pos2(x + 16.0, rect.top()),
                pos2(rect.right() - 14.0, rect.bottom()),
            );
            ui.painter().with_clip_rect(text_rect).text(
                pos2(x + 16.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                crate::theme::ui_font(ui, 14.0),
                color,
            );
            if response.clicked() {
                let modifiers = ui.input(|i| i.modifiers);
                self.select_path(&entry.path, modifiers);
                if !modifiers.shift && !modifiers.ctrl && !modifiers.command {
                    if entry.directory {
                        if open {
                            self.expanded.remove(&entry.path);
                        } else {
                            self.expanded.insert(entry.path.clone());
                        }
                    } else {
                        self.open(entry.path.clone(), true);
                    }
                }
            }
            response.context_menu(|ui| {
                if !entry.directory
                    && ui
                        .button(if self.prefs.pinned.contains(&entry.path) {
                            "Unpin note"
                        } else {
                            "Pin note"
                        })
                        .clicked()
                {
                    self.toggle_pin(&entry.path);
                    ui.close();
                }
                if ui.button("Move selected to…").clicked() {
                    let paths = if self.selected_paths.contains(&entry.path) {
                        self.selected_paths.iter().cloned().collect::<Vec<_>>()
                    } else {
                        vec![entry.path.clone()]
                    };
                    if let Some(destination) = rfd::FileDialog::new()
                        .set_directory(&self.prefs.root)
                        .pick_folder()
                        && let Err(e) = self.move_selected(&paths, &destination)
                    {
                        self.error = Some(e.to_string());
                    }
                    ui.close();
                }
                if ui.button("Rename…").clicked() {
                    self.input = entry
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    self.dialog = Some(Dialog::Rename(entry.path.clone()));
                    ui.close();
                }
                if !entry.directory && ui.button("Move to Archive").clicked() {
                    self.archive(&entry.path);
                    ui.close();
                }
                if entry.directory && ui.button("New note here…").clicked() {
                    self.input = format!(
                        "{}/",
                        entry
                            .path
                            .strip_prefix(&self.prefs.root)
                            .unwrap_or(&entry.path)
                            .display()
                    );
                    self.dialog = Some(Dialog::NewNote);
                    ui.close();
                }
                if ui.button("Reveal in File Explorer").clicked() {
                    let _ = std::process::Command::new("explorer.exe")
                        .arg(format!("/select,{}", entry.path.display()))
                        .spawn();
                    ui.close();
                }
                if !entry.directory {
                    ui.separator();
                    if ui
                        .button("Delete note")
                        .on_hover_text("Move to the Windows Recycle Bin")
                        .clicked()
                    {
                        self.delete_note(&entry.path);
                        ui.close();
                    }
                }
            });
            if entry.directory && open {
                self.tree(ui, entry.path, depth + 1);
            }
        }
        if depth == 0 {
            let empty = ui.allocate_response(
                vec2(ui.available_width(), ui.available_height().max(12.0)),
                Sense::click(),
            );
            empty.context_menu(|ui| self.library_menu(ui));
            if let Some(paths) = empty.dnd_release_payload::<Vec<PathBuf>>() {
                let root = self.prefs.root.clone();
                if let Err(e) = self.move_selected(&paths, &root) {
                    self.error = Some(e.to_string());
                }
            }
        }
    }

    pub(super) fn window_controls(&self, ui: &mut Ui, window: Rect) {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        let left = window.right() - 139.0;
        for (index, label) in [
            "Minimize",
            if maximized { "Restore" } else { "Maximize" },
            "Close",
        ]
        .into_iter()
        .enumerate()
        {
            let rect = Rect::from_min_size(
                pos2(left + index as f32 * 46.0, window.top() + 1.0),
                vec2(46.0, 33.0),
            );
            let response = ui.interact(rect, Id::new(("window-control", index)), Sense::click());
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            let fill = if response.hovered() {
                if index == 2 {
                    Color32::from_rgb(196, 43, 28)
                } else if self.prefs.light {
                    Color32::from_gray(215)
                } else {
                    Color32::from_gray(57)
                }
            } else {
                self.background()
            };
            ui.painter().rect_filled(rect, 0, fill);
            let color = if self.prefs.light && !(index == 2 && response.hovered()) {
                Color32::from_gray(40)
            } else {
                Color32::from_gray(235)
            };
            let stroke = Stroke::new(1.0_f32, color);
            let center = rect.center().round();
            match index {
                0 => {
                    ui.painter()
                        .line_segment([center - vec2(5.0, 0.0), center + vec2(5.0, 0.0)], stroke);
                }
                1 => {
                    let square = Rect::from_center_size(center, vec2(9.0, 9.0));
                    if maximized {
                        let back = square.translate(vec2(1.5, -1.5));
                        ui.painter()
                            .rect_stroke(back, 0, stroke, egui::StrokeKind::Inside);
                        let front = square.translate(vec2(-1.5, 1.5));
                        ui.painter().rect_filled(front, 0, fill);
                        ui.painter()
                            .rect_stroke(front, 0, stroke, egui::StrokeKind::Inside);
                    } else {
                        ui.painter()
                            .rect_stroke(square, 0, stroke, egui::StrokeKind::Inside);
                    }
                }
                _ => {
                    ui.painter()
                        .line_segment([center - vec2(4.5, 4.5), center + vec2(4.5, 4.5)], stroke);
                    ui.painter()
                        .line_segment([center + vec2(-4.5, 4.5), center + vec2(4.5, -4.5)], stroke);
                }
            }
            if response.on_hover_text(label).clicked() {
                ui.ctx().send_viewport_cmd(match index {
                    0 => ViewportCommand::Minimized(true),
                    1 => ViewportCommand::Maximized(!maximized),
                    _ => ViewportCommand::Close,
                });
            }
        }
    }

    pub(super) fn toolbar(&mut self, ui: &mut Ui) {
        if self.compact {
            let title = self
                .docs
                .get(self.active)
                .and_then(|doc| doc.path.file_name())
                .unwrap_or_default()
                .to_string_lossy();
            let response = ui.add_sized(
                [ui.available_width(), 38.0],
                egui::Label::new(title).sense(Sense::click_and_drag()),
            );
            if response.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
            return;
        }
        ui.visuals_mut().selection.bg_fill =
            crate::theme::mix(self.background(), ui.visuals().text_color(), 0.09);
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            if !self.prefs.sidebar && icon_button(ui, Icon::Sidebar, "Show sidebar").clicked() {
                self.prefs.sidebar = true;
            }
            if icon_button(ui, Icon::Back, "Back · Alt+Left").clicked() {
                self.navigate(-1);
            }
            if icon_button(ui, Icon::Forward, "Forward · Alt+Right").clicked() {
                self.navigate(1);
            }
            let mut close = None;
            let mut activate = None;
            let mut delete = None;
            let mut pin = None;
            let mut detach = None;
            let active_path = self.docs.get(self.active).map(|d| d.path.clone());
            let reveal_id = Id::new("revealed-active-tab");
            let reveal = ui
                .ctx()
                .data(|data| data.get_temp::<Option<PathBuf>>(reveal_id))
                != Some(active_path.clone());
            ui.ctx()
                .data_mut(|data| data.insert_temp(reveal_id, active_path));
            ui.scope(|ui| {
                // Treat a normal mouse wheel as horizontal scrolling only over tabs.
                ui.style_mut().always_scroll_the_only_direction = true;
                egui::ScrollArea::horizontal()
                    .id_salt("tabs")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .max_width((ui.available_width() - 90.0).max(40.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, doc) in self.docs.iter().enumerate() {
                                let mut title = doc
                                    .path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                if doc.dirty() {
                                    title.push_str(" ·");
                                }
                                let response = ui.add(
                                    egui::Button::new(RichText::new(title).size(13.5).color(
                                        if index == self.active && !self.settings {
                                            ui.visuals().text_color()
                                        } else {
                                            MUTED
                                        },
                                    ))
                                    .selected(index == self.active && !self.settings)
                                    .frame(index == self.active && !self.settings)
                                    .corner_radius(8)
                                    .min_size(vec2(50.0, 32.0))
                                    .sense(Sense::click_and_drag()),
                                );
                                if reveal && index == self.active {
                                    response.scroll_to_me(Some(Align::Center));
                                }
                                if response.drag_stopped()
                                    && ui.input(|i| {
                                        i.pointer.latest_pos().is_some_and(|p| {
                                            (p.y - response.rect.center().y).abs() > 60.0
                                        })
                                    })
                                {
                                    detach = Some(doc.path.clone());
                                }
                                if response.clicked() {
                                    activate = Some(index);
                                }
                                if response.clicked_by(egui::PointerButton::Middle) {
                                    close = Some(index);
                                }
                                response.context_menu(|ui| {
                                    if ui
                                        .button(if self.prefs.pinned.contains(&doc.path) {
                                            "Unpin note"
                                        } else {
                                            "Pin note"
                                        })
                                        .clicked()
                                    {
                                        pin = Some(doc.path.clone());
                                        ui.close();
                                    }
                                    if ui.button("Move tab to new window").clicked() {
                                        detach = Some(doc.path.clone());
                                        ui.close();
                                    }
                                    if ui.button("Close tab · Ctrl+W").clicked() {
                                        close = Some(index);
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .button("Delete note")
                                        .on_hover_text("Move to the Windows Recycle Bin")
                                        .clicked()
                                    {
                                        delete = Some(doc.path.clone());
                                        ui.close();
                                    }
                                });
                            }
                        });
                    });
            });
            if let Some(index) = activate {
                self.active = index;
                self.settings = false;
                self.read_note();
            }
            if let Some(index) = close {
                self.close_tab(index);
            }
            if let Some(path) = detach {
                self.detach_tab(&path);
            }
            if let Some(path) = pin {
                self.toggle_pin(&path);
            }
            if let Some(path) = delete {
                self.delete_note(&path);
            }
            if icon_button(ui, Icon::Plus, "New tab · Ctrl+T").clicked() {
                self.show_palette();
            }
            let drag =
                ui.allocate_response(vec2(ui.available_width().max(0.0), 38.0), Sense::drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(
                    !ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)),
                ));
            }
        });
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Folder,
    FolderOpen,
    File,
    Search,
    Sidebar,
    Back,
    Forward,
    Plus,
    Settings,
}
fn icon_button(ui: &mut Ui, kind: Icon, tip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 26.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5, Color32::from_white_alpha(14));
    }
    icon(ui, kind, rect.center(), MUTED, 14.0);
    response.on_hover_text(tip)
}
fn icon(ui: &Ui, kind: Icon, c: Pos2, color: Color32, size: f32) {
    let p = ui.painter();
    let s = size / 16.0;
    let stroke = Stroke::new(1.3_f32, color);
    let pt = |x: f32, y: f32| c + vec2(x * s, y * s);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], stroke);
    };
    match kind {
        Icon::Folder | Icon::FolderOpen => {
            let points = vec![
                pt(-7.0, 5.5),
                pt(-7.0, -5.0),
                pt(-2.0, -5.0),
                pt(0.0, -2.5),
                pt(7.0, -2.5),
                pt(7.0, 5.5),
                pt(-7.0, 5.5),
            ];
            p.add(egui::Shape::line(points, stroke));
            if matches!(kind, Icon::FolderOpen) {
                line(pt(-6.0, 1.0), pt(7.0, -0.5));
            }
        }
        Icon::File => {
            p.add(egui::Shape::line(
                vec![
                    pt(-5.0, -7.0),
                    pt(1.0, -7.0),
                    pt(5.0, -3.0),
                    pt(5.0, 7.0),
                    pt(-5.0, 7.0),
                    pt(-5.0, -7.0),
                ],
                stroke,
            ));
            line(pt(0.0, -7.0), pt(0.0, -2.0));
            line(pt(0.0, -2.0), pt(5.0, -2.0));
            line(pt(-2.5, 1.0), pt(2.5, 1.0));
            line(pt(-2.5, 4.0), pt(1.0, 4.0));
        }
        Icon::Search => {
            p.circle_stroke(pt(-1.0, -1.0), 5.5 * s, stroke);
            line(pt(3.0, 3.0), pt(7.0, 7.0));
        }
        Icon::Sidebar => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(16.0 * s, 14.0 * s)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(pt(-3.0, -7.0), pt(-3.0, 7.0));
        }
        Icon::Back | Icon::Forward => {
            let sign = if matches!(kind, Icon::Back) {
                1.0
            } else {
                -1.0
            };
            line(pt(-5.0 * sign, 0.0), pt(6.0 * sign, 0.0));
            line(pt(-5.0 * sign, 0.0), pt(0.0, -5.0));
            line(pt(-5.0 * sign, 0.0), pt(0.0, 5.0));
        }
        Icon::Plus => {
            line(pt(-5.0, 0.0), pt(5.0, 0.0));
            line(pt(0.0, -5.0), pt(0.0, 5.0));
        }
        Icon::Settings => {
            let outline = (0..48)
                .map(|n| {
                    let a = n as f32 * std::f32::consts::TAU / 48.0;
                    let radius = if matches!(n % 6, 1..=3) { 7.5 } else { 5.8 };
                    c + vec2(a.cos(), a.sin()) * radius * s
                })
                .collect();
            p.add(egui::Shape::closed_line(outline, stroke));
            p.circle_stroke(c, 2.5 * s, stroke);
        }
    }
}

fn sidebar_background(ui: &Ui, rect: Rect, light: bool, translucency: f32) {
    let mut mesh = egui::Mesh::default();
    let opacity = (translucency / 100.0).clamp(0.0, 1.0);
    let colors = if light {
        [
            Color32::from_rgb(224, 229, 230),
            Color32::from_rgb(237, 225, 215),
        ]
    } else {
        [
            Color32::from_rgb(23, 29, 31),
            Color32::from_rgb((24.0 + 30.0 * opacity) as u8, 25, 20),
        ]
    };
    mesh.colored_vertex(rect.left_top(), colors[0]);
    mesh.colored_vertex(rect.right_top(), colors[0]);
    mesh.colored_vertex(rect.right_bottom(), colors[1]);
    mesh.colored_vertex(rect.left_bottom(), colors[1]);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}
