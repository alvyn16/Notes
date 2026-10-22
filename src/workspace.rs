use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};
pub mod moves;
pub mod recovery;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub directory: bool,
}

pub fn scan(root: &Path) -> Vec<Entry> {
    inventory(root).0
}
pub fn inventory(root: &Path) -> (Vec<Entry>, Vec<PathBuf>) {
    let mut result = Vec::new();
    let mut files = Vec::new();
    for entry in ignore::WalkBuilder::new(root)
        .hidden(true)
        .follow_links(false)
        .require_git(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        if path == root || path.is_symlink() {
            continue;
        }
        let directory = entry.file_type().is_some_and(|t| t.is_dir());
        if entry.file_type().is_some_and(|t| t.is_file()) {
            files.push(path.to_owned());
        }
        if directory || is_markdown(path) {
            result.push(Entry {
                path: path.to_owned(),
                directory,
            });
        }
    }
    result.sort_by(|a, b| {
        let ap = a.path.parent();
        let bp = b.path.parent();
        if ap == bp {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.path.cmp(&b.path))
        } else {
            a.path.cmp(&b.path)
        }
    });
    (result, files)
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
}

/// Delete only a regular Markdown note inside the current workspace. The OS
/// Recycle Bin owns recovery; never fall back to permanent deletion on failure.
pub fn recycle_note(root: &Path, path: &Path) -> io::Result<()> {
    let root = root.canonicalize()?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || !is_markdown(path) {
        return Err(io::Error::other("Only Markdown notes can be deleted"));
    }
    let path = path.canonicalize()?;
    if !path.starts_with(&root) {
        return Err(io::Error::other("The note is outside this workspace"));
    }
    trash::delete(&path).map_err(|error| io::Error::other(error.to_string()))
}

pub fn safe_path(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Use a relative path inside the workspace",
        ));
    }
    let base = root.canonicalize()?;
    let target = base.join(relative);
    let mut ancestor = target.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| io::Error::other("Invalid path"))?;
    }
    if !ancestor.canonicalize()?.starts_with(&base) {
        return Err(io::Error::other("Path leaves the workspace"));
    }
    Ok(target)
}

pub fn atomic_write(path: &Path, text: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid filename"))?
        .to_string_lossy();
    let temp = path.with_file_name(format!(".{name}.writer-{}.tmp", std::process::id()));
    let mut options = fs::OpenOptions::new();
    use io::Write;
    let mut file = options.write(true).create_new(true).open(&temp)?;
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    drop(file);
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub text: String,
    pub saved: String,
    pub conflict: bool,
}
impl Document {
    pub fn load(path: PathBuf) -> io::Result<Self> {
        let text = fs::read_to_string(&path)?;
        Ok(Self {
            path,
            saved: text.clone(),
            text,
            conflict: false,
        })
    }
    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }
    /// Keep the latest conflicted draft without touching the external version.
    pub fn preserve_draft(&self) -> io::Result<()> {
        let path = recovery::path(&self.path);
        if fs::read_to_string(&path).ok().as_deref() != Some(self.text.as_str()) {
            atomic_write(&path, &self.text)?;
        }
        Ok(())
    }
    pub fn reload(&mut self) -> io::Result<()> {
        if self.conflict {
            self.preserve_draft()?;
        }
        *self = Self::load(self.path.clone())?;
        Ok(())
    }
    #[cfg(test)]
    pub fn refresh(&mut self) -> io::Result<()> {
        let disk = fs::read_to_string(&self.path)?;
        self.observe(&disk);
        Ok(())
    }
    pub fn observe(&mut self, disk: &str) {
        // A conflict still needs explicit resolution even if edits undo back
        // to the saved baseline. Polling must not replace that draft.
        if self.conflict {
            return;
        }
        if disk != self.saved {
            if self.dirty() {
                self.conflict = true;
            } else {
                self.text = disk.to_owned();
                self.saved = disk.to_owned();
            }
        }
    }
    pub fn save(&mut self) -> io::Result<()> {
        if self.conflict {
            self.preserve_draft()?;
            return Err(io::Error::other(
                "Resolve the conflict before saving. Your latest draft is saved as .md.writer-recovery.",
            ));
        }
        if !self.dirty() {
            return Ok(());
        }
        let disk = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) => {
                self.conflict = true;
                self.preserve_draft()?;
                return Err(io::Error::other(format!(
                    "Cannot read the original note: {error}. Your draft is saved as .md.writer-recovery."
                )));
            }
        };
        if disk != self.saved {
            self.conflict = true;
            self.preserve_draft()?;
            return Err(io::Error::other(
                "This note changed outside Notes. Your draft was saved as .md.writer-recovery. Reload or save a copy.",
            ));
        }
        atomic_write(&self.path, &self.text)?;
        self.saved = self.text.clone();
        self.conflict = false;
        Ok(())
    }
}

