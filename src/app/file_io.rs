//! Serial note I/O. The UI owns editable text; workers only receive snapshots.
use super::*;
use std::{collections::HashMap, sync::mpsc};

enum Request {
    Load(PathBuf, u64),
    Save(Document),
    Resolve(Document, Resolution),
}

#[cfg(test)]
mod tests {
    use super::*;
    fn worker_app() -> (tempfile::TempDir, Notes, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let mut app = super::super::tests::notes();
        app.prefs.root = root.path().canonicalize().unwrap();
        let path = workspace::create(&app.prefs.root, "Async", false).unwrap();
        app.io.worker = Some(Worker::start());
        (root, app, path)
    }
    #[test]
    fn dragging_a_dirty_tab_out_defers_detach_until_saved() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        pump(&mut app, |a| a.io.loading.is_empty());
        app.docs[0].text = "draft to move".into();
        let ctx = egui::Context::default();
        let frame = |app: &mut Notes, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 300.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.scope_builder(
                            UiBuilder::new()
                                .max_rect(Rect::from_min_size(Pos2::ZERO, vec2(880.0, 54.0))),
                            |ui| app.toolbar(ui),
                        );
                    });
                },
            )
        };
        let output = frame(&mut app, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text().starts_with("Async") => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        let event = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(position), event(position, true)],
        );
        let outside = position + vec2(0.0, 130.0);
        frame(&mut app, vec![egui::Event::PointerMoved(outside)]);
        frame(&mut app, vec![event(outside, false)]);
        assert!(matches!(&app.io.deferred,Some(Deferred::Detach(target)) if target==&path));
        assert_eq!(app.docs[0].text, "draft to move");
        assert_eq!(app.docs.len(), 1);
    }
    fn pump(app: &mut Notes, done: impl Fn(&Notes) -> bool) {
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(app) {
            app.poll_file_io(&ctx);
            assert!(Instant::now() < deadline, "I/O completion timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn async_load_save_and_close_keep_edits_typed_during_save() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        assert!(app.io.loading.contains(&path));
        pump(&mut app, |a| a.io.loading.is_empty());
        app.docs[0].text = "first snapshot".into();
        assert!(!app.save_all());
        app.docs[0].text = "newer typing".into();
        app.close_tab(0);
        assert_eq!(app.docs.len(), 1);
        pump(&mut app, |a| a.docs.is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), "newer typing");
    }
    #[test]
    fn external_conflict_and_failed_save_keep_tab_and_latest_recovery() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        pump(&mut app, |a| a.io.loading.is_empty());
        app.docs[0].text = "draft".into();
        fs::write(&path, "external").unwrap();
        app.close_tab(0);
        pump(&mut app, |a| a.io.saving.is_empty());
        assert_eq!(app.docs.len(), 1);
        assert!(app.docs[0].conflict);
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        app.docs[0].text = "later edits".into();
        app.save_all();
        pump(&mut app, |a| a.io.saving.is_empty());
        assert_eq!(
            fs::read_to_string(workspace::recovery::path(&path)).unwrap(),
            "later edits"
        );
    }
    #[test]
    fn old_load_completion_cannot_replace_reopened_note() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        let old_token = app.io.next_load;
        app.close_tab(0);
        fs::write(&path, "new contents").unwrap();
        app.open(path.clone(), true);
        let old = Document {
            path: path.clone(),
            text: "stale".into(),
            saved: "stale".into(),
            conflict: false,
        };
        app.apply_file_completion(
            &egui::Context::default(),
            Completion::Loaded(path.clone(), old_token, Ok(old)),
        );
        assert!(app.io.loading.contains(&path));
        pump(&mut app, |a| a.io.loading.is_empty());
        assert_eq!(app.docs[0].text, "new contents");
    }
    #[test]
    fn rename_waits_for_background_save() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        pump(&mut app, |a| a.io.loading.is_empty());
        app.docs[0].text = "latest draft".into();
        assert!(app.defer_until_saved(Deferred::Rename(path.clone(), "Renamed".into())));
        pump(&mut app, |a| a.docs[0].path.ends_with("Renamed.md"));
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(&app.docs[0].path).unwrap(),
            "latest draft"
        );
    }
    #[test]
    fn background_conflict_resolution_preserves_the_chosen_version() {
        for resolution in [
            Resolution::Disk,
            Resolution::Draft,
            Resolution::Copy(PathBuf::new()),
        ] {
            let (_root, mut app, path) = worker_app();
            app.open(path.clone(), true);
            pump(&mut app, |a| a.io.loading.is_empty());
            app.docs[0].text = "draft".into();
            fs::write(&path, "external").unwrap();
            app.save_all();
            pump(&mut app, |a| a.io.saving.is_empty());
            let copy = app.prefs.root.join("copy.md");
            let disk = matches!(resolution, Resolution::Disk);
            let copying = matches!(resolution, Resolution::Copy(_));
            app.resolve_background(if copying {
                Resolution::Copy(copy.clone())
            } else {
                resolution
            });
            pump(&mut app, |a| a.io.resolving.is_empty());
            assert!(app.error.is_none());
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                if disk || copying { "external" } else { "draft" }
            );
            assert!(!workspace::recovery::path(&path).exists());
            if copying {
                assert!(app.docs.is_empty());
                assert_eq!(fs::read_to_string(copy).unwrap(), "draft");
            } else {
                assert!(!app.docs[0].dirty() && !app.docs[0].conflict);
            }
        }
    }
    #[test]
    fn write_failure_cancels_close_and_retains_the_in_memory_draft() {
        let (_root, mut app, path) = worker_app();
        app.open(path.clone(), true);
        pump(&mut app, |a| a.io.loading.is_empty());
        app.docs[0].text = "unsaved draft".into();
        // A directory at the exact atomic temporary-file path makes create_new
        // fail without modifying the original note or relying on ACL privileges.
        let temp = path.with_file_name(format!(".Async.md.writer-{}.tmp", std::process::id()));
        fs::create_dir(&temp).unwrap();
        app.io.closing_window = true;
        app.close_tab(0);
        pump(&mut app, |a| a.io.saving.is_empty());
        assert!(!app.io.closing_window);
        assert_eq!(app.docs[0].text, "unsaved draft");
        assert!(app.docs[0].dirty());
        assert_ne!(fs::read_to_string(&path).unwrap(), "unsaved draft");
        fs::remove_dir(temp).unwrap();
        app.retry_saves();
        app.save_all();
        pump(&mut app, |a| a.io.saving.is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), "unsaved draft");
    }
}
pub(super) enum Completion {
    Loaded(PathBuf, u64, std::io::Result<Document>),
    Saved(Document, std::io::Result<()>),
    Resolved(Document, Resolution, std::io::Result<()>),
}
pub(super) enum Resolution {
    Disk,
    Draft,
    Copy(PathBuf),
}
pub(super) struct Worker {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Completion>,
}
impl Worker {
    pub fn start() -> Self {
        let (tx, requests) = mpsc::channel();
        let (results, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let result = match request {
                    Request::Load(path, token) => {
                        let result = Document::load(path.clone());
                        Completion::Loaded(path, token, result)
                    }
                    Request::Save(mut doc) => {
                        let result = if doc.conflict {
                            doc.preserve_draft()
                        } else {
                            doc.save()
                        };
                        Completion::Saved(doc, result)
                    }
                    Request::Resolve(mut doc, resolution) => {
                        let result = match &resolution {
                            Resolution::Disk => doc.resolve_disk(),
                            Resolution::Draft => doc.resolve_draft(),
                            Resolution::Copy(path) => doc
                                .preserve_draft()
                                .and_then(|_| doc.save_copy(path))
                                .and_then(|_| {
                                    fs::remove_file(workspace::recovery::path(&doc.path))
                                }),
                        };
                        Completion::Resolved(doc, resolution, result)
                    }
                };
                if results.send(result).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }
}
#[derive(Default)]
pub(super) struct FileIo {
    pub worker: Option<Worker>,
    pub loading: BTreeSet<PathBuf>,
    load_tokens: HashMap<PathBuf, u64>,
    next_load: u64,
    pub saving: BTreeSet<PathBuf>,
    pub resolving: BTreeSet<PathBuf>,
    // Avoid repeatedly writing an unchanged recovery draft or retrying a failed
    // save every frame. Explicit Save clears this checkpoint for retry.
    checkpoints: HashMap<PathBuf, u64>,
    pub closing_tab: BTreeSet<PathBuf>,
    pub closing_window: bool,
    deferred: Option<Deferred>,
}
pub(super) enum Deferred {
    OpenWorkspace,
    Detach(PathBuf),
    Update,
    CloseAll(bool),
    Rename(PathBuf, String),
    Archive(PathBuf),
    Delete(PathBuf),
    Move(Vec<PathBuf>, PathBuf),
}
impl Notes {
    pub(super) fn install_update(&mut self) {
        self.updater.install_on_exit = true;
        self.updater.restart = true;
        self.io.closing_window = true;
    }
    pub(super) fn forget_file_io(&mut self, path: &Path) {
        self.io.loading.remove(path);
        self.io.load_tokens.remove(path);
        self.io.checkpoints.remove(path);
        self.io.closing_tab.remove(path);
    }
    pub(super) fn resolve_background(&mut self, resolution: Resolution) {
        let doc = self.docs[self.active].clone();
        if self.io.saving.contains(&doc.path) {
            self.notice = "Protecting draft; try again when saved.".into();
            return;
        }
        self.io.resolving.insert(doc.path.clone());
        self.io.saving.insert(doc.path.clone());
        let _ = self
            .io
            .worker
            .as_ref()
            .unwrap()
            .tx
            .send(Request::Resolve(doc, resolution));
    }
    /// Hold destructive/navigation actions until every save has completed.
    pub(super) fn defer_until_saved(&mut self, action: Deferred) -> bool {
        if self.io.worker.is_none() || self.save_all() {
            return false;
        }
        if self.docs.iter().any(|d| d.conflict) {
            self.error = Some("Resolve draft conflicts before continuing.".into());
        } else {
            self.io.deferred = Some(action);
            self.notice = "Saving before continuing…".into();
        }
        true
    }
    pub(super) fn queue_load(&mut self, path: PathBuf) {
        self.io.loading.insert(path.clone());
        self.io.next_load += 1;
        let token = self.io.next_load;
        self.io.load_tokens.insert(path.clone(), token);
        if let Some(worker) = &self.io.worker {
            let _ = worker.tx.send(Request::Load(path, token));
        }
    }
    pub(super) fn queue_saves(&mut self) -> bool {
        let worker = self.io.worker.as_ref().unwrap();
        for doc in &self.docs {
            if self.io.loading.contains(&doc.path) || self.io.saving.contains(&doc.path) {
                continue;
            }
            if !doc.dirty() && !doc.conflict {
                continue;
            }
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (&doc.text, &doc.saved, doc.conflict).hash(&mut hash);
            let checkpoint = hash.finish();
            if self.io.checkpoints.get(&doc.path) != Some(&checkpoint) {
                self.io.checkpoints.insert(doc.path.clone(), checkpoint);
                self.io.saving.insert(doc.path.clone());
                let _ = worker.tx.send(Request::Save(doc.clone()));
                self.last_scan = Instant::now();
                self.notice = "Saving…".into();
            }
        }
        self.io.saving.is_empty()
            && self.io.loading.is_empty()
            && self.docs.iter().all(|d| !d.dirty() && !d.conflict)
    }
    pub(super) fn retry_saves(&mut self) {
        self.io.checkpoints.clear();
    }
    pub(super) fn poll_file_io(&mut self, ctx: &egui::Context) {
        let completions: Vec<_> = self
            .io
            .worker
            .as_ref()
            .map(|w| w.rx.try_iter().collect())
            .unwrap_or_default();
        for completion in completions {
            self.apply_file_completion(ctx, completion);
        }
        let closing: Vec<_> = self.io.closing_tab.iter().cloned().collect();
        for path in closing {
            if self.io.saving.contains(&path) {
                continue;
            }
            if let Some(index) = self.docs.iter().position(|d| d.path == path) {
                if self.docs[index].conflict {
                    self.io.closing_tab.remove(&path);
                } else if !self.docs[index].dirty() {
                    self.io.closing_tab.remove(&path);
                    self.close_tab(index);
                } else {
                    self.save_all();
                }
            } else {
                self.io.closing_tab.remove(&path);
            }
        }
        if self.io.closing_window && self.save_all() {
            self.io.closing_window = false;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if self.io.deferred.is_some()
            && self.save_all()
            && let Some(action) = self.io.deferred.take()
        {
            match action {
                Deferred::OpenWorkspace => self.open_workspace(),
                Deferred::Detach(path) => self.detach_tab(&path),
                Deferred::Update => self.install_update(),
                Deferred::CloseAll(workspace) => self.close_all(workspace),
                Deferred::Rename(path, name) => match self.rename_note(&path, &name) {
                    Ok(path) => {
                        self.dialog = None;
                        self.open(path, true);
                    }
                    Err(error) => self.error = Some(error.to_string()),
                },
                Deferred::Archive(path) => self.archive(&path),
                Deferred::Delete(path) => self.delete_note(&path),
                Deferred::Move(paths, destination) => {
                    if let Err(error) = self.move_selected(&paths, &destination) {
                        self.error = Some(error.to_string());
                    }
                }
            }
        }
    }
    pub(super) fn apply_file_completion(&mut self, ctx: &egui::Context, completion: Completion) {
        match completion {
            Completion::Resolved(doc, resolution, result) => {
                self.io.resolving.remove(&doc.path);
                self.io.saving.remove(&doc.path);
                match result {
                    Ok(()) => {
                        if let Some(index) = self.docs.iter().position(|d| d.path == doc.path) {
                            if matches!(resolution, Resolution::Copy(_)) {
                                self.forget_file_io(&doc.path);
                                self.docs.remove(index);
                                self.active = self
                                    .active
                                    .saturating_sub(usize::from(index < self.active))
                                    .min(self.docs.len().saturating_sub(1));
                                self.read_note();
                                self.notice =
                                    "Draft copy saved. Original disk version kept.".into();
                            } else {
                                self.editor.reset_external(ctx, &doc.path);
                                self.docs[index] = doc;
                            }
                        }
                        self.error = None;
                        self.refresh_entries();
                    }
                    Err(error) => {
                        self.error = Some(error.to_string());
                        self.io.closing_window = false;
                        self.io.deferred = None;
                    }
                }
            }
            Completion::Loaded(path, token, result) => {
                if self.io.load_tokens.get(&path) != Some(&token) || !self.io.loading.remove(&path)
                {
                    return;
                }
                self.io.load_tokens.remove(&path);
                if let Some(index) = self.docs.iter().position(|d| d.path == path) {
                    match result {
                        Ok(doc) => {
                            if let Some((target, anchor)) = ctx.data(|d| {
                                d.get_temp::<(PathBuf, String)>(Id::new("pending-heading"))
                            }) && target == path
                            {
                                if let Some(offset) =
                                    crate::links::heading_offset(&doc.text, &anchor)
                                {
                                    ctx.data_mut(|d| {
                                        d.insert_temp(
                                            Id::new("heading-target"),
                                            (path.clone(), offset),
                                        )
                                    });
                                } else {
                                    self.notice = format!("Heading not found: {anchor}");
                                }
                                ctx.data_mut(|d| {
                                    d.remove::<(PathBuf, String)>(Id::new("pending-heading"))
                                });
                            }
                            self.docs[index] = doc;
                            self.last_scan = Instant::now();
                            self.refresh_entries();
                        }
                        Err(error) => {
                            self.docs.remove(index);
                            self.active = self
                                .active
                                .saturating_sub(usize::from(index < self.active))
                                .min(self.docs.len().saturating_sub(1));
                            self.error =
                                Some(format!("Could not open {}: {error}", path.display()));
                        }
                    }
                }
            }
            Completion::Saved(saved, result) => {
                self.io.saving.remove(&saved.path);
                if let Some(doc) = self.docs.iter_mut().find(|d| d.path == saved.path) {
                    // Never replace text typed while a snapshot was being saved.
                    doc.saved = saved.saved;
                    doc.conflict |= saved.conflict;
                }
                self.last_scan = Instant::now();
                match result {
                    Ok(()) => {
                        self.notice = if saved.conflict {
                            "Draft protected"
                        } else {
                            "Saved"
                        }
                        .into();
                        self.refresh_entries();
                    }
                    Err(error) => {
                        self.io.closing_window = false;
                        self.io.deferred = None;
                        self.io.closing_tab.remove(&saved.path);
                        self.error = Some(error.to_string());
                    }
                }
            }
        }
        ctx.request_repaint();
    }
}
