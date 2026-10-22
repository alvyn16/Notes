mod background;
mod chrome;
mod controls;
mod dialogs;
mod document;
mod file_io;
mod notes;
mod palette;
mod recovery;
mod settings;
#[cfg(test)]
mod tests;
mod view;

use crate::{
    demo, editor, markdown,
    workspace::{self, Document, Entry},
};
use eframe::egui::{
    self, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Id, Key,
    Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2, ViewportCommand,
    pos2, vec2,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MUTED: Color32 = Color32::from_rgb(146, 146, 146);

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    root: PathBuf,
    tabs: Vec<PathBuf>,
    active: usize,
    sidebar: bool,
    show_library: bool,
    check_updates: bool,
    install_updates: bool,
    last_update_check: u64,
    pinned: Vec<PathBuf>,
    recent: Vec<PathBuf>,
    font_size: f32,
    content_width: f32,
    zoom: f32,
    accent: [u8; 3],
    background: [u8; 3],
    foreground: [u8; 3],
    light_background: [u8; 3],
    light_foreground: [u8; 3],
    translucency: f32,
    contrast: f32,
    light: bool,
    top_padding: f32,
    ui_font: String,
    editor_font: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            tabs: vec![],
            active: 0,
            sidebar: true,
            show_library: false,
            check_updates: true,
            install_updates: false,
            last_update_check: 0,
            pinned: vec![],
            recent: vec![],
            font_size: 16.0,
            content_width: 736.0,
            zoom: 1.0,
            accent: [255, 101, 0],
            background: [17, 17, 17],
            foreground: [207, 207, 207],
            light_background: [250, 249, 247],
            light_foreground: [40, 40, 40],
            translucency: 35.0,
            contrast: 16.0,
            light: false,
            top_padding: 126.0,
            ui_font: "Segoe UI".into(),
            editor_font: "Segoe UI".into(),
        }
    }
}

#[derive(Clone)]
enum Dialog {
    NewNote,
    NewFolder,
    Rename(PathBuf),
}

pub struct Notes {
    prefs: Preferences,
    entries: Vec<Entry>,
    docs: Vec<Document>,
    active: usize,
    expanded: BTreeSet<PathBuf>,
    history: Vec<PathBuf>,
    history_pos: usize,
    settings: bool,
    view: view::View,
    palette: bool,
    query: String,
    palette_index: usize,
    focus_palette: bool,
    dialog: Option<Dialog>,
    input: String,
    error: Option<String>,
    notice: String,
    last_edit: Instant,
    last_scan: Instant,
    editor: editor::Editor,
    palette_cache: Vec<(PathBuf, String)>,
    index: Option<crate::index::Index>,
    search_query: String,
    compact: bool,
    selected_paths: BTreeSet<PathBuf>,
    selection_anchor: Option<PathBuf>,
    startup_open: bool,
    recovery: recovery::Recovery,
    edge_blur: crate::edge_blur::EdgeBlur,
    io: file_io::FileIo,
    broker: Option<crate::windows::broker::Broker>,
    updater: crate::updates::Updater,
}

