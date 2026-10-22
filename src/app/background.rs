use super::*;
impl Notes {
    pub(super) fn refresh_entries(&mut self) {
        self.last_scan = Instant::now();
        if let Some(index) = &self.index {
            index.refresh();
        } else {
            self.entries = workspace::scan(&self.prefs.root);
        }
    }
    pub(super) fn poll_index(&mut self, ctx: &egui::Context) {
        let updates: Vec<_> = self
            .index
            .as_ref()
            .map(|index| index.updates().collect())
            .unwrap_or_default();
        for update in updates {
            self.apply_index_update(ctx, update);
        }
    }
    pub(super) fn apply_index_update(&mut self, ctx: &egui::Context, update: crate::index::Update) {
        match update {
            crate::index::Update::Snapshot(snapshot) if snapshot.started >= self.last_scan => {
                self.discover_recovery(&snapshot.paths);
                self.entries = snapshot.entries;
                ctx.data_mut(|data| data.insert_temp(Id::new("workspace-files"), snapshot.paths));
                for doc in &mut self.docs {
                    if doc.dirty()
                        || doc.conflict
                        || self.io.loading.contains(&doc.path)
                        || self.io.saving.contains(&doc.path)
                    {
                        continue;
                    }
                    if let Some(text) = snapshot.files.get(&doc.path)
                        && text.as_ref() != doc.saved
                    {
                        doc.observe(text);
                        self.editor.reset_external(ctx, &doc.path);
                    }
                }
                if self.startup_open
                    && self.docs.is_empty()
                    && let Some(entry) = self
                        .entries
                        .iter()
                        .find(|e| {
                            !e.directory
                                && e.path.ends_with("Daily Notes/Monday, April 28, 2026.md")
                        })
                        .or_else(|| self.entries.iter().find(|e| !e.directory))
                {
                    self.open(entry.path.clone(), true);
                    self.startup_open = false;
                }
            }
            crate::index::Update::Search(query, hits, started)
                if query == self.search_query && started >= self.last_scan =>
            {
                self.palette_cache = hits
            }
            _ => {}
        }
    }
}
