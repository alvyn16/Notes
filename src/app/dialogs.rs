use super::*;

impl Notes {
    pub(super) fn dialog_ui(&mut self, ctx: &egui::Context) {
        let dialog = self.dialog.clone().unwrap();
        let title = match &dialog {
            Dialog::NewNote => "Create a new note",
            Dialog::NewFolder => "Create a folder",
            Dialog::Rename(_) => "Rename",
        };
        let mut submit = false;
        let mut cancel = false;
        let bg = self.background();
        let fg = ctx.style().visuals.text_color();
        let frame = Frame::new()
            .fill(controls::blend(bg, fg, 0.025))
            .stroke(Stroke::new(1.0_f32, controls::blend(bg, fg, 0.12)))
            .corner_radius(12)
            .inner_margin(Margin::same(24))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 32,
                spread: 2,
                color: Color32::from_black_alpha(65),
            });
        let response = egui::Modal::new(Id::new("file-dialog"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 96.0).clamp(220.0, 376.0));
                ui.label(RichText::new(title).size(19.0));
                ui.add_space(5.0);
                let description = match &dialog {
                    Dialog::NewNote => "Give your next thought a place to live.",
                    Dialog::NewFolder => "A little order for your notes.",
                    Dialog::Rename(_) => "Choose a new name for this item.",
                };
                ui.label(
                    RichText::new(description)
                        .size(13.0)
                        .color(controls::blend(bg, fg, 0.6)),
                );
                ui.add_space(22.0);
                let label = match &dialog {
                    Dialog::NewNote => "Note name",
                    Dialog::NewFolder => "Folder name",
                    Dialog::Rename(_) => "Name",
                };
                ui.label(
                    RichText::new(label)
                        .size(12.0)
                        .color(controls::blend(bg, fg, 0.75)),
                );
                ui.add_space(4.0);
                let input_id = Id::new("file-dialog-name");
                let focused = ctx.memory(|m| m.has_focus(input_id));
                let field = Frame::new()
                    .fill(controls::blend(bg, fg, 0.045))
                    .stroke(Stroke::new(
                        1.0_f32,
                        if focused {
                            controls::blend(bg, self.accent(), 0.65)
                        } else {
                            controls::blend(bg, fg, 0.16)
                        },
                    ))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(12, 10));
                let response = field
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .id(input_id)
                                .frame(false)
                                .font(FontId::new(
                                    14.0,
                                    FontFamily::Name(self.prefs.ui_font.clone().into()),
                                ))
                                .margin(Margin::ZERO)
                                .desired_width(f32::INFINITY)
                                .hint_text(match &dialog {
                                    Dialog::NewNote => "Untitled note",
                                    Dialog::NewFolder => "New folder",
                                    Dialog::Rename(_) => "New name",
                                }),
                        )
                    })
                    .inner;
                // Focus only when this dialog opens, leaving its buttons keyboard-accessible.
                let focus_id = Id::new("file-dialog-initial-focus");
                if !ctx.data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false)) {
                    response.request_focus();
                    ctx.data_mut(|d| d.insert_temp(focus_id, true));
                }
                ui.add_space(7.0);
                ui.label(
                    RichText::new(match &dialog {
                        Dialog::NewNote => "Saved as a Markdown file in your workspace.",
                        Dialog::NewFolder => "Created inside your current workspace.",
                        Dialog::Rename(_) => "Your note’s contents stay the same.",
                    })
                    .size(11.5)
                    .color(controls::blend(bg, fg, 0.50)),
                );
                ui.add_space(24.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let action = match &dialog {
                        Dialog::NewNote => "Create note",
                        Dialog::NewFolder => "Create folder",
                        Dialog::Rename(_) => "Rename",
                    };
                    if controls::action(ui, action, true, !self.input.trim().is_empty(), 104.0)
                        .clicked()
                    {
                        submit = true;
                    }
                    ui.add_space(2.0);
                    if controls::action(ui, "Cancel", false, true, 76.0).clicked() {
                        cancel = true;
                    }
                });
                if response.has_focus()
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
                {
                    submit = true;
                }
            });
        if response.should_close() || cancel {
            self.dialog = None;
        }
        if self.dialog.is_none() || submit && !self.input.trim().is_empty() {
            ctx.data_mut(|d| d.remove::<bool>(Id::new("file-dialog-initial-focus")));
        }
        if submit && !self.input.trim().is_empty() {
            let result = match dialog {
                Dialog::NewNote => workspace::create(&self.prefs.root, &self.input, false),
                Dialog::NewFolder => workspace::create(&self.prefs.root, &self.input, true),
                Dialog::Rename(ref old) => {
                    if self.defer_until_saved(file_io::Deferred::Rename(
                        old.clone(),
                        self.input.clone(),
                    )) {
                        return;
                    }
                    if !self.save_all() {
                        return;
                    }
                    self.rename_note(old, &self.input.clone())
                }
            };
            match result {
                Ok(path) => {
                    self.dialog = None;
                    self.refresh_entries();
                    if path.is_file() {
                        if let Some(parent) = path.parent() {
                            self.expanded.insert(parent.to_owned());
                        }
                        self.open(path, true);
                    }
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
}
