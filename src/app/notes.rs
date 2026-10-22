use super::*;

impl Notes {
    pub(super) fn toggle_pin(&mut self, path: &Path) {
        if self.prefs.pinned.iter().any(|p| p == path) {
            self.prefs.pinned.retain(|p| p != path);
        } else {
            self.prefs.pinned.push(path.to_owned());
            self.prefs.show_library = true;
        }
    }

    pub(super) fn rename_note(&mut self, old: &Path, name: &str) -> std::io::Result<PathBuf> {
        let mut name = PathBuf::from(name.trim());
        if name.components().count() != 1 {
            return Err(std::io::Error::other("Enter only a filename"));
        }
        if old.is_file() && workspace::is_markdown(old) {
            if name.extension().is_none() {
                name.set_extension(old.extension().unwrap());
            }
            if !workspace::is_markdown(&name) {
                return Err(std::io::Error::other(
                    "Notes must keep a .md or .markdown extension",
                ));
            }
        }
        let parent = old.parent().unwrap_or(&self.prefs.root);
        let relative = parent
            .strip_prefix(&self.prefs.root)
            .map_err(|_| std::io::Error::other("The path is outside this workspace"))?
            .join(&name);
        let target = workspace::safe_path(&self.prefs.root, &relative)?;
        self.move_path(old, &target)?;
        Ok(target)
    }

    /// A successful move updates every path-based navigation/session index.
    fn move_path(&mut self, old: &Path, target: &Path) -> std::io::Result<()> {
        let root = self.prefs.root.canonicalize()?;
        if old == target && old != root {
            return Ok(());
        }
        if !old.canonicalize()?.starts_with(&root) || old == root || target.starts_with(old) {
            return Err(std::io::Error::other("Invalid move destination"));
        }
        let relative = target
            .strip_prefix(&root)
            .map_err(|_| std::io::Error::other("Move stays inside this workspace"))?;
        workspace::safe_path(&root, relative)?;
        let case_only = old
            .to_string_lossy()
            .eq_ignore_ascii_case(&target.to_string_lossy())
            && old.canonicalize()? == target.canonicalize().unwrap_or_default();
        if target.exists() && !case_only {
            return Err(std::io::Error::other("That name already exists"));
        }
        if !self.save_all() {
            return Err(std::io::Error::other(
                "Save or resolve current drafts before moving notes",
            ));
        }
        let changes = workspace::moves::move_with_links(&root, old, target)?;
        let remap = |path: &mut PathBuf| {
            if let Ok(suffix) = path.strip_prefix(old) {
                *path = if suffix.as_os_str().is_empty() {
                    target.to_owned()
                } else {
                    target.join(suffix)
                };
            }
        };
        for doc in &mut self.docs {
            let old_path = doc.path.clone();
            remap(&mut doc.path);
            if doc.path != old_path {
                self.editor.remap_path(&old_path, &doc.path);
            }
            if let Some(change) = changes.iter().find(|change| change.path == doc.path) {
                self.editor
                    .record_rewrite(&doc.path, &change.before, &change.after);
                doc.text = change.after.clone();
                doc.saved = change.after.clone();
            }
        }
        for path in &mut self.history {
            remap(path);
        }
        for path in self
            .prefs
            .pinned
            .iter_mut()
            .chain(self.prefs.recent.iter_mut())
        {
            remap(path);
        }
        for path in &mut self.prefs.tabs {
            remap(path);
        }
        self.expanded = std::mem::take(&mut self.expanded)
            .into_iter()
            .map(|mut path| {
                remap(&mut path);
                path
            })
            .collect();
        for (path, _) in &mut self.palette_cache {
            remap(path);
        }
        self.selected_paths = std::mem::take(&mut self.selected_paths)
            .into_iter()
            .map(|mut p| {
                remap(&mut p);
                p
            })
            .collect();
        if let Some(path) = &mut self.selection_anchor {
            remap(path);
        }
        for entry in &mut self.entries {
            remap(&mut entry.path);
        }
        self.refresh_entries();
        Ok(())
    }

