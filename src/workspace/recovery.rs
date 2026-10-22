use super::*;

const SUFFIX: &str = ".writer-recovery";
pub fn scan(root: &Path) -> Vec<PathBuf> {
    // Recovery must remain discoverable even when the vault ignores sidecars.
    ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .follow_links(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .flatten()
        .filter(|entry| {
            entry.file_type().is_some_and(|t| t.is_file())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(SUFFIX))
        })
        .map(|entry| entry.into_path())
        .collect()
}
pub fn path(note: &Path) -> PathBuf {
    let mut name = note.as_os_str().to_os_string();
    name.push(SUFFIX);
    PathBuf::from(name)
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub path: PathBuf,
    pub original: PathBuf,
}
pub fn discover(paths: &[PathBuf]) -> Vec<Draft> {
    paths
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.strip_suffix(SUFFIX)?;
            let mut original = path.with_file_name(name);
            if !is_markdown(&original) {
                return None;
            }
            // Old versions discarded the .markdown extension. Only attribute that
            // legacy draft when there is no .md note competing for the same name.
            if !original.exists() && original.extension().is_some_and(|e| e == "md") {
                let legacy = original.with_extension("markdown");
                if legacy.exists() && !self::path(&legacy).exists() {
                    original = legacy;
                }
            }
            Some(Draft {
                path: path.clone(),
                original,
            })
        })
        .collect()
}

impl Document {
    pub fn resolve_disk(&mut self) -> io::Result<()> {
        let mut disk = self.clone();
        disk.reload()?;
        match fs::remove_file(path(&self.path)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        *self = disk;
        Ok(())
    }
    pub fn resolve_draft(&mut self) -> io::Result<()> {
        // This is an explicit overwrite action, never the autosave path.
        self.preserve_draft()?;
        atomic_write(&self.path, &self.text)?;
        fs::remove_file(path(&self.path))?;
        self.saved = self.text.clone();
        self.conflict = false;
        Ok(())
    }
    pub fn save_copy(&self, destination: &Path) -> io::Result<()> {
        if destination == self.path
            || destination
                .canonicalize()
                .ok()
                .zip(self.path.canonicalize().ok())
                .is_some_and(|(a, b)| a == b)
        {
            return Err(io::Error::other(
                "Choose a different file for the draft copy",
            ));
        }
        atomic_write(destination, &self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignored_sidecars_remain_discoverable() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(".gitignore"), "*.writer-recovery\n").unwrap();
        let note = create(root.path(), "Note", false).unwrap();
        fs::write(path(&note), "draft").unwrap();
        assert!(
            !inventory(root.path())
                .1
                .iter()
                .any(|p| p.ends_with("Note.md.writer-recovery"))
        );
        assert_eq!(discover(&scan(root.path())).len(), 1);
    }
    #[test]
    fn extensions_have_distinct_drafts_and_legacy_drafts_are_discovered() {
        let root = tempfile::tempdir().unwrap();
        let md = create(root.path(), "Same.md", false).unwrap();
        let markdown = create(root.path(), "Same.markdown", false).unwrap();
        assert_ne!(path(&md), path(&markdown));
        for (note, text) in [(&md, "md draft"), (&markdown, "markdown draft")] {
            let mut doc = Document::load(note.clone()).unwrap();
            doc.text = text.into();
            doc.preserve_draft().unwrap();
        }
        let drafts = discover(&inventory(root.path()).1);
        assert_eq!(drafts.len(), 2);
        assert_eq!(fs::read_to_string(path(&md)).unwrap(), "md draft");
        assert_eq!(
            fs::read_to_string(path(&markdown)).unwrap(),
            "markdown draft"
        );
        fs::remove_file(&md).unwrap();
        fs::remove_file(path(&markdown)).unwrap();
        assert_eq!(discover(&[path(&md)])[0].original, markdown);
    }
    #[test]
    fn resolving_conflicts_and_saving_copies_preserves_the_chosen_version() {
        let root = tempfile::tempdir().unwrap();
        let note = create(root.path(), "Note", false).unwrap();
        let mut doc = Document::load(note.clone()).unwrap();
        doc.text = "draft".into();
        fs::write(&note, "disk").unwrap();
        assert!(doc.save().is_err());
        assert!(doc.save_copy(&note).is_err());
        let copy = root.path().join("copy.md");
        doc.save_copy(&copy).unwrap();
        assert_eq!(fs::read_to_string(&copy).unwrap(), "draft");
        assert_eq!(fs::read_to_string(&note).unwrap(), "disk");
        doc.resolve_disk().unwrap();
        assert_eq!(doc.text, "disk");
        assert!(!doc.conflict);
        assert!(!path(&note).exists());
        doc.text = "chosen draft".into();
        doc.conflict = true;
        doc.resolve_draft().unwrap();
        assert_eq!(fs::read_to_string(note).unwrap(), "chosen draft");
        assert!(!doc.dirty());
        assert!(!doc.conflict);
    }
}