impl Notes {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = FontDefinitions::default();
        for (name, path, family) in [
            (
                "Segoe UI",
                "C:/Windows/Fonts/segoeui.ttf",
                FontFamily::Proportional,
            ),
            (
                "Segoe UI Bold",
                "C:/Windows/Fonts/segoeuib.ttf",
                FontFamily::Name("bold".into()),
            ),
            (
                "Consolas",
                "C:/Windows/Fonts/consola.ttf",
                FontFamily::Monospace,
            ),
        ] {
            if let Ok(data) = fs::read(path) {
                fonts
                    .font_data
                    .insert(name.into(), FontData::from_owned(data).into());
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, name.into());
            }
        }
        for (name, path) in [
            ("Segoe UI", "C:/Windows/Fonts/segoeui.ttf"),
            ("Arial", "C:/Windows/Fonts/arial.ttf"),
            ("Calibri", "C:/Windows/Fonts/calibri.ttf"),
            ("Consolas", "C:/Windows/Fonts/consola.ttf"),
        ] {
            if let Ok(data) = fs::read(path) {
                fonts
                    .font_data
                    .insert(name.into(), FontData::from_owned(data).into());
                let mut family = vec![name.into()];
                family.extend(
                    fonts
                        .families
                        .get(&FontFamily::Proportional)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|fallback| fallback != name),
                );
                fonts.families.insert(FontFamily::Name(name.into()), family);
            }
        }
        for (family, path) in [
            ("Segoe UI", "C:/Windows/Fonts/segoeuib.ttf"),
            ("Arial", "C:/Windows/Fonts/arialbd.ttf"),
            ("Calibri", "C:/Windows/Fonts/calibrib.ttf"),
            ("Consolas", "C:/Windows/Fonts/consolab.ttf"),
        ] {
            let name = format!("bold:{family}");
            let mut fallbacks = fonts
                .families
                .get(&FontFamily::Name(family.into()))
                .cloned()
                .unwrap_or_default();
            if let Ok(data) = fs::read(path) {
                fonts
                    .font_data
                    .insert(name.clone(), FontData::from_owned(data).into());
                fallbacks.insert(0, name.clone());
            }
            fonts
                .families
                .insert(FontFamily::Name(name.into()), fallbacks);
        }
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut prefs: Preferences = cc
            .storage
            .and_then(|s| eframe::get_value(s, "preferences"))
            .unwrap_or_default();
        let cli = std::env::args_os().nth(1).map(PathBuf::from);
        let compact = cli
            .as_ref()
            .is_some_and(|path| path.is_file() && workspace::is_markdown(path));
        let mut error = None;
        if let Some(path) = cli {
            if path.is_dir() {
                prefs.root = path;
                prefs.tabs.clear();
            } else if compact {
                let path = path.canonicalize().unwrap_or(path);
                prefs.root = path.parent().unwrap_or(Path::new(".")).to_owned();
                prefs.tabs = vec![path];
                prefs.sidebar = false;
            } else {
                error =
                    Some("Pass a Markdown file or workspace folder as the first argument.".into());
            }
        }
        if prefs.root.as_os_str().is_empty() || !prefs.root.is_dir() {
            let base = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            prefs.root = base.join("WriterWindows").join("Example Vault");
            if let Err(e) = demo::seed(&prefs.root) {
                error = Some(e.to_string());
            }
        }
        prefs.root = prefs.root.canonicalize().unwrap_or(prefs.root);
        let entries: Vec<Entry> = vec![];
        let last_scan = Instant::now();
        let index = Some(crate::index::Index::start(prefs.root.clone()));
        let mut io = file_io::FileIo::default();
        io.worker = Some(file_io::Worker::start());
        let docs: Vec<_> = prefs
            .tabs
            .iter()
            .filter(|p| p.starts_with(&prefs.root))
            .map(|path| {
                io.loading.insert(path.clone());
                Document {
                    path: path.clone(),
                    text: String::new(),
                    saved: String::new(),
                    conflict: false,
                }
            })
            .collect();
        let active = prefs.active.min(docs.len().saturating_sub(1));
        let expanded = [prefs.root.join("Daily Notes"), prefs.root.join("Projects")]
            .into_iter()
            .collect();
        let history = docs
            .get(active)
            .map(|d| vec![d.path.clone()])
            .unwrap_or_default();
        let startup_open = docs.is_empty();
        let mut app = Self {
            prefs,
            entries,
            docs,
            active,
            expanded,
            history,
            history_pos: 0,
            settings: false,
            view: view::View::default(),
            palette: false,
            query: String::new(),
            palette_index: 0,
            focus_palette: false,
            dialog: None,
            input: String::new(),
            error,
            notice: String::new(),
            last_edit: Instant::now(),
            last_scan,
            editor: editor::Editor::default(),
            palette_cache: vec![],
            index,
            search_query: String::new(),
            compact,
            selected_paths: BTreeSet::new(),
            selection_anchor: None,
            startup_open,
            recovery: recovery::Recovery::default(),
            edge_blur: crate::edge_blur::EdgeBlur::default(),
            io,
            broker: None,
            updater: crate::updates::Updater::default(),
        };
        if std::env::var_os("NOTES_PROFILE_PATH").is_none() {
            let owner = if std::env::args_os().any(|a| a == "--new-window") {
                app.docs
                    .first()
                    .map(|d| d.path.clone())
                    .unwrap_or_else(|| app.prefs.root.clone())
            } else {
                app.prefs.root.clone()
            };
            match crate::windows::broker::Broker::start(&owner) {
                Ok(Some(broker)) => app.broker = Some(broker),
                Ok(None) => {
                    if crate::windows::broker::forward_cli() {
                        app.io.closing_window = true;
                    }
                }
                Err(error) => app.notice = format!("Window sharing unavailable: {error}"),
            }
        }
        for path in app.io.loading.clone() {
            app.queue_load(path);
        }
        app
    }

    fn accent(&self) -> Color32 {
        Color32::from_rgb(
            self.prefs.accent[0],
            self.prefs.accent[1],
            self.prefs.accent[2],
        )
    }
    fn background(&self) -> Color32 {
        if self.prefs.light {
            Color32::from_rgb(
                self.prefs.light_background[0],
                self.prefs.light_background[1],
                self.prefs.light_background[2],
            )
        } else {
            Color32::from_rgb(
                self.prefs.background[0],
                self.prefs.background[1],
                self.prefs.background[2],
            )
        }
    }

    fn style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = if self.prefs.light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        style.visuals.override_text_color = Some(if self.prefs.light {
            Color32::from_rgb(
                self.prefs.light_foreground[0],
                self.prefs.light_foreground[1],
                self.prefs.light_foreground[2],
            )
        } else {
            Color32::from_rgb(
                self.prefs.foreground[0],
                self.prefs.foreground[1],
                self.prefs.foreground[2],
            )
        });
        style.visuals.panel_fill = self.background();
        style.visuals.selection.bg_fill = self.accent();
        style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        style.visuals.hyperlink_color = self.accent();
        let surface_contrast = (self.prefs.contrast / 16.0).clamp(0.0, 2.5);
        let bg = self.background();
        let fg = style.visuals.text_color();
        style.visuals.code_bg_color = crate::theme::code_background(bg, fg);
        style.visuals.window_fill = crate::theme::mix(bg, fg, 0.025);
        style.visuals.extreme_bg_color = crate::theme::mix(bg, fg, 0.04);
        for (widget, amount) in [
            (&mut style.visuals.widgets.inactive, 0.065),
            (&mut style.visuals.widgets.hovered, 0.11),
            (&mut style.visuals.widgets.active, 0.15),
            (&mut style.visuals.widgets.open, 0.10),
        ] {
            widget.bg_fill = crate::theme::mix(bg, fg, amount);
            widget.weak_bg_fill = widget.bg_fill;
            widget.bg_stroke = Stroke::new(
                1.0_f32,
                crate::theme::mix(bg, fg, (0.12 * surface_contrast).clamp(0.04, 0.3)),
            );
            widget.fg_stroke.color = fg;
            widget.corner_radius = egui::CornerRadius::same(6);
            widget.expansion = 0.0;
        }
        style.visuals.window_stroke = Stroke::new(1.0_f32, crate::theme::mix(bg, fg, 0.12));
        style.visuals.widgets.noninteractive.bg_stroke = style.visuals.window_stroke;
        style.spacing.item_spacing = vec2(6.0, 6.0);
        style.spacing.button_padding = vec2(9.0, 5.0);
        style.spacing.interact_size = vec2(28.0, 26.0);
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(14.0, FontFamily::Name(self.prefs.ui_font.clone().into())),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(14.0, FontFamily::Name(self.prefs.ui_font.clone().into())),
        );
        ctx.set_style(style);
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("diagram-appearance"),
                (
                    (self.prefs.contrast / 26.666667).clamp(0.2, 1.0),
                    self.prefs.ui_font.clone(),
                ),
            )
        });
    }

    fn read_note(&mut self) {
        self.view.read();
        self.editor.find.open = false;
        if let Some(doc) = self.docs.get(self.active) {
            self.prefs.recent.retain(|p| p != &doc.path);
            self.prefs.recent.insert(0, doc.path.clone());
            self.prefs.recent.truncate(20);
        }
    }

    fn toggle_editor_mode(&mut self) {
        self.view.toggle();
        if self.view.editing {
            self.editor.focus();
        } else {
            self.editor.find.open = false;
        }
    }

    fn open(&mut self, path: PathBuf, record: bool) {
        self.startup_open = false;
        if let Some(index) = self.docs.iter().position(|d| d.path == path) {
            self.active = index;
        } else {
            if self.io.worker.is_some() {
                self.docs.push(Document {
                    path: path.clone(),
                    text: String::new(),
                    saved: String::new(),
                    conflict: false,
                });
                self.active = self.docs.len() - 1;
                self.queue_load(path.clone());
            } else {
                match Document::load(path.clone()) {
                    Ok(d) => {
                        self.docs.push(d);
                        self.active = self.docs.len() - 1;
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        return;
                    }
                }
            }
        }
        self.settings = false;
        self.read_note();
        if record && self.history.get(self.history_pos) != Some(&path) {
            self.history.truncate(self.history_pos + 1);
            self.history.push(path);
            self.history_pos = self.history.len() - 1;
        }
    }

    fn navigate(&mut self, delta: isize) {
        let pos = self.history_pos as isize + delta;
        if pos >= 0 && pos < (self.history.len() as isize) {
            self.history_pos = pos as usize;
            self.open(self.history[self.history_pos].clone(), false);
        }
    }

    fn detach_tab(&mut self, path: &Path) {
        if self.defer_until_saved(file_io::Deferred::Detach(path.to_owned())) {
            return;
        }
        if !self.save_all() {
            return;
        }
        match crate::windows::open_detached(path) {
            Ok(()) => {
                if let Some(index) = self.docs.iter().position(|d| d.path == path) {
                    self.close_tab(index);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn close_tab(&mut self, index: usize) {
        let path = self.docs[index].path.clone();
        if self.docs[index].conflict {
            self.save_all();
            self.error =
                Some("Resolve this conflict or save a copy before closing the note.".into());
            return;
        }
        if self.io.worker.is_some() && (self.docs[index].dirty() || self.io.saving.contains(&path))
        {
            self.io.closing_tab.insert(path);
            self.save_all();
            return;
        }
        if self.docs[index].dirty()
            && let Err(e) = self.docs[index].save()
        {
            self.error = Some(e.to_string());
            return;
        }
        self.forget_file_io(&path);
        self.docs.remove(index);
        self.read_note();
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.docs.len().saturating_sub(1));
    }

    fn open_workspace(&mut self) {
        if self.defer_until_saved(file_io::Deferred::OpenWorkspace) {
            return;
        }
        if !self.save_all() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open a Markdown workspace")
            .pick_folder()
            && let Err(e) = crate::windows::open(&path)
        {
            self.error = Some(e.to_string());
        }
    }

    fn close_all(&mut self, workspace: bool) {
        if self.defer_until_saved(file_io::Deferred::CloseAll(workspace)) || !self.save_all() {
            return;
        }
        for path in self.docs.iter().map(|d| d.path.clone()).collect::<Vec<_>>() {
            self.forget_file_io(&path);
        }
        self.docs.clear();
        self.active = 0;
        self.read_note();
        if workspace {
            self.history.clear();
            self.history_pos = 0;
        }
    }

    fn save_all(&mut self) -> bool {
        if self.io.worker.is_some() {
            return self.queue_saves();
        }
        let mut success = true;
        let mut saved = false;
        for doc in &mut self.docs {
            if doc.conflict {
                success = false;
                if let Err(e) = doc.preserve_draft() {
                    self.error = Some(format!("Could not preserve the latest draft: {e}"));
                }
            } else if doc.dirty() {
                match doc.save() {
                    Ok(()) => saved = true,
                    Err(e) => {
                        self.error = Some(e.to_string());
                        success = false;
                    }
                }
            }
        }
        if saved {
            self.refresh_entries();
        }
        success
    }

    fn show_palette(&mut self) {
        self.palette = true;
        self.query.clear();
        self.palette_index = 0;
        self.focus_palette = true;
        self.palette_cache.clear();
        self.search_query.clear();
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if crate::media::is_open(ctx)
            || ctx.data(|d| {
                d.get_temp::<bool>(Id::new("diagram-fullscreen-open"))
                    .unwrap_or(false)
            })
        {
            return;
        }
        if self.recovery.is_pending() {
            return;
        }
        let command = |key| egui::KeyboardShortcut::new(egui::Modifiers::CTRL, key);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::N))
        {
            self.open_workspace();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::P)))
            || ctx.input_mut(|i| i.consume_shortcut(&command(Key::O)))
        {
            self.show_palette();
        }
        if self.palette || self.dialog.is_some() {
            return;
        }
        if !self.compact {
            for (index, key) in [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ]
            .into_iter()
            .enumerate()
            {
                if ctx.input_mut(|i| i.consume_shortcut(&command(key))) && index < self.docs.len() {
                    self.active = index;
                    self.settings = false;
                    self.read_note();
                }
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Comma))) {
            self.settings = true;
        }
        if !self.compact && ctx.input_mut(|i| i.consume_shortcut(&command(Key::Backslash))) {
            self.prefs.sidebar = !self.prefs.sidebar;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::N))) {
            self.dialog = Some(Dialog::NewNote);
            self.input.clear();
        }
        if !self.compact && ctx.input_mut(|i| i.consume_shortcut(&command(Key::T))) {
            self.show_palette();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::W))) {
            if self.settings {
                self.settings = false;
            } else if !self.docs.is_empty() {
                self.close_tab(self.active);
            }
        }
        if self.docs.get(self.active).is_some_and(|d| {
            self.io.loading.contains(&d.path) || self.io.resolving.contains(&d.path)
        }) {
            return;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::S))) {
            self.retry_saves();
            if self.save_all() {
                self.notice = "Saved".into();
            }
        }
        let find = ctx.input_mut(|i| i.consume_shortcut(&command(Key::F)));
        let replace = ctx.input_mut(|i| i.consume_shortcut(&command(Key::H)));
        if (find || replace)
            && let Some(doc) = self.docs.get(self.active)
        {
            self.view.find();
            self.settings = false;
            self.editor.open_find(ctx, &doc.path, &doc.text, replace);
        }
        let previous = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                Key::G,
            ))
        }) || ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, Key::F3));
        let next = ctx.input_mut(|i| i.consume_shortcut(&command(Key::G)))
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F3));
        if previous || next {
            self.view.find();
            self.settings = false;
            self.editor.navigate(!previous);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Plus)))
            || ctx.input_mut(|i| i.consume_shortcut(&command(Key::Equals)))
        {
            self.prefs.zoom = (self.prefs.zoom + 0.1).min(3.0);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(Key::Minus))) {
            self.prefs.zoom = (self.prefs.zoom - 0.1).max(0.5);
        }
        if !ctx.input(|i| i.modifiers.alt)
            && ctx.input_mut(|i| i.consume_shortcut(&command(Key::Num0)))
        {
            self.prefs.zoom = 1.0;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F4)) {
            self.toggle_editor_mode();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::ArrowLeft)) {
            self.navigate(-1);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::ArrowRight)) {
            self.navigate(1);
        }
        let previous_tab = ctx
            .input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Tab));
        if (previous_tab || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab)))
            && !self.docs.is_empty()
            && !self.compact
        {
            self.active = (self.active + if previous_tab { self.docs.len() - 1 } else { 1 })
                % self.docs.len();
            self.settings = false;
            self.read_note();
        }
        if self.editor.find.open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.editor.dismiss_find();
            self.view.close_find();
        } else if self.view.editing
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.read_note();
        }
    }

    fn statusbar(&mut self, ui: &mut Ui) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(22.0);
            if !self.settings
                && let Some(doc) = self.docs.get(self.active)
            {
                let paragraphs = doc
                    .text
                    .split("\n\n")
                    .filter(|s| !s.trim().is_empty())
                    .count();
                for label in [
                    format!("{paragraphs} paragraphs"),
                    format!("{} characters", doc.text.chars().count()),
                    format!("{} words", doc.text.split_whitespace().count()),
                ] {
                    ui.label(RichText::new(label).size(12.5).color(MUTED));
                    ui.add_space(14.0);
                }
            }
            if !self.notice.is_empty() {
                ui.label(RichText::new(&self.notice).small().color(MUTED));
            }
        });
    }
}

