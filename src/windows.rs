pub mod broker;
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

const DEFAULT_SIZE: eframe::egui::Vec2 = eframe::egui::vec2(1200.0, 800.0);
const MIN_SIZE: eframe::egui::Vec2 = eframe::egui::vec2(620.0, 420.0);

fn invalid_size(size: eframe::egui::Vec2) -> bool {
    !size.is_finite() || size.x < MIN_SIZE.x || size.y < MIN_SIZE.y
}

/// Eframe restores geometry after applying the initial viewport options. Check
/// the final builder, so a damaged saved size cannot override our minimum.
pub fn validate_viewport(
    mut viewport: eframe::egui::ViewportBuilder,
) -> eframe::egui::ViewportBuilder {
    if viewport.inner_size.is_some_and(invalid_size) {
        viewport = viewport
            .with_inner_size(DEFAULT_SIZE)
            .with_position([40.0, 40.0]);
    }
    viewport.with_min_inner_size(MIN_SIZE)
}

/// Some Windows launch states can override the initial builder. Repair once
/// after the actual native rectangle exists, without interfering with resize.
pub fn check_startup_size(ctx: &eframe::egui::Context) {
    use eframe::egui::{Id, ViewportCommand};
    let checked = Id::new("startup-window-size-checked");
    if ctx.data(|data| data.get_temp::<bool>(checked).unwrap_or(false)) {
        return;
    }
    let info = ctx.input(|i| i.viewport().clone());
    if info.minimized == Some(true) {
        return;
    }
    let Some(rect) = info.inner_rect else { return };
    ctx.data_mut(|data| data.insert_temp(checked, true));
    if invalid_size(rect.size()) {
        let size = info.monitor_size.map_or(DEFAULT_SIZE, |monitor| {
            DEFAULT_SIZE.min((monitor - eframe::egui::vec2(80.0, 100.0)).max(MIN_SIZE))
        });
        ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MIN_SIZE));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(eframe::egui::pos2(
            40.0, 40.0,
        )));
    }
}

pub fn profile_path() -> Option<PathBuf> {
    std::env::var_os("NOTES_PROFILE_PATH")
        .or_else(|| std::env::var_os("WRITER_PROFILE_PATH"))
        .map(PathBuf::from)
        .or_else(|| {
            eframe::storage_dir("Writer").map(|root| match std::env::args_os().nth(1) {
                Some(path) => {
                    let name = profile_name(&PathBuf::from(path));
                    if std::env::args_os().any(|a| a == "--new-window") {
                        root.join(format!("detached-{}-{name}", std::process::id()))
                    } else {
                        root.join(name)
                    }
                }
                None => root.join("app.ron"),
            })
        })
}
fn profile_name(path: &Path) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.canonicalize()
        .unwrap_or_else(|_| path.to_owned())
        .hash(&mut hash);
    format!("workspace-{:x}.ron", hash.finish())
}
pub fn open(path: &Path) -> std::io::Result<()> {
    launch(path, false)
}
pub fn open_detached(path: &Path) -> std::io::Result<()> {
    launch(path, true)
}
fn launch(path: &Path, detached: bool) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    if detached {
        cmd.arg(path).arg("--new-window");
    } else {
        cmd.arg(path);
    }
    cmd.env_remove("NOTES_PROFILE_PATH")
        .env_remove("WRITER_PROFILE_PATH");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.spawn()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_startup_repairs_small_rect_once_and_fits_the_monitor() {
        use eframe::egui::{Context, Pos2, RawInput, Rect, ViewportCommand, ViewportId, vec2};
        let ctx = Context::default();
        let mut input = RawInput::default();
        let viewport = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        viewport.inner_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(159.2, 64.0)));
        viewport.monitor_size = Some(vec2(1000.0, 700.0));
        let output = ctx.run(input.clone(), check_startup_size);
        let commands = &output.viewport_output[&ViewportId::ROOT].commands;
        assert!(commands.iter().any(|command| matches!(command, ViewportCommand::InnerSize(size) if *size == vec2(920.0, 600.0))));
        let output = ctx.run(input, check_startup_size);
        assert!(
            !output.viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, ViewportCommand::InnerSize(_)))
        );
    }

    #[test]
    fn corrupt_saved_geometry_is_repaired_but_normal_geometry_is_preserved() {
        use eframe::egui::{ViewportBuilder, pos2, vec2};
        for size in [vec2(159.2, 64.0), vec2(0.0, 0.0), vec2(f32::NAN, 800.0)] {
            let viewport = validate_viewport(ViewportBuilder::default().with_inner_size(size));
            assert_eq!(viewport.inner_size, Some(DEFAULT_SIZE));
            assert_eq!(viewport.min_inner_size, Some(MIN_SIZE));
        }
        let viewport = validate_viewport(
            ViewportBuilder::default()
                .with_inner_size([900.0, 600.0])
                .with_position([120.0, 90.0]),
        );
        assert_eq!(viewport.inner_size, Some(vec2(900.0, 600.0)));
        assert_eq!(viewport.position, Some(pos2(120.0, 90.0)));
    }
    #[test]
    fn workspaces_and_single_files_have_independent_sessions() {
        assert_ne!(
            profile_name(Path::new("one")),
            profile_name(Path::new("two"))
        );
        assert_ne!(
            profile_name(Path::new("one")),
            profile_name(Path::new("one/note.md"))
        );
        assert_eq!(
            profile_name(Path::new("one")),
            profile_name(Path::new("one"))
        );
    }
}
