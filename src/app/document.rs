use super::*;

impl Notes {
    pub(super) fn document(&mut self, ui: &mut Ui) {
        if self.docs.is_empty() {
            ui.add_space((ui.available_height() * 0.30).max(48.0));
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("A little space to think.").size(23.0));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Start a new note, or open a workspace.")
                        .size(13.0)
                        .color(controls::blend(
                            ui.visuals().panel_fill,
                            ui.visuals().text_color(),
                            0.60,
                        )),
                );
                ui.add_space(26.0);
                if controls::action(ui, "Create a note", true, true, 190.0).clicked() {
                    self.dialog = Some(Dialog::NewNote);
                    self.input.clear();
                }
                ui.add_space(4.0);
                if controls::action(ui, "Open workspace…", false, true, 190.0).clicked() {
                    self.open_workspace();
                }
            });
            return;
        }
        if self.io.loading.contains(&self.docs[self.active].path)
            || self.io.resolving.contains(&self.docs[self.active].path)
        {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.spinner();
                ui.label(
                    if self.io.resolving.contains(&self.docs[self.active].path) {
                        "Resolving conflict..."
                    } else {
                        "Opening note..."
                    },
                );
            });
            return;
        }
        if self.docs[self.active].conflict {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    self.accent(),
                    "This note changed on disk. Your draft is preserved.",
                );
                if ui
                    .button("Keep disk version")
                    .on_hover_text("Discard this draft and keep the version on disk")
                    .clicked()
                {
                    if self.io.worker.is_some() {
                        self.resolve_background(file_io::Resolution::Disk);
                    } else {
                        match self.docs[self.active].resolve_disk() {
                            Ok(()) => {
                                self.editor
                                    .reset_external(ui.ctx(), &self.docs[self.active].path);
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                }
                if ui
                    .button("Keep my draft")
                    .on_hover_text("Replace the disk version with this draft")
                    .clicked()
                {
                    if self.io.worker.is_some() {
                        self.resolve_background(file_io::Resolution::Draft);
                    } else {
                        match self.docs[self.active].resolve_draft() {
                            Ok(()) => {
                                self.error = None;
                                self.refresh_entries();
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                }
                if ui.button("Save copy and close…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("Recovered note.md")
                        .save_file()
                {
                    if self.io.worker.is_some() {
                        self.resolve_background(file_io::Resolution::Copy(path));
                    } else if let Err(e) = self.save_conflict_copy(&path) {
                        self.error = Some(e.to_string());
                    }
                }
            });
        }
        if self.docs.is_empty() {
            ui.ctx().request_repaint();
            return;
        }
        if self.io.resolving.contains(&self.docs[self.active].path) {
            return;
        }
        let path = self.docs[self.active].path.clone();
        markdown::prepare_document(ui.ctx(), &path, &self.docs[self.active].text);
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new("workspace-root"), self.prefs.root.clone()));
        #[cfg(debug_assertions)]
        if crate::qa::prepare(ui.ctx(), &path, &self.docs[self.active].text) {
            self.view.editing = true;
            self.editor.begin_at(ui.ctx(), &path, 0);
        }
        let finding = self.editor.find.open;
        if self
            .editor
            .find_bar(ui, &path, &mut self.docs[self.active].text)
        {
            self.last_edit = Instant::now();
        }
        if finding && !self.editor.find.open {
            self.view.close_find();
        }
        let width = self
            .prefs
            .content_width
            .min((ui.available_width() - 72.0).max(260.0));
        let pad = ((ui.available_width() - width) / 2.0).max(24.0);
        let size = self.prefs.font_size * self.prefs.zoom;
        let accent = self.accent();
        egui::ScrollArea::vertical()
            .animated(false)
            .id_salt(("document", &path, self.view.editing))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.style_mut().override_font_id = Some(FontId::new(
                            size,
                            FontFamily::Name(self.prefs.editor_font.clone().into()),
                        ));
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add_space(self.prefs.top_padding);
                        if self.view.editing {
                            let anchor_id = Id::new("heading-target");
                            if let Some((target, offset)) = ui
                                .ctx()
                                .data(|data| data.get_temp::<(PathBuf, usize)>(anchor_id))
                                && target == path
                            {
                                self.editor.begin_at(
                                    ui.ctx(),
                                    &path,
                                    editor::commands::char_index(
                                        &self.docs[self.active].text,
                                        offset,
                                    ),
                                );
                                ui.ctx()
                                    .data_mut(|data| data.remove::<(PathBuf, usize)>(anchor_id));
                            }
                            if self.editor.show(
                                ui,
                                &path,
                                &mut self.docs[self.active].text,
                                editor::Options {
                                    font: self.prefs.editor_font.clone(),
                                    size,
                                    accent,
                                    width,
                                },
                            ) {
                                self.last_edit = Instant::now();
                            }
                            if let Some(link) = self.editor.link.take() {
                                self.follow_link(ui.ctx(), &link);
                            }
                        } else {
                            let text = self.docs[self.active].text.clone();
                            let mut link = None;
                            for (index, block) in markdown::cached_blocks(ui.ctx(), &path, &text)
                                .iter()
                                .enumerate()
                            {
                                ui.push_id(index, |ui| {
                                    let block_y = ui.cursor().top();
                                    let action = markdown::render(
                                        ui,
                                        &text[block.range.clone()],
                                        &block.kind,
                                        size,
                                        accent,
                                        (&self.prefs.root, &path),
                                    );
                                    for local in &action.toggles {
                                        let offset = block.range.start + local;
                                        editor::transaction(
                                            ui.ctx(),
                                            &path,
                                            &mut self.docs[self.active].text,
                                            |text, selection| {
                                                let checked = text
                                                    .as_bytes()
                                                    .get(offset)
                                                    .is_some_and(|c| *c == b'x' || *c == b'X');
                                                text.replace_range(
                                                    offset..offset + 1,
                                                    if checked { " " } else { "x" },
                                                );
                                                selection
                                            },
                                        );
                                        self.last_edit = Instant::now();
                                    }
                                    let anchor_id = Id::new("heading-target");
                                    if let Some((target, offset)) = ui
                                        .ctx()
                                        .data(|data| data.get_temp::<(PathBuf, usize)>(anchor_id))
                                        && target == path
                                        && block.range.contains(&offset)
                                    {
                                        ui.scroll_to_rect(ui.min_rect(), Some(Align::TOP));
                                        ui.ctx().data_mut(|data| {
                                            data.remove::<(PathBuf, usize)>(anchor_id)
                                        });
                                    }
                                    if action.edit {
                                        self.view.editing = true;
                                        self.editor.begin_block(ui.ctx(), &path, &text, block);
                                        self.editor.anchor_at(
                                            editor::commands::char_index(&text, block.range.start),
                                            block_y,
                                        );
                                    }
                                    if action.toggle {
                                        let offset = block.range.start + 3;
                                        editor::transaction(
                                            ui.ctx(),
                                            &path,
                                            &mut self.docs[self.active].text,
                                            |text, selection| {
                                                text.replace_range(
                                                    offset..offset + 1,
                                                    if matches!(
                                                        block.kind,
                                                        markdown::Kind::Task(true)
                                                    ) {
                                                        " "
                                                    } else {
                                                        "x"
                                                    },
                                                );
                                                selection
                                            },
                                        );
                                        self.last_edit = Instant::now();
                                    }
                                    if action.link.is_some() {
                                        link = action.link;
                                    }
                                });
                            }
                            if let Some(link) = link {
                                self.follow_link(ui.ctx(), &link);
                            }
                            if ui
                                .allocate_response(vec2(width, 90.0), Sense::click())
                                .clicked()
                            {
                                self.view.editing = true;
                                self.editor.begin_at(ui.ctx(), &path, text.chars().count());
                            }
                        }
                        ui.add_space(60.0);
                    });
                });
            });
    }

    fn follow_link(&mut self, ctx: &egui::Context, link: &str) {
        if link.starts_with("https://")
            || link.starts_with("http://")
            || link.starts_with("mailto:")
            || link.starts_with("tel:")
        {
            ctx.open_url(egui::OpenUrl::new_tab(link));
            return;
        }
        let Some(doc) = self.docs.get(self.active) else {
            return;
        };
        let files = ctx
            .data(|data| data.get_temp::<std::sync::Arc<Vec<PathBuf>>>(Id::new("workspace-files")))
            .unwrap_or_default();
        let Some(path) =
            crate::links::resolve_indexed(&self.prefs.root, &doc.path, link, true, &files)
        else {
            self.error = Some(format!("Cannot open note: {link}"));
            return;
        };
        self.open(path.clone(), true);
        if let Some((_, anchor)) = link.split_once('#') {
            if self.io.loading.contains(&path) {
                ctx.data_mut(|d| {
                    d.insert_temp(Id::new("pending-heading"), (path, anchor.to_owned()))
                });
                return;
            }
            if let Some(offset) = crate::links::heading_offset(&self.docs[self.active].text, anchor)
            {
                ctx.data_mut(|data| data.insert_temp(Id::new("heading-target"), (path, offset)));
            } else {
                self.notice = format!("Heading not found: {anchor}");
            }
        }
    }
}
