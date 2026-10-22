pub mod commands;
mod layout;
mod lines;
mod multiple;
pub mod search;
mod widgets;

use commands::{Command, Selection, byte_index, char_index};
use eframe::egui::{
    self, Align, Color32, Context, FontId, Id, ImeEvent, Key, KeyboardShortcut, Modifiers,
    RichText, TextEdit, Ui,
};
use egui::{
    text::{CCursor, CCursorRange},
    widgets::text_edit::TextEditState,
};
use layout::{Appearance, Cache};
use search::Find;
use std::path::{Path, PathBuf};

pub struct Options {
    pub font: String,
    pub size: f32,
    pub accent: Color32,
    pub width: f32,
}

#[derive(Default)]
pub struct Editor {
    pub find: Find,
    cache: Cache,
    last_path: Option<PathBuf>,
    reveal: Option<usize>,
    focus: bool,
    composing: bool,
    navigation: Option<bool>,
    status: String,
    multiple: multiple::Multiple,
    pub link: Option<String>,
    reflow: bool,
    path_changes: Vec<(PathBuf, PathBuf)>,
    text_changes: Vec<(PathBuf, String, String)>,
    anchor: Option<(usize, f32, u8)>,
    last_layout: Option<(PathBuf, std::sync::Arc<egui::Galley>)>,
}

pub fn id(path: &Path) -> Id {
    Id::new(("document-editor", path))
}
fn cursor(selection: Selection) -> CCursorRange {
    CCursorRange::two(CCursor::new(selection.anchor), CCursor::new(selection.head))
}
fn selection(ctx: &Context, path: &Path, text: &str) -> Selection {
    TextEditState::load(ctx, id(path))
        .and_then(|s| s.cursor.char_range())
        .map_or(Selection::default(), |s| Selection {
            anchor: s.secondary.index,
            head: s.primary.index,
        })
        .clamp(text)
}
pub fn reset_document(ctx: &Context, path: &Path) {
    TextEditState::default().store(ctx, id(path));
}

/// All non-typing edits share TextEdit's undo stack, including replace-all.
pub fn transaction(
    ctx: &Context,
    path: &Path,
    text: &mut String,
    transform: impl FnOnce(&mut String, Selection) -> Selection,
) -> bool {
    let mut state = TextEditState::load(ctx, id(path)).unwrap_or_default();
    let before = selection(ctx, path, text);
    let original = text.clone();
    let after = transform(text, before).clamp(text);
    if *text == original {
        return false;
    }
    let mut undoer = state.undoer();
    undoer.add_undo(&(cursor(before), original));
    // feed_state clears the redo branch for a fresh edit. add_undo alone
    // records a boundary but deliberately leaves that old branch intact.
    undoer.feed_state(ctx.input(|i| i.time), &(cursor(after), text.clone()));
    undoer.add_undo(&(cursor(after), text.clone()));
    state.set_undoer(undoer);
    state.cursor.set_char_range(Some(cursor(after)));
    state.store(ctx, id(path));
    true
}

