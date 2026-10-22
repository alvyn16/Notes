use super::*;

#[derive(Default)]
pub(super) struct Multiple {
    ranges: Vec<Selection>,
}
impl Multiple {
    pub fn clear(&mut self) {
        self.ranges.clear();
    }
    pub fn input(&mut self, ctx: &Context, path: &Path, text: &mut String) -> bool {
        if ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab) || ((i.modifiers.ctrl || i.modifiers.command) && i.events.iter().any(|event| matches!(event, egui::Event::Key { key, pressed: true, .. } if !matches!(key, Key::D | Key::C | Key::V | Key::X | Key::Z | Key::Y))))) {
            self.clear();
        }
        if ctx.input(|i| {
            [
                Key::ArrowLeft,
                Key::ArrowRight,
                Key::ArrowUp,
                Key::ArrowDown,
                Key::Home,
                Key::End,
            ]
            .iter()
            .any(|key| i.key_pressed(*key))
        }) {
            self.clear();
        }
        if ctx.input(|i| {
            i.pointer.any_pressed()
                || i.key_pressed(Key::Escape)
                || i.events.iter().any(|e| matches!(e, egui::Event::Ime(_)))
        }) {
            self.clear();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::D)) {
            let mut selected = selection(ctx, path, text);
            if selected.range().is_empty() {
                let chars: Vec<_> = text.chars().collect();
                let mut start = selected.head;
                let mut end = start;
                while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                    start -= 1;
                }
                while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                    end += 1;
                }
                selected = Selection {
                    anchor: start,
                    head: end,
                };
                self.ranges = vec![selected];
            } else {
                if self.ranges.is_empty() {
                    self.ranges.push(selected);
                }
                let range = selected.range();
                let needle = &text[byte_index(text, range.start)..byte_index(text, range.end)];
                if !needle.is_empty() {
                    let matches = super::search::matches(text, needle, true, false);
                    let next = matches
                        .iter()
                        .filter(|m| m.start >= byte_index(text, range.end))
                        .chain(matches.iter())
                        .map(|r| Selection {
                            anchor: char_index(text, r.start),
                            head: char_index(text, r.end),
                        })
                        .find(|s| !self.ranges.contains(s));
                    if let Some(next) = next {
                        self.ranges.push(next);
                        selected = next;
                    }
                }
            }
            let mut state = TextEditState::load(ctx, id(path)).unwrap_or_default();
            state.cursor.set_char_range(Some(cursor(selected)));
            state.store(ctx, id(path));
            return false;
        }
        if self.ranges.len() < 2 {
            return false;
        }
        if ctx.input(|i| i.modifiers.command || i.modifiers.ctrl)
            && ctx.input(|i| i.key_pressed(Key::Z) || i.key_pressed(Key::Y))
        {
            self.clear();
            return false;
        }
        let events = ctx.input(|i| i.events.clone());
        let copy = events
            .iter()
            .any(|e| matches!(e, egui::Event::Copy | egui::Event::Cut));
        if copy {
            let mut ranges = self.ranges.clone();
            ranges.sort_by_key(|s| s.range().start);
            ctx.copy_text(
                ranges
                    .iter()
                    .map(|s| {
                        &text[byte_index(text, s.range().start)..byte_index(text, s.range().end)]
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        let mut inserted = String::new();
        let mut edit = false;
        let mut backspace = false;
        let mut delete = false;
        for event in &events {
            match event {
                egui::Event::Text(s) | egui::Event::Paste(s) => {
                    inserted.push_str(s);
                    edit = true;
                }
                egui::Event::Cut => edit = true,
                egui::Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.is_none() => {
                    edit = true;
                    backspace = true;
                }
                egui::Event::Key {
                    key: Key::Delete,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.is_none() => {
                    edit = true;
                    delete = true;
                }
                _ => {}
            }
        }
        if copy || edit {
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Copy
                            | egui::Event::Cut
                            | egui::Event::Key {
                                key: Key::Backspace | Key::Delete,
                                ..
                            }
                    )
                })
            });
        }
        if !edit {
            return false;
        }
        let mut ranges = self.ranges.clone();
        for s in &mut ranges {
            if s.range().is_empty() {
                if backspace {
                    s.anchor = s.head.saturating_sub(1);
                }
                if delete {
                    s.head = (s.head + 1).min(text.chars().count());
                }
            }
        }
        ranges.sort_by_key(|s| s.range().start);
        ranges.dedup();
        let mut shift = 0_isize;
        self.ranges = ranges
            .iter()
            .map(|s| {
                let r = s.range();
                let at = r.start.saturating_add_signed(shift) + inserted.chars().count();
                shift += inserted.chars().count() as isize - r.len() as isize;
                Selection::caret(at)
            })
            .collect();
        let primary = *self.ranges.last().unwrap();
        transaction(ctx, path, text, |text, _| {
            for s in ranges.into_iter().rev() {
                commands::replace(text, s, &inserted);
            }
            primary
        })
    }
    pub fn paint(&self, ui: &Ui, output: &egui::text_edit::TextEditOutput) {
        for s in self.ranges.iter().take(self.ranges.len().saturating_sub(1)) {
            let first = output.galley.pos_from_cursor(CCursor::new(s.range().start));
            let last = output.galley.pos_from_cursor(CCursor::new(s.range().end));
            let mut rect = egui::Rect::from_min_max(first.min, last.max)
                .translate(output.galley_pos.to_vec2());
            if s.range().is_empty() {
                rect.max.x = rect.min.x + 1.5;
            }
            ui.painter()
                .rect_filled(rect, 0, ui.visuals().selection.bg_fill.linear_multiply(0.5));
        }
    }
}
