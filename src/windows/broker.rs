//! One discoverable owner per workspace. Explicit tear-offs bypass ownership.
//! Endpoints live in the user's AppData; requests require an OS-random token.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Serialize, Deserialize)]
struct Endpoint {
    root: PathBuf,
    port: u16,
    token: String,
    pid: u32,
    default_window: bool,
}
#[derive(Serialize, Deserialize)]
struct Request {
    token: String,
    path: Option<PathBuf>,
}
pub struct Broker {
    _lock: File,
    endpoint: PathBuf,
    stop: Arc<AtomicBool>,
    rx: mpsc::Receiver<Option<PathBuf>>,
}
impl Drop for Broker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = fs::remove_file(&self.endpoint);
    }
}
fn directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Notes/Windows")
}
pub fn isolated() -> bool {
    std::env::var_os("NOTES_PROFILE_PATH").is_some()
        || std::env::args_os().any(|a| a == "--new-window")
}
pub fn forward_cli() -> bool {
    if isolated() {
        return false;
    }
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    forward(&directory(), path.as_deref())
}
fn forward(dir: &Path, path: Option<&Path>) -> bool {
    let path = match path {
        Some(p) => match p.canonicalize() {
            Ok(p) => Some(p),
            Err(_) => return false,
        },
        None => None,
    };
    let mut endpoints: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|s| s == "json"))
        .filter_map(|e| {
            fs::read(e.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<Endpoint>(&b).ok())
        })
        .filter(|e| {
            path.as_ref().map_or(e.default_window, |p| {
                p == &e.root || (p.starts_with(&e.root) && crate::workspace::is_markdown(p))
            })
        })
        .collect();
    endpoints.sort_by_key(|e| std::cmp::Reverse(e.root.components().count()));
    endpoints
        .into_iter()
        .any(|endpoint| send(&endpoint, path.clone()))
}
fn send(endpoint: &Endpoint, path: Option<PathBuf>) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], endpoint.port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    #[cfg(windows)]
    unsafe {
        allow_foreground(endpoint.pid);
    }
    let Ok(mut bytes) = serde_json::to_vec(&Request {
        token: endpoint.token.clone(),
        path,
    }) else {
        return false;
    };
    bytes.push(b'\n');
    if stream.write_all(&bytes).is_err() {
        return false;
    }
    let mut response = [0];
    stream.read_exact(&mut response).is_ok() && response[0] == 1
}
#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "AllowSetForegroundWindow"]
    fn allow_foreground(pid: u32) -> i32;
}
impl Broker {
    pub fn start(root: &Path) -> io::Result<Option<Self>> {
        Self::start_in(&directory(), root, std::env::args_os().nth(1).is_none())
    }
    fn start_in(dir: &Path, root: &Path, default_window: bool) -> io::Result<Option<Self>> {
        use std::hash::{Hash, Hasher};
        fs::create_dir_all(dir)?;
        let root = root.canonicalize()?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hash);
        let stem = format!("{:x}", hash.finish());
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = match options.open(dir.join(format!("{stem}.lock"))) {
            Ok(lock) => lock,
            Err(e) if e.raw_os_error() == Some(32) => return Ok(None),
            Err(e) => return Err(e),
        };
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        let token: String = random.iter().map(|n| format!("{n:02x}")).collect();
        let endpoint = Endpoint {
            root: root.clone(),
            port: listener.local_addr()?.port(),
            token: token.clone(),
            pid: std::process::id(),
            default_window,
        };
        let endpoint_path = dir.join(format!("{stem}.json"));
        let bytes = serde_json::to_vec(&endpoint)?;
        let temp = dir.join(format!("{stem}-{}.tmp", std::process::id()));
        fs::write(&temp, bytes)?;
        fs::rename(temp, &endpoint_path)?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        std::thread::spawn(move || {
            while !cancelled.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
                        let request = read_request(&mut stream).filter(|r| r.token == token);
                        let valid = request.filter(|r| {
                            r.path.as_ref().is_none_or(|p| {
                                p == &root
                                    || (p.starts_with(&root) && crate::workspace::is_markdown(p))
                            })
                        });
                        let accepted = valid.is_some_and(|r| tx.send(r.path).is_ok());
                        let _ = stream.write_all(&[u8::from(accepted)]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(30))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Some(Self {
            _lock: lock,
            endpoint: endpoint_path,
            stop,
            rx,
        }))
    }
    pub fn requests(&self) -> impl Iterator<Item = Option<PathBuf>> + '_ {
        self.rx.try_iter()
    }
}
fn read_request(stream: &mut TcpStream) -> Option<Request> {
    use std::io::BufRead;
    let mut bytes = Vec::new();
    std::io::BufReader::new(stream.take(65536))
        .read_until(b'\n', &mut bytes)
        .ok()?;
    if bytes.last() != Some(&b'\n') {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_owner_routes_files_authenticates_and_restarts_after_close() {
        let dir = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let note = crate::workspace::create(root.path(), "Note", false).unwrap();
        let broker = Broker::start_in(dir.path(), root.path(), true)
            .unwrap()
            .unwrap();
        assert!(
            Broker::start_in(dir.path(), root.path(), true)
                .unwrap()
                .is_none()
        );
        assert!(forward(dir.path(), Some(&note)));
        assert_eq!(broker.requests().next().unwrap(), Some(note));
        let mut endpoint: Endpoint =
            serde_json::from_slice(&fs::read(&broker.endpoint).unwrap()).unwrap();
        endpoint.token = "wrong token".into();
        assert!(!send(&endpoint, None));
        assert!(broker.requests().next().is_none());
        drop(broker);
        assert!(
            Broker::start_in(dir.path(), root.path(), true)
                .unwrap()
                .is_some()
        );
    }
}