impl Editor {
    pub fn record_rewrite(&mut self, path: &Path, before: &str, after: &str) {
        self.text_changes
            .push((path.to_owned(), before.into(), after.into()));
    }
    pub fn remap_path(&mut self, old: &Path, new: &Path) {
        for (path, _, _) in &mut self.text_changes {
            if path == old {
                *path = new.to_owned();
            }
        }
        self.path_changes.push((old.to_owned(), new.to_owned()));
        if self.last_path.as_deref() == Some(old) {
            self.last_path = Some(new.to_owned());
        }
    }
    pub fn apply_path_changes(&mut self, ctx: &Context) {
        for (old, new) in self.path_changes.drain(..) {
            if old == new {
                continue;
            }
            if let Some(state) = TextEditState::load(ctx, id(&old)) {
                state.store(ctx, id(&new));
            }
            ctx.data_mut(|d| d.remove::<TextEditState>(id(&old)));
            if ctx.memory(|m| m.has_focus(id(&old))) {
                ctx.memory_mut(|m| m.request_focus(id(&new)));
            }
        }
        for (path, mut before, after) in self.text_changes.drain(..) {
            transaction(ctx, &path, &mut before, |text, selected| {
                *text = after;
                selected
            });
        }
    }
    pub fn reset_external(&mut self, ctx: &Context, path: &Path) {
        reset_document(ctx, path);
        if self.last_path.as_deref() == Some(path) {
            self.multiple.clear();
            self.last_path = None;
            self.reveal = None;
            self.reflow = false;
        }
    }
    pub fn open_find(&mut self, ctx: &Context, path: &Path, text: &str, replace: bool) {
        self.multiple.clear();
        let selected = selection(ctx, path, text).range();
        if !selected.is_empty() {
            let selected = &text[byte_index(text, selected.start)..byte_index(text, selected.end)];
            if !selected.contains(['\n', '\r']) && selected.chars().count() <= 200 {
                self.find.query = selected.into();
            }
        }
        self.find.open = true;
        self.find.replace_open = replace;
        self.find.focus_query = true;
        self.find.refresh(text);
        let start = byte_index(text, selected.start);
        self.find.select_from(start);
        self.reveal = self.find.active().map(|r| char_index(text, r.start));
    }
    pub fn navigate(&mut self, forward: bool) {
        self.find.open = true;
        self.navigation = Some(forward);
    }
    pub fn dismiss_find(&mut self) {
        self.multiple.clear();
        self.find.open = false;
        self.find.focus_query = false;
        self.focus = true;
    }
    pub fn focus(&mut self) {
        self.focus = true;
    }
    pub fn begin_at(&mut self, ctx: &Context, path: &Path, character: usize) {
        self.multiple.clear();
        self.select(ctx, path, Selection::caret(character));
        self.focus = true;
        self.reveal = Some(character);
    }
    /// Keep a clicked source position at its current screen height on unfolding.
    pub fn anchor_at(&mut self, character: usize, screen_y: f32) {
        self.anchor = Some((character, screen_y, 4));
        self.reveal = None;
    }
    pub fn begin_block(
        &mut self,
        ctx: &Context,
        path: &Path,
        text: &str,
        block: &crate::markdown::Block,
    ) {
        let start = char_index(text, block.range.start);
        self.begin_at(ctx, path, start);
        if matches!(&block.kind, crate::markdown::Kind::Code(language) if crate::markdown::is_mermaid(language))
        {
            self.select(
                ctx,
                path,
                Selection {
                    anchor: char_index(text, block.range.end),
                    head: start,
                },
            );
        }
    }
    fn select(&self, ctx: &Context, path: &Path, selection: Selection) {
        let mut state = TextEditState::load(ctx, id(path)).unwrap_or_default();
        state.cursor.set_char_range(Some(cursor(selection)));
        state.store(ctx, id(path));
    }
    fn jump_to_match(&mut self, ctx: &Context, path: &Path, text: &str) {
        if let Some(range) = self.find.active() {
            let start = char_index(text, range.start);
            let end = char_index(text, range.end);
            self.select(
                ctx,
                path,
                Selection {
                    anchor: start,
                    head: end,
                },
            );
            self.reveal = Some(start);
        }
    }