impl eframe::App for Notes {
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(gl) = gl {
            self.edge_blur.destroy(gl);
        }
    }
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Text edits are mirrored into the document every frame. Persisting
        // the session must not end the active edit or steal keyboard focus.
        self.save_all();
        self.prefs.tabs = self.docs.iter().map(|d| d.path.clone()).collect();
        self.prefs.active = self.active;
        eframe::set_value(storage, "preferences", &self.prefs);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        crate::windows::check_startup_size(ctx);
        self.style(ctx);
        self.editor.apply_path_changes(ctx);
        self.poll_file_io(ctx);
        self.updater.poll(self.prefs.install_updates);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if self.prefs.check_updates
            && !crate::windows::broker::isolated()
            && now.saturating_sub(self.prefs.last_update_check) > 86400
            && !self.updater.busy()
        {
            self.prefs.last_update_check = now;
            self.updater.check();
        }
        let requests: Vec<_> = self
            .broker
            .as_ref()
            .map(|b| b.requests().collect())
            .unwrap_or_default();
        for path in requests {
            if let Some(path) = path
                && workspace::is_markdown(&path)
            {
                self.compact = false;
                self.open(path, true);
            }
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        #[cfg(debug_assertions)]
        if let Some(scene) = crate::qa::scene(ctx) {
            if scene == "palette" {
                self.show_palette();
            } else if scene == "image-viewer" {
                crate::media::capture_viewer(ctx);
            } else if scene == "diagram-viewer" {
            } else if scene == "settings" {
                self.settings = true;
            } else if scene == "library" {
                self.prefs.sidebar = true;
                self.compact = false;
                self.prefs.show_library = true;
                if let Some(doc) = self.docs.first() {
                    self.prefs.pinned = vec![doc.path.clone()];
                }
            } else {
                self.docs.clear();
                self.startup_open = false;
                self.index = None;
                self.prefs.sidebar = false;
                if scene == "new-note" {
                    self.dialog = Some(Dialog::NewNote);
                    self.input.clear();
                }
            }
        }
        #[cfg(debug_assertions)]
        crate::qa::capture(ctx);
        for path in ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        }) {
            if path.is_file() && workspace::is_markdown(&path) {
                if path.starts_with(&self.prefs.root) && !self.compact {
                    self.open(path, true);
                } else if let Err(e) = crate::windows::open(&path) {
                    self.error = Some(e.to_string());
                }
            }
        }
        self.shortcuts(ctx);
        ctx.data_mut(|d| d.insert_temp(Id::new("diagram-fullscreen-open"), false));
        if self.last_edit.elapsed() > Duration::from_millis(650) {
            self.save_all();
        }
        self.poll_index(ctx);
        ctx.request_repaint_after(Duration::from_millis(300));
        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 12, self.background());
                ui.painter().rect_stroke(
                    rect.shrink(0.5),
                    12,
                    Stroke::new(1.0_f32, Color32::from_gray(60)),
                    egui::StrokeKind::Inside,
                );
                let side = if self.prefs.sidebar {
                    240.0_f32.min(rect.width() * 0.32)
                } else {
                    0.0
                };
                if side > 0.0 {
                    let sr = Rect::from_min_max(
                        rect.min + vec2(1.0, 1.0),
                        pos2(rect.left() + side, rect.bottom() - 1.0),
                    );
                    ui.scope_builder(UiBuilder::new().max_rect(sr), |ui| self.sidebar(ui));
                    ui.painter().line_segment(
                        [
                            pos2(rect.left() + side, rect.top()),
                            pos2(rect.left() + side, rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, Color32::from_gray(39)),
                    );
                }
                let left = rect.left() + side;
                let title = Rect::from_min_max(
                    pos2(left + 1.0, rect.top() + 2.0),
                    pos2(rect.right() - 149.0, rect.top() + 54.0),
                );
                ui.scope_builder(UiBuilder::new().max_rect(title), |ui| self.toolbar(ui));
                self.window_controls(ui, rect);
                let content = Rect::from_min_max(
                    pos2(left + 1.0, rect.top() + 54.0),
                    pos2(rect.right() - 5.0, rect.bottom() - 31.0),
                );
                ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
                    ui.set_clip_rect(content.intersect(ui.clip_rect()));
                    if self.settings {
                        self.settings_ui(ui);
                    } else {
                        self.document(ui);
                    }
                });
                if !self.settings && !self.docs.is_empty() {
                    self.edge_blur.show(ui, content, self.background());
                }
                let status =
                    Rect::from_min_max(pos2(left, rect.bottom() - 30.0), rect.max - vec2(3.0, 3.0));
                ui.scope_builder(UiBuilder::new().max_rect(status), |ui| self.statusbar(ui));
                let resize =
                    Rect::from_min_size(rect.right_bottom() - vec2(12.0, 12.0), vec2(12.0, 12.0));
                let response = ui
                    .interact(resize, Id::new("resize"), Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                if response.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(
                        egui::ResizeDirection::SouthEast,
                    ));
                }
            });
        crate::media::viewer(ctx);
        if self.palette {
            self.palette_ui(ctx);
        }
        if self.dialog.is_some() {
            self.dialog_ui(ctx);
        }
        self.recovery_ui(ctx);
        if let Some(error) = self.error.clone() {
            egui::Window::new("Notes — could not complete action")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.save_all()
            && self.updater.install_on_exit
            && let Err(error) = self.updater.apply()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.error = Some(format!("Could not start update: {error}"));
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.save_all() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.docs.iter().any(|d| d.conflict) {
                self.error = Some(
                    "Resolve the external-edit conflict or save a copy before closing Notes."
                        .into(),
                );
            } else {
                self.io.closing_window = true;
                self.notice = "Saving before closing...".into();
            }
        }
    }
}
