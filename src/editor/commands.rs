use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}
impl Selection {
    pub fn caret(position: usize) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
    pub fn clamp(self, text: &str) -> Self {
        let len = text.chars().count();
        Self {
            anchor: self.anchor.min(len),
            head: self.head.min(len),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Wrap(&'static str),
    Link,
    Heading(usize),
    Bullet,
    Numbered,
    Task,
    Quote,
    Enter,
    Indent,
    Outdent,
    Line(super::lines::Operation),
}

pub fn byte_index(text: &str, character: usize) -> usize {
    text.char_indices()
        .nth(character)
        .map_or(text.len(), |(index, _)| index)
}
pub fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

pub fn replace(text: &mut String, selection: Selection, replacement: &str) -> Selection {
    let range = selection.clamp(text).range();
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end);
    text.replace_range(start..end, replacement);
    Selection::caret(range.start + replacement.chars().count())
}

pub fn apply(text: &mut String, selection: Selection, command: Command) -> Selection {
    let selection = selection.clamp(text);
    let range = selection.range();
    match command {
        Command::Line(operation) => super::lines::apply(text, selection, operation),
        Command::Wrap(marker) => {
            let start = byte_index(text, range.start);
            let end = byte_index(text, range.end);
            let selected = &text[start..end];
            let count = marker.chars().count();
            if selected.len() >= marker.len() * 2
                && selected.starts_with(marker)
                && selected.ends_with(marker)
            {
                let inner = selected[marker.len()..selected.len() - marker.len()].to_owned();
                text.replace_range(start..end, &inner);
                Selection {
                    anchor: range.start,
                    head: range.end - 2 * count,
                }
            } else if text[..start].ends_with(marker) && text[end..].starts_with(marker) {
                text.replace_range(end..end + marker.len(), "");
                text.replace_range(start - marker.len()..start, "");
                Selection {
                    anchor: range.start - count,
                    head: range.end - count,
                }
            } else {
                let replacement = format!("{marker}{selected}{marker}");
                text.replace_range(start..end, &replacement);
                Selection {
                    anchor: range.start + count,
                    head: range.end + count,
                }
            }
        }
        Command::Link => {
            let start = byte_index(text, range.start);
            let end = byte_index(text, range.end);
            let label = if range.is_empty() {
                "link text"
            } else {
                &text[start..end]
            };
            let replacement = format!("[{label}](https://)");
            let url = range.start + label.chars().count() + 3;
            text.replace_range(start..end, &replacement);
            Selection {
                anchor: url,
                head: url + 8,
            }
        }
        Command::Enter => enter(text, selection),
        Command::Indent | Command::Outdent => {
            let start = byte_index(text, selection.range().start);
            let first = text[..start].rfind('\n').map_or(0, |n| n + 1);
            let mut numbers = std::collections::BTreeMap::<usize, usize>::new();
            for line in text[..first].lines() {
                let depth = line.len() - line.trim_start_matches([' ', '\t']).len();
                if let (_, Some(n), _) = prefix(line.trim_start()) {
                    numbers.insert(depth, n);
                } else if !line.trim().is_empty() && prefix(line.trim_start()).0.is_empty() {
                    numbers.clear();
                }
            }
            transform_lines(text, selection, |line| {
                let mut line = if matches!(command, Command::Indent) {
                    format!("  {line}")
                } else if let Some(rest) = line.strip_prefix('\t') {
                    rest.to_owned()
                } else {
                    let n = line.bytes().take(2).take_while(|c| *c == b' ').count();
                    line[n..].to_owned()
                };
                let depth = line.len() - line.trim_start_matches([' ', '\t']).len();
                let (marker, number, _) = prefix(&line[depth..]);
                if number.is_some() {
                    let next = numbers.get(&depth).map_or(1, |n| n + 1);
                    let delimiter = if marker.contains(')') { ')' } else { '.' };
                    let end = depth + marker.len();
                    line.replace_range(depth..end, &format!("{next}{delimiter} "));
                    numbers.retain(|old, _| *old <= depth);
                    numbers.insert(depth, next);
                }
                line
            })
        }
        Command::Heading(level) => transform_lines(text, selection, |line| {
            let n = line.bytes().take_while(|c| *c == b'#').count();
            let body = if (1..=6).contains(&n) && line.as_bytes().get(n) == Some(&b' ') {
                &line[n + 1..]
            } else {
                line
            };
            if level == 0 || n == level {
                body.to_owned()
            } else {
                format!("{} {body}", "#".repeat(level))
            }
        }),
        Command::Bullet | Command::Numbered | Command::Task | Command::Quote => {
            let mut number = 0;
            transform_lines(text, selection, |line| {
                number += 1;
                let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
                let body = &line[indent..];
                let (old_prefix, _, _) = prefix(body);
                let marker = match command {
                    Command::Bullet => "- ".into(),
                    Command::Numbered => format!("{number}. "),
                    Command::Task => "- [ ] ".into(),
                    _ => "> ".into(),
                };
                let matching = match command {
                    Command::Task => body.starts_with("- ["),
                    Command::Bullet => body.starts_with("- ") && !body.starts_with("- ["),
                    Command::Numbered => {
                        body.chars().next().is_some_and(|c| c.is_ascii_digit())
                            && !old_prefix.is_empty()
                    }
                    _ => body.starts_with("> "),
                };
                format!(
                    "{}{}{}",
                    &line[..indent],
                    if matching { "" } else { &marker },
                    &body[old_prefix.len()..]
                )
            })
        }
    }
}

fn transform_lines(
    text: &mut String,
    selection: Selection,
    mut transform: impl FnMut(&str) -> String,
) -> Selection {
    let range = selection.range();
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end);
    let first = text[..start].rfind('\n').map_or(0, |n| n + 1);
    // A selection ending at the start of a line must not indent that next line.
    let last = if end > first && text.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\n') {
        end - 1
    } else {
        end
    };
    let last = text[last..].find('\n').map_or(text.len(), |n| last + n);
    let original = &text[first..last];
    let mut replacement = String::new();
    let mut mapped_anchor = selection.anchor;
    let mut mapped_head = selection.head;
    let base = char_index(text, first);
    let mut old_offset = base;
    let mut new_offset = base;
    for (index, line) in original.split('\n').enumerate() {
        if index > 0 {
            replacement.push('\n');
        }
        let (body, cr) = line.strip_suffix('\r').map_or((line, ""), |s| (s, "\r"));
        let transformed = transform(body) + cr;
        let old_len = line.chars().count();
        let new_len = transformed.chars().count();
        // These commands change prefixes, so retain the caret's distance from the line end.
        for (position, mapped) in [
            (selection.anchor, &mut mapped_anchor),
            (selection.head, &mut mapped_head),
        ] {
            if position >= old_offset && position <= old_offset + old_len {
                *mapped = new_offset
                    + (position - old_offset)
                        .saturating_add_signed(new_len as isize - old_len as isize)
                        .min(new_len);
            }
        }
        old_offset += old_len + 1;
        new_offset += new_len + 1;
        replacement.push_str(&transformed);
    }
    let delta = replacement.chars().count() as isize - original.chars().count() as isize;
    if selection.anchor > old_offset - 1 {
        mapped_anchor = selection.anchor.saturating_add_signed(delta);
    }
    if selection.head > old_offset - 1 {
        mapped_head = selection.head.saturating_add_signed(delta);
    }
    text.replace_range(first..last, &replacement);
    Selection {
        anchor: mapped_anchor,
        head: mapped_head,
    }
}

