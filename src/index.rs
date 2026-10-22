//! One cancellable worker per workspace owns scanning, content I/O and search.
use crate::workspace::{self, Entry};
use notify::{RecursiveMode, Watcher};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};

pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub files: HashMap<PathBuf, Arc<str>>,
    pub started: Instant,
    pub paths: Arc<Vec<PathBuf>>,
}
pub enum Update {
    Snapshot(Snapshot),
    Search(String, Vec<(PathBuf, String)>, Instant),
}
enum Request {
    Refresh,
    Search(String),
}
pub struct Index {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    stop: Arc<AtomicBool>,
}
impl Drop for Index {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
impl Index {
    pub fn start(root: PathBuf) -> Self {
        let (tx, requests) = mpsc::channel();
        let (updates, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        std::thread::spawn(move || {
            let (events, changes) = mpsc::channel();
            let mut watcher =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    if let Ok(event) = event
                        && !matches!(event.kind, notify::EventKind::Access(_))
                    {
                        let _ = events.send(());
                    }
                })
                .ok();
            if let Some(watcher) = &mut watcher {
                let _ = watcher.watch(&root, RecursiveMode::Recursive);
            }
            let mut cache = HashMap::<PathBuf, (Option<SystemTime>, u64, Arc<str>)>::new();
            let mut files = HashMap::new();
            let mut query = String::new();
            let mut refresh = true;
            let mut last = Instant::now();
            let mut indexed_at = last;
            while !cancelled.load(Ordering::Relaxed) {
                if refresh {
                    let started = Instant::now();
                    indexed_at = started;
                    let (entries, mut paths) = workspace::inventory(&root);
                    paths.extend(workspace::recovery::scan(&root));
                    paths.sort();
                    paths.dedup();
                    files.clear();
                    for entry in &entries {
                        if cancelled.load(Ordering::Relaxed) {
                            return;
                        }
                        if entry.directory {
                            continue;
                        }
                        let Ok(metadata) = fs::metadata(&entry.path) else {
                            continue;
                        };
                        let stamp = metadata.modified().ok();
                        let cached = cache
                            .get(&entry.path)
                            .filter(|(m, len, _)| *m == stamp && *len == metadata.len());
                        let text = if let Some((_, _, text)) = cached {
                            text.clone()
                        } else {
                            let Ok(text) = fs::read_to_string(&entry.path) else {
                                continue;
                            };
                            Arc::<str>::from(text)
                        };
                        files.insert(entry.path.clone(), text.clone());
                        cache.insert(entry.path.clone(), (stamp, metadata.len(), text));
                    }
                    cache.retain(|path, _| files.contains_key(path));
                    if updates
                        .send(Update::Snapshot(Snapshot {
                            entries,
                            files: files.clone(),
                            started,
                            paths: Arc::new(paths),
                        }))
                        .is_err()
                    {
                        return;
                    }
                    let _ = updates.send(Update::Search(
                        query.clone(),
                        search(&files, &query),
                        indexed_at,
                    ));
                    last = Instant::now();
                    refresh = false;
                }
                match requests.recv_timeout(Duration::from_millis(100)) {
                    Ok(Request::Refresh) => refresh = true,
                    Ok(Request::Search(q)) => {
                        query = q;
                        let _ = updates.send(Update::Search(
                            query.clone(),
                            search(&files, &query),
                            indexed_at,
                        ));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    Err(_) => {}
                }
                // Coalesce watcher bursts; periodically reconcile missed events.
                if last.elapsed() > Duration::from_millis(250) && changes.try_recv().is_ok() {
                    while changes.try_recv().is_ok() {}
                    refresh = true;
                }
                refresh |= last.elapsed() > Duration::from_secs(10);
            }
        });
        Self { tx, rx, stop }
    }
    pub fn refresh(&self) {
        let _ = self.tx.send(Request::Refresh);
    }
    pub fn search(&self, query: String) {
        let _ = self.tx.send(Request::Search(query));
    }
    pub fn updates(&self) -> impl Iterator<Item = Update> + '_ {
        self.rx.try_iter()
    }
}
fn search(files: &HashMap<PathBuf, Arc<str>>, query: &str) -> Vec<(PathBuf, String)> {
    if query.is_empty() {
        return vec![];
    }
    let q = query.to_lowercase();
    let mut results: Vec<_> = files
        .iter()
        .filter_map(|(path, text)| {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let line = text.lines().find(|line| line.to_lowercase().contains(&q));
            if !name.contains(&q) && line.is_none() {
                return None;
            }
            Some((
                path.clone(),
                line.unwrap_or("").chars().take(120).collect::<String>(),
            ))
        })
        .collect();
    results.sort_by_key(|(path, _)| {
        (
            !path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .contains(&q),
            path.clone(),
        )
    });
    results.truncate(200);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watcher_refreshes_external_changes_and_ignore_rules() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Note.md");
        fs::write(&path, "first").unwrap();
        let worker = Index::start(root.path().to_owned());
        loop {
            if matches!(
                worker.rx.recv_timeout(Duration::from_secs(5)).unwrap(),
                Update::Snapshot(_)
            ) {
                break;
            }
        }
        fs::write(&path, "external update").unwrap();
        loop {
            if let Update::Snapshot(snapshot) =
                worker.rx.recv_timeout(Duration::from_secs(5)).unwrap()
                && snapshot
                    .files
                    .get(&path)
                    .is_some_and(|text| text.as_ref() == "external update")
            {
                break;
            }
        }
        fs::write(root.path().join(".gitignore"), "Note.md\n").unwrap();
        loop {
            if let Update::Snapshot(snapshot) =
                worker.rx.recv_timeout(Duration::from_secs(5)).unwrap()
                && !snapshot.files.contains_key(&path)
            {
                break;
            }
        }
    }
    #[test]
    fn worker_indexes_ten_thousand_notes_and_searches_content() {
        let root = tempfile::tempdir().unwrap();
        for n in 0..10_000 {
            fs::write(
                root.path().join(format!("note-{n}.md")),
                if n == 9999 {
                    "unique café phrase"
                } else {
                    "ordinary note"
                },
            )
            .unwrap();
        }
        let start = Instant::now();
        let worker = Index::start(root.path().to_owned());
        worker.search("café".into());
        loop {
            match worker.rx.recv_timeout(Duration::from_secs(30)).unwrap() {
                Update::Search(q, results, _) if q == "café" => {
                    assert_eq!(results.len(), 1);
                    assert!(results[0].0.ends_with("note-9999.md"));
                    break;
                }
                _ => {}
            }
        }
        eprintln!("10,000-note index/search: {:?}", start.elapsed());
    }
}