pub fn create(root: &Path, name: &str, directory: bool) -> io::Result<PathBuf> {
    let mut relative = PathBuf::from(name.trim());
    if !directory && !is_markdown(&relative) {
        relative.set_extension("md");
    }
    let path = safe_path(root, &relative)?;
    if directory {
        fs::create_dir(&path)?;
    } else {
        use io::Write;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        f.write_all(
            format!(
                "# {}\n\n",
                path.file_stem().unwrap_or_default().to_string_lossy()
            )
            .as_bytes(),
        )?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standalone_gitignore_supports_nested_rules_and_negation() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join(".gitignore"),
            "excluded.md\nprivate/\n*.draft.md\n!keep.draft.md\n",
        )
        .unwrap();
        for name in [
            "visible.md",
            "excluded.md",
            "hidden.draft.md",
            "keep.draft.md",
        ] {
            fs::write(root.path().join(name), "note").unwrap();
        }
        for name in ["private", "nested"] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        fs::write(root.path().join("private/secret.md"), "secret").unwrap();
        fs::write(root.path().join("nested/.gitignore"), "local.md\n").unwrap();
        fs::write(root.path().join("nested/local.md"), "hidden").unwrap();
        fs::write(root.path().join("nested/visible.md"), "visible").unwrap();
        let paths: std::collections::BTreeSet<_> = scan(root.path())
            .into_iter()
            .map(|e| e.path.strip_prefix(root.path()).unwrap().to_owned())
            .collect();
        assert_eq!(
            paths,
            ["keep.draft.md", "nested", "nested/visible.md", "visible.md"]
                .map(PathBuf::from)
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn conflict_preserves_latest_draft_before_reload() {
        let root = tempfile::tempdir().unwrap();
        let path = create(root.path(), "Note", false).unwrap();
        let mut doc = Document::load(path.clone()).unwrap();
        doc.text = "first draft".into();
        fs::write(&path, "external version").unwrap();
        assert!(doc.save().is_err());
        doc.text = "latest draft with café".into();
        assert!(doc.save().is_err());
        assert_eq!(
            fs::read_to_string(path.with_extension("md.writer-recovery")).unwrap(),
            doc.text
        );
        doc.text.push_str(" and one last edit");
        let latest = doc.text.clone();
        doc.reload().unwrap();
        assert_eq!(doc.text, "external version");
        assert!(!doc.conflict);
        assert_eq!(
            fs::read_to_string(path.with_extension("md.writer-recovery")).unwrap(),
            latest
        );
    }

    #[test]
    fn failed_recovery_write_does_not_discard_draft_on_reload() {
        let root = tempfile::tempdir().unwrap();
        let path = create(root.path(), "Note", false).unwrap();
        let mut doc = Document::load(path.clone()).unwrap();
        doc.conflict = true;
        doc.text = "keep this draft".into();
        fs::create_dir(path.with_extension("md.writer-recovery")).unwrap();
        assert!(doc.reload().is_err());
        assert_eq!(doc.text, "keep this draft");
        assert!(doc.conflict);
    }
    #[test]
    fn recycle_rejects_folders_non_notes_and_outside_files() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_note = create(outside.path(), "Keep", false).unwrap();
        let other = root.path().join("Keep.txt");
        fs::write(&other, "keep").unwrap();
        assert!(recycle_note(root.path(), root.path()).is_err());
        assert!(recycle_note(root.path(), &other).is_err());
        assert!(recycle_note(root.path(), &outside_note).is_err());
        assert!(recycle_note(root.path(), &root.path().join("missing.md")).is_err());
        assert_eq!(fs::read_to_string(other).unwrap(), "keep");
        assert!(outside_note.exists());
        assert!(root.path().exists());
    }
    #[test]
    fn save_refresh_and_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let p = create(dir.path(), "Note", false).unwrap();
        let mut d = Document::load(p.clone()).unwrap();
        d.text.push_str("Hello");
        d.save().unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), d.text);
        fs::write(&p, "External").unwrap();
        d.refresh().unwrap();
        assert_eq!(d.text, "External");
        d.text = "Draft".into();
        fs::write(&p, "External 2").unwrap();
        assert!(d.save().is_err());
        assert!(d.conflict);
        assert_eq!(fs::read_to_string(&p).unwrap(), "External 2");
        assert_eq!(
            fs::read_to_string(p.with_extension("md.writer-recovery")).unwrap(),
            "Draft"
        );
    }
    #[test]
    fn paths_and_creation_are_safe() {
        let d = tempfile::tempdir().unwrap();
        assert!(safe_path(d.path(), Path::new("../escape.md")).is_err());
        assert!(safe_path(d.path(), Path::new("C:/escape.md")).is_err());
        create(d.path(), "Note", false).unwrap();
        assert!(create(d.path(), "Note", false).is_err());
        assert_eq!(scan(d.path()).len(), 1);
    }
    #[test]
    fn missing_file_never_recreated_by_autosave() {
        let d = tempfile::tempdir().unwrap();
        let p = create(d.path(), "Note", false).unwrap();
        let mut doc = Document::load(p.clone()).unwrap();
        doc.text.push('x');
        fs::remove_file(&p).unwrap();
        assert!(doc.save().is_err());
        assert!(!p.exists());
    }
}
