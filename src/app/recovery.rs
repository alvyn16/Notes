use super::*;

#[derive(Default)]
pub(super) struct Recovery {
    pending: Vec<Pending>,
    seen: BTreeSet<PathBuf>,
}

struct Pending {
    draft: workspace::recovery::Draft,
    text: String,
    disk: String,
}
impl Recovery {
    pub(super) fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}
impl Notes {
    pub(super) fn discover_recovery(&mut self, paths: &[PathBuf]) {
        for draft in workspace::recovery::discover(paths) {
            if !self.recovery.seen.insert(draft.path.clone()) {
                continue;
            }
            if self
                .docs
                .iter()
                .any(|doc| doc.path == draft.original && (doc.conflict || doc.dirty()))
            {
                continue;
            }
            match fs::read_to_string(&draft.path) {
                Ok(text) => {
                    let disk = fs::read_to_string(&draft.original).unwrap_or_default();
                    if disk != text {
                        self.recovery.pending.push(Pending { draft, text, disk });
                    }
                }
                Err(e) => self.error = Some(format!("Could not read recovered draft: {e}")),
            }
        }
    }
    pub(super) fn recovery_ui(&mut self, ctx: &egui::Context) {
        if self.recovery.pending.is_empty() || self.dialog.is_some() || self.palette {
            return;
        }
        let pending = &self.recovery.pending[0];
        let mut action = 0;
        let frame = Frame::new()
            .fill(ctx.style().visuals.window_fill)
            .corner_radius(12)
            .inner_margin(24)
            .stroke(Stroke::new(
                1.0_f32,
                controls::blend(self.background(), ctx.style().visuals.text_color(), 0.15),
            ));
        let response = egui::Modal::new(Id::new("recovery-review")).frame(frame).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width()-100.0).clamp(260.0,760.0));
            ui.label(RichText::new("A draft was recovered").size(20.0));
            ui.label(pending.draft.original.strip_prefix(&self.prefs.root).unwrap_or(&pending.draft.original).display().to_string());
            ui.add_space(12.0);
            ui.label("Compare the versions. Reviewing a draft leaves the disk version untouched until you choose which one to keep.");
            ui.add_space(12.0);
            ui.columns(2, |columns| {
                for column in columns.iter_mut() { column.spacing_mut().item_spacing.x = 16.0; }
                for (column, (title, text)) in columns.iter_mut().zip([("Disk version", &pending.disk), ("Recovered draft", &pending.text)]) {
                    column.label(RichText::new(title).strong());
                    egui::ScrollArea::vertical().id_salt(title).max_height(260.0).show(column, |ui| { ui.add(egui::Label::new(if title == "Disk version" && !pending.draft.original.exists() { "The original note is missing." } else { text }).wrap().selectable(true)); });
                }
            });
            ui.add_space(18.0);
            ui.horizontal_wrapped(|ui| {
                if controls::action(ui,"Review draft",true,true,110.0).clicked() { action = 1; }
                if controls::action(ui,"Keep disk version",false,pending.draft.original.is_file(),126.0).on_hover_text("Discard this recovered draft").clicked() { action = 2; }
                if controls::action(ui,"Later",false,true,64.0).clicked() { action = 3; }
            });
            if let Some(error) = &self.error { ui.add_space(8.0); ui.colored_label(ui.visuals().error_fg_color,error); }
        });
        if response.should_close() {
            action = 3;
        }
        let result = match action {
            1 => self.review_recovery(),
            2 => fs::remove_file(&self.recovery.pending[0].draft.path),
            _ => Ok(()),
        };
        if action != 0 {
            match result {
                Ok(()) => {
                    self.recovery.pending.remove(0);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn review_recovery(&mut self) -> std::io::Result<()> {
        let pending = &self.recovery.pending[0];
        if fs::read_to_string(&pending.draft.path)? != pending.text {
            return Err(std::io::Error::other(
                "This recovery file changed. Reopen Notes to review the latest version.",
            ));
        }
        let current = workspace::recovery::path(&pending.draft.original);
        if current != pending.draft.path && current.exists() {
            return Err(std::io::Error::other(
                "A newer recovery draft already exists for this note",
            ));
        }
        if self
            .docs
            .iter()
            .any(|doc| doc.path == pending.draft.original && doc.dirty())
        {
            return Err(std::io::Error::other(
                "Save or resolve the current edits before opening this recovered draft",
            ));
        }
        let mut doc = match Document::load(pending.draft.original.clone()) {
            Ok(doc) => doc,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document {
                path: pending.draft.original.clone(),
                text: String::new(),
                saved: String::new(),
                conflict: false,
            },
            Err(e) => return Err(e),
        };
        doc.text = pending.text.clone();
        doc.conflict = true;
        doc.preserve_draft()?;
        if workspace::recovery::path(&doc.path) != pending.draft.path {
            fs::remove_file(&pending.draft.path)?;
        }
        if let Some(index) = self.docs.iter().position(|d| d.path == doc.path) {
            self.docs[index] = doc;
            self.active = index;
        } else {
            self.docs.push(doc);
            self.active = self.docs.len() - 1;
        }
        self.startup_open = false;
        self.read_note();
        self.error = None;
        Ok(())
    }
    pub(super) fn save_conflict_copy(&mut self, destination: &Path) -> std::io::Result<()> {
        self.docs[self.active].preserve_draft()?;
        self.docs[self.active].save_copy(destination)?;
        fs::remove_file(workspace::recovery::path(&self.docs[self.active].path))?;
        // Copy succeeded: closing this tab explicitly resolves its conflict.
        self.docs.remove(self.active);
        self.active = self.active.min(self.docs.len().saturating_sub(1));
        self.error = None;
        self.notice = "Draft copy saved. Original disk version kept.".into();
        self.read_note();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_discovers_unopened_and_deleted_notes_and_review_preserves_disk() {
        let root = tempfile::tempdir().unwrap();
        let mut app = super::super::tests::notes();
        app.prefs.root = root.path().canonicalize().unwrap();
        let note = workspace::create(&app.prefs.root, "Unopened", false).unwrap();
        fs::write(&note, "disk version").unwrap();
        fs::write(workspace::recovery::path(&note), "recovered draft").unwrap();
        let deleted = app.prefs.root.join("Deleted.md");
        fs::write(workspace::recovery::path(&deleted), "deleted note draft").unwrap();
        let paths = workspace::inventory(&app.prefs.root).1;
        app.discover_recovery(&paths);
        assert_eq!(app.recovery.pending.len(), 2);
        app.discover_recovery(&paths);
        assert_eq!(app.recovery.pending.len(), 2);
        let at = app
            .recovery
            .pending
            .iter()
            .position(|p| p.draft.original == note)
            .unwrap();
        app.recovery.pending.swap(0, at);
        app.review_recovery().unwrap();
        assert_eq!(app.docs[0].text, "recovered draft");
        assert!(app.docs[0].conflict);
        assert_eq!(fs::read_to_string(&note).unwrap(), "disk version");
        app.docs[0].resolve_draft().unwrap();
        assert_eq!(fs::read_to_string(&note).unwrap(), "recovered draft");
        let at = app
            .recovery
            .pending
            .iter()
            .position(|p| p.draft.original == deleted)
            .unwrap();
        app.recovery.pending.swap(0, at);
        app.review_recovery().unwrap();
        app.docs[app.active].resolve_draft().unwrap();
        assert_eq!(fs::read_to_string(deleted).unwrap(), "deleted note draft");
    }
}
