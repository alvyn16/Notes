use super::*;
use crate::links::rewrite::{moved, rewrite};

pub struct Change {
    pub path: PathBuf,
    pub before: String,
    pub after: String,
}
/// Stage every affected note before moving anything; roll back on write failure.
pub fn move_with_links(root: &Path, old: &Path, new: &Path) -> io::Result<Vec<Change>> {
    let root_path = root.canonicalize()?;
    let old_path = old.canonicalize()?;
    let new_path = new
        .parent()
        .ok_or_else(|| io::Error::other("Invalid move destination"))?
        .canonicalize()?
        .join(
            new.file_name()
                .ok_or_else(|| io::Error::other("Invalid filename"))?,
        );
    let (root, old, new) = (root_path.as_path(), old_path.as_path(), new_path.as_path());
    let (entries, paths) = inventory(root);
    let mut changes = Vec::new();
    for entry in entries.into_iter().filter(|entry| !entry.directory) {
        let before = fs::read_to_string(&entry.path)?;
        let after = rewrite(&before, &entry.path, root, &paths, old, new);
        if before != after {
            changes.push(Change {
                path: moved(&entry.path, old, new),
                before,
                after,
            });
        }
    }
    let sidecar = (old.is_file() && recovery::path(old).exists())
        .then(|| (recovery::path(old), recovery::path(new)));
    if let Some((source, target)) = &sidecar
        && target.exists()
        && source.canonicalize()? != target.canonicalize()?
    {
        return Err(io::Error::other(
            "A recovery draft already exists at the destination",
        ));
    }
    // Recheck the staged contents before the first mutation.
    for change in &changes {
        let original = moved(&change.path, new, old);
        if fs::read_to_string(original)? != change.before {
            return Err(io::Error::other(
                "A linked note changed while preparing the move. Try again.",
            ));
        }
    }
    fs::rename(old, new)?;
    let mut written = 0;
    let mut sidecar_moved = false;
    let result = (|| {
        if let Some((source, target)) = &sidecar {
            fs::rename(source, target)?;
            sidecar_moved = true;
        }
        for change in &changes {
            if fs::read_to_string(&change.path)? != change.before {
                return Err(io::Error::other("A linked note changed during the move"));
            }
            atomic_write(&change.path, &change.after)?;
            written += 1;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_failed = false;
        for change in changes[..written].iter().rev() {
            if fs::read_to_string(&change.path).ok().as_deref() == Some(&change.after) {
                rollback_failed |= atomic_write(&change.path, &change.before).is_err();
            } else {
                rollback_failed = true;
            }
        }
        if sidecar_moved && let Some((source, target)) = &sidecar {
            rollback_failed |= fs::rename(target, source).is_err();
        }
        rollback_failed |= fs::rename(new, old).is_err();
        return Err(io::Error::other(if rollback_failed {
            format!(
                "{error}. Some files could not be rolled back; inspect the source and destination before retrying."
            )
        } else {
            format!("{error}. The move was rolled back.")
        }));
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_rename_keeps_its_recovery_sidecar() {
        let root = tempfile::tempdir().unwrap();
        let old = create(root.path(), "Old.markdown", false).unwrap();
        fs::write(recovery::path(&old), "recover me").unwrap();
        let new = old.with_file_name("New.markdown");
        move_with_links(root.path(), &old, &new).unwrap();
        assert!(!recovery::path(&old).exists());
        assert_eq!(
            fs::read_to_string(recovery::path(&new)).unwrap(),
            "recover me"
        );
    }
    #[test]
    fn folder_moves_update_backlinks_internal_links_and_images() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let old = root.join("Folder");
        fs::create_dir(&old).unwrap();
        let note = create(&old, "Note", false).unwrap();
        fs::write(old.join("image.png"), "image").unwrap();
        fs::write(&note, "![image](image.png) [outside](../Other.md)").unwrap();
        fs::write(
            root.join("Other.md"),
            "[[Folder/Note|Alias]] ![](Folder/image.png)",
        )
        .unwrap();
        let target = root.join("Moved");
        move_with_links(root, &old, &target).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("Note.md")).unwrap(),
            "![image](image.png) [outside](../Other.md)"
        );
        assert_eq!(
            fs::read_to_string(root.join("Other.md")).unwrap(),
            "[[Moved/Note.md|Alias]] ![](Moved/image.png)"
        );
    }
    #[test]
    fn failed_backlink_write_rolls_back_move_and_earlier_writes() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let old = create(root, "Note", false).unwrap();
        for name in ["A.md", "B.md"] {
            fs::write(root.join(name), "[[Note]]").unwrap();
        }
        let locked_temp = root.join(format!(".B.md.writer-{}.tmp", std::process::id()));
        fs::write(&locked_temp, "unrelated temp").unwrap();
        assert!(move_with_links(root, &old, &root.join("New.md")).is_err());
        assert!(old.exists());
        assert!(!root.join("New.md").exists());
        for name in ["A.md", "B.md"] {
            assert_eq!(fs::read_to_string(root.join(name)).unwrap(), "[[Note]]");
        }
    }
}
