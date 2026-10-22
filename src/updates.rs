use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Deserialize)]
pub struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}
#[derive(Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
pub struct Staged {
    pub path: PathBuf,
    pub hash: String,
    pub version: String,
}
enum ResultMessage {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<Staged, String>),
}
#[derive(Default)]
pub struct Updater {
    rx: Option<mpsc::Receiver<ResultMessage>>,
    pub available: Option<Release>,
    pub staged: Option<Staged>,
    pub message: String,
    pub install_on_exit: bool,
    pub restart: bool,
}
fn version(tag: &str) -> Option<(u64, u64, u64)> {
    let parts = tag
        .trim_start_matches('v')
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match parts.as_slice() {
        [a, b, c] => Some((*a, *b, *c)),
        _ => None,
    }
}
fn candidate(release: &Release, current: &str) -> bool {
    !release.draft
        && !release.prerelease
        && version(&release.tag_name)
            .zip(version(current))
            .is_some_and(|(new, old)| new > old)
}
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(90)))
        .build()
        .into()
}
fn check() -> Result<Option<Release>, String> {
    let mut response = match agent()
        .get("https://api.github.com/repos/alvyn16/Notes/releases/latest")
        .header("User-Agent", "Notes-Windows")
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let release: Release = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json()
        .map_err(|e| e.to_string())?;
    Ok(candidate(&release, env!("CARGO_PKG_VERSION")).then_some(release))
}
fn trusted_asset(asset: &Asset) -> Result<String, String> {
    if asset.name != "Notes.exe"
        || !asset
            .browser_download_url
            .starts_with("https://github.com/alvyn16/Notes/releases/download/")
        || asset.size == 0
        || asset.size > 120 * 1024 * 1024
    {
        return Err("Release has no supported Windows executable.".into());
    }
    let hash = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or("Release is missing its SHA-256 digest.")?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid release checksum.".into());
    }
    Ok(hash.to_ascii_lowercase())
}
fn download(release: &Release) -> Result<Staged, String> {
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "Notes.exe")
        .ok_or("Release has no Notes.exe asset.")?;
    let expected = trusted_asset(asset)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let path = exe.with_file_name(format!(".Notes-update-{suffix}.exe"));
    let result = (|| {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let response = agent()
            .get(&asset.browser_download_url)
            .header("User-Agent", "Notes-Windows")
            .call()
            .map_err(|e| e.to_string())?;
        let mut reader = response.into_body().into_reader().take(asset.size + 1);
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > asset.size {
                return Err("Download exceeds the release size.".into());
            }
            hash.update(&buffer[..n]);
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        let actual = format!("{:x}", hash.finalize());
        if size != asset.size || actual != expected {
            return Err("Update checksum verification failed.".into());
        }
        Ok(Staged {
            path: path.clone(),
            hash: actual,
            version: release.tag_name.trim_start_matches('v').to_owned(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
    }
    result
}
impl Updater {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn check(&mut self) {
        if self.busy() {
            return;
        }
        self.message = "Checking for updates…".into();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(ResultMessage::Checked(check()));
        });
    }
    fn download(&mut self) {
        let Some(release) = self.available.clone() else {
            return;
        };
        self.message = "Downloading and verifying update…".into();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(ResultMessage::Downloaded(download(&release)));
        });
    }
    pub fn poll(&mut self, auto_install: bool) {
        let Some(result) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return;
        };
        self.rx = None;
        match result {
            ResultMessage::Checked(Ok(Some(release))) => {
                self.message = format!("Notes {} is available.", release.tag_name);
                self.available = Some(release);
                if auto_install {
                    self.download();
                }
            }
            ResultMessage::Checked(Ok(None)) => {
                self.message = "No newer published release is available.".into()
            }
            ResultMessage::Downloaded(Ok(staged)) => {
                self.staged = Some(staged);
                self.install_on_exit = auto_install;
                self.message = "Update verified. Ready to install when Notes closes.".into();
            }
            ResultMessage::Checked(Err(error)) | ResultMessage::Downloaded(Err(error)) => {
                self.message = format!("Update unavailable: {error}")
            }
        }
    }
    pub fn ui(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        ui.label(format!("Notes {}", env!("CARGO_PKG_VERSION")));
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
        if self.busy() {
            ui.spinner();
            return false;
        }
        if ui.button("Check for updates").clicked() {
            self.check();
        }
        if self.available.is_some()
            && self.staged.is_none()
            && ui.button("Download update").clicked()
        {
            self.download();
        }
        if self.staged.is_some() {
            ui.checkbox(&mut self.install_on_exit, "Install when Notes closes");
            ui.label("Close other Notes windows before installing. A backup of the previous version is kept.");
            if ui.button("Install update and restart").clicked() {
                return true;
            }
        }
        false
    }
    pub fn apply(&mut self) -> std::io::Result<()> {
        let Some(staged) = &self.staged else {
            return Ok(());
        };
        let mut command = crate::distribution::script_command(
            "apply-update.ps1",
            include_str!("../scripts/apply-update.ps1"),
        )?;
        command
            .arg("-Source")
            .arg(&staged.path)
            .arg("-Target")
            .arg(std::env::current_exe()?)
            .arg("-Sha256")
            .arg(&staged.hash)
            .arg("-ParentPid")
            .arg(std::process::id().to_string())
            .arg("-Version")
            .arg(&staged.version);
        if self.restart {
            command.arg("-Restart");
        }
        let log_dir = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Notes");
        fs::create_dir_all(&log_dir)?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("updates.log"))?;
        command
            .stdout(std::process::Stdio::from(log.try_clone()?))
            .stderr(std::process::Stdio::from(log));
        command.spawn()?;
        self.staged = None;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_reject_downgrades_prereleases_untrusted_assets_and_missing_hashes() {
        let mut release = Release {
            tag_name: "v0.3.0".into(),
            draft: false,
            prerelease: false,
            assets: vec![],
        };
        assert!(candidate(&release, "0.2.0"));
        assert!(!candidate(&release, "0.4.0"));
        release.prerelease = true;
        assert!(!candidate(&release, "0.2.0"));
        let mut asset = Asset {
            name: "Notes.exe".into(),
            browser_download_url: "https://evil.example/Notes.exe".into(),
            digest: Some(format!("sha256:{}", "a".repeat(64))),
            size: 20,
        };
        assert!(trusted_asset(&asset).is_err());
        asset.browser_download_url =
            "https://github.com/alvyn16/Notes/releases/download/v0.3.0/Notes.exe".into();
        assert!(trusted_asset(&asset).is_ok());
        asset.digest = None;
        assert!(trusted_asset(&asset).is_err());
    }
}