    /// Render above the scroll area so finding never scrolls the controls away.
    pub fn find_bar(&mut self, ui: &mut Ui, path: &Path, text: &mut String) -> bool {
        if !self.find.open {
            return false;
        }
        self.find.refresh(text);
        let mut changed_query = false;
        let mut step = None;
        let mut replace_one = false;
        let mut replace_all = false;
        let mut close = false;
        if ui.ctx().memory(|m| m.has_focus(Id::new("find-query"))) {
            if ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter)) {
                step = Some(false);
            } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                step = Some(true);
            }
        }
        if ui.ctx().memory(|m| m.has_focus(Id::new("replace-text")))
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            replace_one = true;
        }
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(14, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut self.find.query)
                            .id(Id::new("find-query"))
                            .hint_text("Find in note")
                            .desired_width(190.0),
                    );
                    if self.find.focus_query {
                        response.request_focus();
                        self.find.focus_query = false;
                    }
                    changed_query |= response.changed();
                    let count = self.find.matches.len();
                    ui.label(if count == 0 {
                        "No matches".into()
                    } else {
                        format!("{} / {count}", self.find.current.unwrap_or(0) + 1)
                    });
                    if ui
                        .add_enabled(count > 0, egui::Button::new("Prev"))
                        .on_hover_text("Previous match · Shift+F3")
                        .clicked()
                    {
                        step = Some(false);
                    }
                    if ui
                        .add_enabled(count > 0, egui::Button::new("Next"))
                        .on_hover_text("Next match · F3")
                        .clicked()
                    {
                        step = Some(true);
                    }
                    changed_query |= ui
                        .checkbox(&mut self.find.match_case, "Match case")
                        .changed();
                    changed_query |= ui
                        .checkbox(&mut self.find.whole_word, "Whole words")
                        .changed();
                    changed_query |= ui
                        .checkbox(&mut self.find.regex, "Regex")
                        .on_hover_text("Regular expressions; replacement supports $1 and ${name}")
                        .changed();
                    if ui
                        .selectable_label(self.find.replace_open, "Replace")
                        .clicked()
                    {
                        self.find.replace_open = !self.find.replace_open;
                    }
                    if ui
                        .button("Close")
                        .on_hover_text("Close find · Escape")
                        .clicked()
                    {
                        close = true;
                    }
                });
                if let Some(error) = &self.find.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if self.find.replace_open {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut self.find.replacement)
                                .id(Id::new("replace-text"))
                                .hint_text("Replace with")
                                .desired_width(190.0),
                        );
                        let enabled = !self.find.matches.is_empty();
                        replace_one |= ui
                            .add_enabled(enabled, egui::Button::new("Replace one"))
                            .clicked();
                        replace_all |= ui
                            .add_enabled(enabled, egui::Button::new("Replace all"))
                            .clicked();
                        if !self.status.is_empty() {
                            ui.label(RichText::new(&self.status).small());
                        }
                    });
                }
            });
        if changed_query {
            self.status.clear();
            self.find.current = None;
            self.find.refresh(text);
            self.jump_to_match(ui.ctx(), path, text);
        }
        if let Some(forward) = step {
            self.find.next(forward);
            self.jump_to_match(ui.ctx(), path, text);
        }
        let mut changed = false;
        if replace_one || replace_all {
            let find = &mut self.find;
            let mut count = 0;
            changed = transaction(ui.ctx(), path, text, |text, current| {
                count = if replace_all {
                    find.replace_all(text)
                } else {
                    usize::from(find.replace_one(text))
                };
                find.active().map_or(current, |r| Selection {
                    anchor: char_index(text, r.start),
                    head: char_index(text, r.end),
                })
            });
            self.status = format!("Replaced {count}");
            self.jump_to_match(ui.ctx(), path, text);
        }
        if close {
            self.dismiss_find();
        }
        changed
    }

    pub fn show(&mut self, ui: &mut Ui, path: &Path, text: &mut String, options: Options) -> bool {
        crate::markdown::prepare_document(ui.ctx(), path, text);
        self.apply_path_changes(ui.ctx());
        if self.reflow {
            self.reflow = false;
        }
        let Options {
            font,
            size,
            accent,
            width,
        } = options;
        if self.last_path.as_deref() != Some(path) {
            self.multiple.clear();
            self.last_path = Some(path.to_owned());
            self.find.current = None;
            self.composing = false;
            self.find.refresh(text);
            self.find.select_from(0);
        }
        self.find.refresh(text);
        if let Some(forward) = self.navigation.take() {
            self.find.next(forward);
            self.jump_to_match(ui.ctx(), path, text);
        }
        if self.focus {
            ui.ctx().memory_mut(|m| m.request_focus(id(path)));
            if self.reveal.is_none() && self.anchor.is_none() {
                self.reveal = Some(selection(ui.ctx(), path, text).head);
            }
            self.focus = false;
        }
        let mut changed = false;
        if ui.ctx().memory(|m| m.has_focus(id(path))) {
            changed |= self.multiple.input(ui.ctx(), path, text);
            let selected = selection(ui.ctx(), path, text);
            if !self.composing && ui.input(|i| i.key_pressed(Key::Backspace)) {
                let mut candidate = text.clone();
                if let Some(after) = lines::backspace_list(&mut candidate, selected) {
                    ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
                    changed |= transaction(ui.ctx(), path, text, |text, _| {
                        *text = candidate;
                        after
                    });
                }
            }
            for event in ui.input(|i| i.events.clone()) {
                if let egui::Event::Ime(event) = event {
                    match event {
                        ImeEvent::Preedit(s) => self.composing = !s.is_empty(),
                        ImeEvent::Commit(_) | ImeEvent::Disabled => self.composing = false,
                        _ => {}
                    }
                }
            }
            if let Some(command) = keyboard_command(ui.ctx(), self.composing) {
                changed |= transaction(ui.ctx(), path, text, |text, current| {
                    commands::apply(text, current, command)
                });
                self.reveal = Some(selection(ui.ctx(), path, text).head);
            }
        }
        self.find.refresh(text);
        let selected = selection(ui.ctx(), path, text).range();
        let start = byte_index(text, selected.start);
        let end = byte_index(text, selected.end);
        let active = if ui.ctx().memory(|m| m.has_focus(id(path))) {
            Some(
                text[..start].rfind('\n').map_or(0, |n| n + 1)
                    ..text[end..].find('\n').map_or(text.len(), |n| end + n + 1),
            )
        } else {
            Some(usize::MAX..usize::MAX)
        };
        let folds = widgets::prepare(ui, path, text, &active, self.find.open, size);
        let appearance = Appearance {
            font: font.clone(),
            size,
            foreground: ui.visuals().text_color(),
            code_background: ui.visuals().code_bg_color,
            accent,
            dark: ui.visuals().dark_mode,
            active,
            folded: folds
                .iter()
                .map(|fold| (fold.block.range.clone(), fold.height))
                .collect(),
        };
        let job = self.cache.get(text, &appearance, &self.find);
        let layout_text = text.clone();
        let mut layouter = |ui: &Ui, _buffer: &dyn egui::TextBuffer, wrap_width: f32| {
            // TextEdit invokes this again after typing, so never reuse a layout
            // whose text no longer matches the buffer.
            let mut job = if _buffer.as_str() == job.text {
                job.clone()
            } else {
                self.find.refresh(_buffer.as_str());
                let mut updated = appearance.clone();
                updated.folded = widgets::prepare(
                    ui,
                    path,
                    _buffer.as_str(),
                    &updated.active,
                    self.find.open,
                    size,
                )
                .into_iter()
                .map(|fold| (fold.block.range, fold.height))
                .collect();
                self.cache.get(_buffer.as_str(), &updated, &self.find)
            };
            job.wrap.max_width = wrap_width;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let output = TextEdit::multiline(text)
            .id(id(path))
            .font(FontId::new(size, egui::FontFamily::Name(font.into())))
            .frame(false)
            .margin(egui::Margin::ZERO)
            .desired_width(width)
            .desired_rows(20)
            .lock_focus(true)
            .cursor_at_end(false)
            .layouter(&mut layouter)
            .show(ui);
        changed |= output.response.changed();
        let explicit_anchor = self.anchor.take();
        if let Some((character, screen_y, frames)) = explicit_anchor {
            let y =
                output.galley_pos.y + output.galley.pos_from_cursor(CCursor::new(character)).top();
            // Cancel TextEdit's automatic caret-centering target before applying
            // an exact content delta; targeting the viewport itself needs no scroll.
            ui.scroll_to_rect_animation(ui.clip_rect(), None, egui::style::ScrollAnimation::none());
            ui.scroll_with_delta_animation(
                egui::vec2(0.0, screen_y - y),
                egui::style::ScrollAnimation::none(),
            );
            // Fold heights can settle on the following frame after measurement.
            if frames > 1 {
                self.anchor = Some((character, screen_y, frames - 1));
            }
        } else if self.reveal.is_none()
            && !changed
            && let Some((previous_path, previous)) = &self.last_layout
            && previous_path == path
            && previous.text() == text
        {
            // Measure the same character in both layouts. Comparing local
            // positions excludes ordinary wheel scrolling from compensation.
            let point = egui::vec2(
                0.0,
                (ui.clip_rect().top() - output.galley_pos.y + 24.0).max(0.0),
            );
            let character = previous.cursor_from_pos(point);
            let before = previous.pos_from_cursor(character).top();
            let after = output.galley.pos_from_cursor(character).top();
            if (before - after).abs() > 0.5 {
                ui.scroll_with_delta_animation(
                    egui::vec2(0.0, before - after),
                    egui::style::ScrollAnimation::none(),
                );
            }
        }
        self.last_layout = Some((path.to_owned(), output.galley.clone()));
        self.multiple.paint(ui, &output);
        if *text == layout_text {
            changed |= widgets::paint(self, ui, path, text, &output, &folds, size, accent, width);
            widgets::source_preview(self, ui, path, text, &output, &appearance.active, width);
        } else {
            ui.ctx().request_repaint();
        }
        if output.cursor_range.map(|range| range.primary.index) != Some(selected.end) {
            ui.ctx().request_repaint();
        }
        if explicit_anchor.is_none()
            && output.response.has_focus()
            && (output.response.changed()
                || output.response.clicked()
                || ui.input(|i| {
                    [
                        Key::ArrowUp,
                        Key::ArrowDown,
                        Key::ArrowLeft,
                        Key::ArrowRight,
                        Key::Home,
                        Key::End,
                        Key::PageUp,
                        Key::PageDown,
                    ]
                    .iter()
                    .any(|key| i.key_pressed(*key))
                }))
            && let Some(range) = output.cursor_range
        {
            let caret = output
                .galley
                .pos_from_cursor(range.primary)
                .translate(output.galley_pos.to_vec2());
            ui.scroll_to_rect(
                caret.expand2(egui::vec2(0.0, crate::edge_blur::HEIGHT + 12.0)),
                None,
            );
        }
        if let Some(character) = self.reveal.take() {
            let rect = output
                .galley
                .pos_from_cursor(CCursor::new(character))
                .translate(output.galley_pos.to_vec2());
            ui.scroll_to_rect(rect.expand(12.0), Some(Align::Center));
        }
        let mut command = None;
        let mut select_all = false;
        output.response.context_menu(|ui| {
            for (label, c) in [
                ("Bold · Ctrl+B", Command::Wrap("**")),
                ("Italic · Ctrl+I", Command::Wrap("*")),
                ("Link · Ctrl+K", Command::Link),
                ("Inline code · Ctrl+E", Command::Wrap("`")),
                ("Task list", Command::Task),
                ("Bullet list", Command::Bullet),
                ("Numbered list", Command::Numbered),
                ("Blockquote", Command::Quote),
            ] {
                if ui.button(label).clicked() {
                    command = Some(c);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Select all · Ctrl+A").clicked() {
                select_all = true;
                ui.close();
            }
        });
        if let Some(command) = command {
            changed |= transaction(ui.ctx(), path, text, |text, s| {
                commands::apply(text, s, command)
            });
            self.focus = true;
        }
        if select_all {
            self.select(
                ui.ctx(),
                path,
                Selection {
                    anchor: 0,
                    head: text.chars().count(),
                },
            );
            self.focus = true;
        }
        changed
    }
}

fn keyboard_command(ctx: &Context, composing: bool) -> Option<Command> {
    let ctrl = Modifiers::CTRL;
    let shift = ctrl | Modifiers::SHIFT;
    let alt = ctrl | Modifiers::ALT;
    let shortcuts = [
        (
            Modifiers::ALT,
            Key::ArrowUp,
            Command::Line(lines::Operation::Move(true)),
        ),
        (
            Modifiers::ALT,
            Key::ArrowDown,
            Command::Line(lines::Operation::Move(false)),
        ),
        (
            Modifiers::ALT | Modifiers::SHIFT,
            Key::ArrowUp,
            Command::Line(lines::Operation::Copy(true)),
        ),
        (
            Modifiers::ALT | Modifiers::SHIFT,
            Key::ArrowDown,
            Command::Line(lines::Operation::Copy(false)),
        ),
        (shift, Key::K, Command::Line(lines::Operation::Delete)),
        (ctrl, Key::Enter, Command::Line(lines::Operation::Insert)),
        (ctrl, Key::CloseBracket, Command::Indent),
        (ctrl, Key::OpenBracket, Command::Outdent),
        (ctrl, Key::B, Command::Wrap("**")),
        (ctrl, Key::I, Command::Wrap("*")),
        (ctrl, Key::K, Command::Link),
        (ctrl, Key::E, Command::Wrap("`")),
        (shift, Key::X, Command::Wrap("~~")),
        (shift, Key::Num8, Command::Bullet),
        (shift, Key::Num7, Command::Numbered),
        (shift, Key::Enter, Command::Task),
        (shift, Key::Period, Command::Quote),
        (alt, Key::Num1, Command::Heading(1)),
        (alt, Key::Num2, Command::Heading(2)),
        (alt, Key::Num3, Command::Heading(3)),
        (alt, Key::Num4, Command::Heading(4)),
        (alt, Key::Num5, Command::Heading(5)),
        (alt, Key::Num6, Command::Heading(6)),
        (alt, Key::Num0, Command::Heading(0)),
        (Modifiers::SHIFT, Key::Tab, Command::Outdent),
        (Modifiers::NONE, Key::Tab, Command::Indent),
    ];
    for (modifiers, key, command) in shortcuts {
        if ctx.input(|i| {
            i.modifiers.ctrl == modifiers.ctrl
                && i.modifiers.alt == modifiers.alt
                && i.modifiers.shift == modifiers.shift
        }) && ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)))
        {
            return Some(command);
        }
    }
    if !composing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        return Some(Command::Enter);
    }
    None
}

#[cfg(test)]
mod tests;