    pub(super) fn select_path(&mut self, path: &Path, modifiers: egui::Modifiers) {
        if modifiers.shift {
            if let Some(anchor) = &self.selection_anchor {
                let visible: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|e| {
                        e.path
                            .ancestors()
                            .skip(1)
                            .take_while(|p| *p != self.prefs.root)
                            .all(|p| self.expanded.contains(p))
                    })
                    .map(|e| e.path.clone())
                    .collect();
                if let (Some(a), Some(b)) = (
                    visible.iter().position(|p| p == anchor),
                    visible.iter().position(|p| p == path),
                ) {
                    self.selected_paths
                        .extend(visible[a.min(b)..=a.max(b)].iter().cloned());
                }
            }
        } else if modifiers.ctrl || modifiers.command {
            if !self.selected_paths.remove(path) {
                self.selected_paths.insert(path.to_owned());
            }
            self.selection_anchor = Some(path.to_owned());
        } else {
            self.selected_paths = [path.to_owned()].into_iter().collect();
            self.selection_anchor = Some(path.to_owned());
        }
    }

    pub(super) fn move_selected(
        &mut self,
        paths: &[PathBuf],
        destination: &Path,
    ) -> std::io::Result<()> {
        if self.defer_until_saved(file_io::Deferred::Move(
            paths.to_vec(),
            destination.to_owned(),
        )) {
            return Ok(());
        }
        if !self.save_all() {
            return Err(std::io::Error::other(
                "Resolve draft conflicts before moving notes",
            ));
        }
        let destination = destination.canonicalize()?;
        let root = self.prefs.root.canonicalize()?;
        if !destination.starts_with(&root) || !destination.is_dir() {
            return Err(std::io::Error::other(
                "Choose a folder inside the workspace",
            ));
        }
        let paths: Vec<_> = paths
            .iter()
            .filter(|path| {
                !paths
                    .iter()
                    .any(|other| *path != other && path.starts_with(other))
            })
            .cloned()
            .collect();
        // Reject invalid batches before moving any file.
        let mut targets = BTreeSet::new();
        for old in &paths {
            let target = destination.join(
                old.file_name()
                    .ok_or_else(|| std::io::Error::other("Invalid filename"))?,
            );
            if target.exists() || destination.starts_with(old) {
                return Err(std::io::Error::other(
                    "Move would overwrite a file or put a folder inside itself",
                ));
            }
            if !targets.insert(target) {
                return Err(std::io::Error::other(
                    "Several selected files have the same name",
                ));
            }
            if !old.canonicalize()?.starts_with(&root) || old == &root {
                return Err(std::io::Error::other(
                    "Move source is outside this workspace",
                ));
            }
        }
        for old in paths {
            self.move_path(&old, &destination.join(old.file_name().unwrap()))?;
        }
        self.expanded.insert(destination);
        Ok(())
    }

    pub(super) fn archive(&mut self, path: &Path) {
        if self.defer_until_saved(file_io::Deferred::Archive(path.to_owned())) {
            return;
        }
        if !self.save_all() {
            return;
        }
        let folder = self.prefs.root.join("Archive");
        let target = folder.join(path.file_name().unwrap_or_default());
        let result = fs::create_dir_all(folder).and_then(|_| self.move_path(path, &target));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
    }

    pub(super) fn delete_note(&mut self, path: &Path) {
        if self.defer_until_saved(file_io::Deferred::Delete(path.to_owned())) {
            return;
        }
        // Save this note before recycling so restoring it recovers the latest
        // draft. A disk conflict must be resolved before deleting either copy.
        if let Some(doc) = self.docs.iter_mut().find(|doc| doc.path == path)
            && let Err(error) = doc.save()
        {
            self.error = Some(error.to_string());
            return;
        }
        if let Err(error) = workspace::recycle_note(&self.prefs.root, path) {
            self.error = Some(format!("Could not delete note: {error}"));
            return;
        }
        if let Some(index) = self.docs.iter().position(|doc| doc.path == path) {
            self.docs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.docs.len().saturating_sub(1));
        }
        let retained_before = self
            .history
            .iter()
            .take(self.history_pos + 1)
            .filter(|entry| entry.as_path() != path)
            .count();
        self.history.retain(|entry| entry != path);
        self.history_pos = retained_before.saturating_sub(1);
        self.palette_cache.retain(|(entry, _)| entry != path);
        self.read_note();
        self.entries.retain(|entry| entry.path != path);
        self.prefs.pinned.retain(|p| p != path);
        self.prefs.recent.retain(|p| p != path);
        self.refresh_entries();
        self.notice = "Note moved to Recycle Bin".into();
    }
}