fn prefix(body: &str) -> (&str, Option<usize>, bool) {
    if body.starts_with("- [ ] ") || body.starts_with("- [x] ") || body.starts_with("- [X] ") {
        return (&body[..6], None, true);
    }
    if body.starts_with("- ")
        || body.starts_with("* ")
        || body.starts_with("+ ")
        || body.starts_with("> ")
    {
        return (&body[..2], None, false);
    }
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && (body[digits..].starts_with(". ") || body[digits..].starts_with(") ")) {
        return (&body[..digits + 2], body[..digits].parse().ok(), false);
    }
    ("", None, false)
}

fn enter(text: &mut String, selection: Selection) -> Selection {
    let range = selection.range();
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end);
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if text[start..end].contains('\n') {
        return replace(text, selection, newline);
    }
    let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
    let before = &text[line_start..start];
    let indentation = before.len() - before.trim_start_matches([' ', '\t']).len();
    let indent = &before[..indentation];
    let in_code = pulldown_cmark::Parser::new(text)
        .into_offset_iter()
        .any(|(event, range)| {
            matches!(
                event,
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(_))
            ) && range.start <= start
                && start <= range.end
        });
    if in_code {
        return replace(text, selection, &format!("{newline}{indent}"));
    }
    let body = &before[indentation..];
    let (marker, number, task) = prefix(body);
    let line_end = text[end..].find('\n').map_or(text.len(), |n| end + n);
    if !marker.is_empty()
        && body[marker.len()..].trim().is_empty()
        && text[end..line_end].trim().is_empty()
    {
        let replacement = if indent.is_empty() {
            String::new()
        } else if let Some(rest) = indent.strip_suffix('\t') {
            format!("{rest}{marker}")
        } else {
            let parent = super::lines::parent_indent(text, line_start, indent.len());
            format!("{}{marker}", &indent[..parent])
        };
        text.replace_range(line_start..end, &replacement);
        return Selection::caret(char_index(text, line_start) + replacement.chars().count());
    }
    let continued = if task {
        "- [ ] ".to_owned()
    } else if let Some(number) = number {
        format!(
            "{}{} ",
            number.saturating_add(1),
            if marker.contains(')') { ')' } else { '.' }
        )
    } else {
        marker.to_owned()
    };
    replace(text, selection, &format!("{newline}{indent}{continued}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_wrap_toggle_and_reverse_selection() {
        let mut text = "Hello café 🦀".to_owned();
        let selection = Selection {
            anchor: 12,
            head: 6,
        };
        let s = apply(&mut text, selection, Command::Wrap("**"));
        assert_eq!(text, "Hello **café 🦀**");
        let s = apply(&mut text, s, Command::Wrap("**"));
        assert_eq!(text, "Hello café 🦀");
        assert_eq!(s.range(), 6..12);
    }
    #[test]
    fn links_select_the_destination() {
        let mut text = "café".into();
        let s = apply(&mut text, Selection { anchor: 0, head: 4 }, Command::Link);
        assert_eq!(text, "[café](https://)");
        assert_eq!(
            &text[byte_index(&text, s.anchor)..byte_index(&text, s.head)],
            "https://"
        );
    }
    #[test]
    fn enter_continues_and_exits_lists() {
        for (before, after) in [
            ("- work", "- work\n- "),
            ("12. work", "12. work\n13. "),
            ("- [x] done", "- [x] done\n- [ ] "),
            ("> quote", "> quote\n> "),
            ("- ", ""),
            ("  - ", "- "),
        ] {
            let mut text = before.to_owned();
            let caret = Selection::caret(text.chars().count());
            apply(&mut text, caret, Command::Enter);
            assert_eq!(text, after);
        }
    }
    #[test]
    fn multiline_indent_preserves_selection_and_crlf() {
        let mut text = "café\r\nsecond\r\nthird".into();
        let s = Selection {
            anchor: 0,
            head: 14,
        };
        let s = apply(&mut text, s, Command::Indent);
        assert_eq!(text, "  café\r\n  second\r\nthird");
        apply(&mut text, s, Command::Outdent);
        assert_eq!(text, "café\r\nsecond\r\nthird");
    }
    #[test]
    fn enter_in_code_does_not_continue_a_markdown_list() {
        let mut text = "```text\n- sample\n```".to_owned();
        let position = text.find("sample").unwrap() + "sample".len();
        let position = char_index(&text, position);
        apply(&mut text, Selection::caret(position), Command::Enter);
        assert_eq!(text, "```text\n- sample\n\n```");
    }
    #[test]
    fn enter_at_start_of_list_body_does_not_delete_its_marker() {
        let mut text = "- remaining".to_owned();
        apply(&mut text, Selection::caret(2), Command::Enter);
        assert_eq!(text, "- \n- remaining");
    }
}
