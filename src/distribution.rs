//! Per-user native setup UI; PowerShell performs Windows registration off-thread.
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
};
pub fn install_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs/Notes")
}
pub fn script_command(name: &str, script: &str) -> std::io::Result<Command> {
    let dir = std::env::temp_dir().join(format!("Notes-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    std::fs::write(&path, script)?;
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
fn notices() -> String {
    [
        include_str!("../licenses/LaTeX-Rust-NOTICE.txt"),
        include_str!("../licenses/LaTeX-Rust-MIT.txt"),
        include_str!("../licenses/STIX-OFL.txt"),
        include_str!("../licenses/LPPL-1.3c.txt"),
    ]
    .join("\n\n")
}
fn install(source: &Path, associate: bool) -> Result<PathBuf, String> {
    let target = install_dir();
    let mut command = script_command("install.ps1", include_str!("../scripts/install.ps1"))
        .map_err(|e| e.to_string())?;
    command
        .arg("-Source")
        .arg(source)
        .arg("-InstallDir")
        .arg(&target)
        .arg("-Version")
        .arg(env!("CARGO_PKG_VERSION"));
    if associate {
        command.arg("-Associate");
    }
    let result = command.output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into_owned());
    }
    std::fs::write(target.join("LICENSE"), include_str!("../LICENSE"))
        .map_err(|e| e.to_string())?;
    std::fs::write(target.join("THIRD-PARTY-NOTICES.txt"), notices()).map_err(|e| e.to_string())?;
    let licenses = target.join("licenses");
    std::fs::create_dir_all(&licenses).map_err(|e| e.to_string())?;
    for (name, text) in [
        (
            "LaTeX-Rust-NOTICE.txt",
            include_str!("../licenses/LaTeX-Rust-NOTICE.txt"),
        ),
        (
            "LaTeX-Rust-MIT.txt",
            include_str!("../licenses/LaTeX-Rust-MIT.txt"),
        ),
        ("STIX-OFL.txt", include_str!("../licenses/STIX-OFL.txt")),
        ("LPPL-1.3c.txt", include_str!("../licenses/LPPL-1.3c.txt")),
        ("dvipsnames.tsv", include_str!("../licenses/dvipsnames.tsv")),
    ] {
        std::fs::write(licenses.join(name), text).map_err(|e| e.to_string())?;
    }
    Ok(target.join("Notes.exe"))
}
pub fn setup_requested() -> bool {
    std::env::args_os().any(|a| a == "--install")
        || std::env::current_exe().ok().is_some_and(|p| {
            p.file_stem()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("Notes-Setup"))
        })
}
pub fn uninstall_requested() -> bool {
    std::env::args_os().any(|a| a == "--uninstall")
}
pub fn uninstall() -> std::io::Result<()> {
    if rfd::MessageDialog::new()
        .set_title("Uninstall Notes")
        .set_description(
            "Remove Notes and its Windows registration? Your notes and preferences will be kept.",
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        != rfd::MessageDialogResult::Yes
    {
        return Ok(());
    }
    let source = std::env::current_exe()?;
    script_command("uninstall.ps1", include_str!("../scripts/install.ps1"))?
        .arg("-Source")
        .arg(&source)
        .arg("-InstallDir")
        .arg(install_dir())
        .arg("-Uninstall")
        .arg("-ParentPid")
        .arg(std::process::id().to_string())
        .spawn()?;
    Ok(())
}
#[derive(Default)]
pub struct Setup {
    associate: bool,
    receiver: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    installed: Option<PathBuf>,
    error: Option<String>,
}
impl eframe::App for Setup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(debug_assertions)]
        crate::qa::capture(ctx);
        if let Some(result) = self.receiver.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.receiver = None;
            match result {
                Ok(path) => self.installed = Some(path),
                Err(error) => self.error = Some(error),
            }
        }
        ctx.style_mut(|style| {
            style.visuals.panel_fill = egui::Color32::from_gray(17);
            style.visuals.override_text_color = Some(egui::Color32::from_gray(207));
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
        });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_gray(17))
                    .inner_margin(24),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().interact_size.y = 34.0;
                ui.spacing_mut().button_padding = egui::vec2(14.0, 7.0);
                ui.heading("Notes");
                ui.label(format!("Install version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(16.0);
                ui.label("Installs for your Windows account. No administrator access is needed.");
                ui.label(install_dir().display().to_string());
                ui.add_space(12.0);
                ui.checkbox(
                    &mut self.associate,
                    "Add Notes to Open with for Markdown files",
                );
                ui.label("You can choose Notes as the default app in Windows Settings.");
                ui.add_space(16.0);
                if self.receiver.is_some() {
                    ui.spinner();
                    ui.label("Installing Notes…");
                } else if let Some(path) = &self.installed {
                    ui.label("Notes is installed.");
                    if ui.button("Open Notes").clicked() {
                        let _ = Command::new(path).spawn();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else if ui.button("Install").clicked() {
                    self.error = None;
                    let associate = self.associate;
                    let (tx, rx) = mpsc::channel();
                    self.receiver = Some(rx);
                    std::thread::spawn(move || {
                        let result = std::env::current_exe()
                            .map_err(|e| e.to_string())
                            .and_then(|path| install(&path, associate));
                        let _ = tx.send(result);
                    });
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(12.0);
                ui.label("GPL-3.0-only. Third-party notices are installed with the app.");
            });
        if self.receiver.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}
