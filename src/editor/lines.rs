use super::commands::{Selection, byte_index, char_index};

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Move(bool),
    Copy(bool),
    Delete,
    Insert,
}
pub fn apply(text: &mut String, selection: Selection, operation: Operation) -> Selection {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with('\n');
    let mut lines: Vec<String> = text
        .split_terminator('\n')
        .map(|s| s.trim_end_matches('\r').to_owned())
        .collect();
    if lines.is_empty() || trailing {
        lines.push(String::new());
    }
    let range = selection.range();
    let start = byte_index(text, range.start);
    let end = byte_index(text, range.end);
    let first = text[..start].bytes().filter(|c| *c == b'\n').count();
    let last = text[..end.saturating_sub(usize::from(
        end > start && text.as_bytes().get(end - 1) == Some(&b'\n'),
    ))]
        .bytes()
        .filter(|c| *c == b'\n')
        .count();
    let anchor_column = text[..byte_index(text, selection.anchor)]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .chars()
        .count();
    let head_column = text[..byte_index(text, selection.head)]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .chars()
        .count();
    let count = last - first + 1;
    let target = match operation {
        Operation::Move(true) if first > 0 => {
            lines[first - 1..=last].rotate_left(1);
            first - 1
        }
        Operation::Move(false) if last + 1 < lines.len() => {
            lines[first..=last + 1].rotate_right(1);
            first + 1
        }
        Operation::Copy(up) => {
            let at = if up { first } else { last + 1 };
            let copy = lines[first..=last].to_vec();
            lines.splice(at..at, copy);
            at
        }
        Operation::Delete => {
            lines.drain(first..=last);
            if lines.is_empty() {
                lines.push(String::new());
            }
            first.min(lines.len() - 1)
        }
        Operation::Insert => {
            lines.insert(last + 1, String::new());
            last + 1
        }
        _ => return selection,
    };
    *text = lines.join(newline);
    let position = |row: usize, column: usize| {
        let row = row.min(lines.len() - 1);
        lines[..row]
            .iter()
            .map(|s| s.chars().count() + newline.chars().count())
            .sum::<usize>()
            + column.min(lines[row].chars().count())
    };
    if matches!(operation, Operation::Delete | Operation::Insert) {
        return Selection::caret(position(target, 0));
    }
    let start_pos = position(
        target,
        if selection.anchor <= selection.head {
            anchor_column
        } else {
            head_column
        },
    );
    let end_pos = position(
        target + count - 1,
        if selection.anchor <= selection.head {
            head_column
        } else {
            anchor_column
        },
    );
    if selection.anchor <= selection.head {
        Selection {
            anchor: start_pos,
            head: end_pos,
        }
    } else {
        Selection {
            anchor: end_pos,
            head: start_pos,
        }
    }
}

pub fn backspace_list(text: &mut String, selection: Selection) -> Option<Selection> {
    if !selection.range().is_empty() {
        return None;
    }
    let caret = byte_index(text, selection.head);
    let start = text[..caret].rfind('\n').map_or(0, |n| n + 1);
    let line = &text[start..caret];
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let body = &line[indent..];
    let marker =
        regex::Regex::new(r"^(?:[-+*] (?:\[[ xX]\] )?|\d+[.)] |> )$").expect("list prefix");
    if !marker.is_match(body) {
        return None;
    }
    if indent > 0 {
        let remove = indent - parent_indent(text, start, indent);
        text.replace_range(start..start + remove, "");
        Some(Selection::caret(selection.head - remove))
    } else {
        text.replace_range(start..caret, "");
        Some(Selection::caret(char_index(text, start)))
    }
}

pub(super) fn parent_indent(text: &str, start: usize, indent: usize) -> usize {
    let marker = regex::Regex::new(r"^(?:[-+*] |\d+[.)] |> )").expect("list marker");
    text[..start]
        .lines()
        .rev()
        .find_map(|line| {
            let depth = line.len() - line.trim_start_matches([' ', '\t']).len();
            (depth < indent && marker.is_match(line.trim_start())).then_some(depth)
        })
        .unwrap_or(indent.saturating_sub(2))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_operations_preserve_unicode_crlf_and_selection() {
        let mut text = "first\r\ncafé\r\nlast".to_owned();
        let s = Selection::caret(9);
        let s = apply(&mut text, s, Operation::Move(true));
        assert_eq!(text, "café\r\nfirst\r\nlast");
        assert_eq!(s.head, 2);
        let s = apply(&mut text, s, Operation::Copy(false));
        assert_eq!(text, "café\r\ncafé\r\nfirst\r\nlast");
        apply(&mut text, s, Operation::Delete);
        assert_eq!(text, "café\r\nfirst\r\nlast");
    }
}
