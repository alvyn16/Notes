use super::search::Find;
use eframe::egui::{Color32, FontFamily, FontId, Stroke, TextFormat, text::LayoutJob};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

#[derive(Clone)]
pub struct Appearance {
    pub font: String,
    pub size: f32,
    pub foreground: Color32,
    pub code_background: Color32,
    pub accent: Color32,
    pub dark: bool,
    pub active: Option<Range<usize>>,
    pub folded: Vec<(Range<usize>, f32)>,
}

#[derive(Default)]
pub struct Cache {
    key: Option<u64>,
    job: LayoutJob,
}
impl Cache {
    pub fn get(&mut self, text: &str, appearance: &Appearance, find: &Find) -> LayoutJob {
        let mut key = DefaultHasher::new();
        text.hash(&mut key);
        appearance.font.hash(&mut key);
        appearance.size.to_bits().hash(&mut key);
        appearance.foreground.hash(&mut key);
        appearance.code_background.hash(&mut key);
        appearance.accent.hash(&mut key);
        appearance.dark.hash(&mut key);
        appearance.active.hash(&mut key);
        for (range, height) in &appearance.folded {
            range.hash(&mut key);
            height.to_bits().hash(&mut key);
        }
        find.open.hash(&mut key);
        find.query.hash(&mut key);
        find.match_case.hash(&mut key);
        find.whole_word.hash(&mut key);
        find.regex.hash(&mut key);
        find.current.hash(&mut key);
        let key = key.finish();
        if self.key != Some(key) {
            self.key = Some(key);
            self.job = highlight(text, appearance, find);
        }
        self.job.clone()
    }
}

fn highlight(text: &str, a: &Appearance, find: &Find) -> LayoutJob {
    let normal = TextFormat {
        font_id: FontId::new(a.size, FontFamily::Name(a.font.clone().into())),
        color: a.foreground,
        line_height: Some(a.size * 1.55),
        ..Default::default()
    };
    let muted = TextFormat {
        color: if a.dark {
            Color32::from_gray(120)
        } else {
            Color32::from_gray(140)
        },
        ..normal.clone()
    };
    let mut job = LayoutJob::default();
    let mut offset = 0;
    let mut heading = 0;
    let mut bold = false;
    let mut italic = false;
    let mut link = false;
    let mut code = false;
    let mut strike = false;
    for (event, range) in Parser::new_ext(
        text,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = level as usize,
            Event::End(TagEnd::Heading(_)) => heading = 0,
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Link { .. }) => link = true,
            Event::End(TagEnd::Link) => link = false,
            Event::Start(Tag::CodeBlock(_)) => code = true,
            Event::End(TagEnd::CodeBlock) => code = false,
            Event::Start(Tag::Strikethrough) => strike = true,
            Event::End(TagEnd::Strikethrough) => strike = false,
            Event::Text(_) | Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => {
                let start = range.start.max(offset);
                if start >= range.end {
                    continue;
                }
                if offset < start {
                    markup(&mut job, text, offset..start, &muted, find, a);
                }
                let mut format = normal.clone();
                if heading > 0 {
                    let scale = match heading {
                        1 => 1.65,
                        2 => 1.38,
                        _ => 1.12,
                    };
                    format.font_id =
                        FontId::new(a.size * scale, crate::theme::bold(&normal.font_id.family));
                    format.line_height = Some(a.size * scale * 1.4);
                } else if bold {
                    format.font_id.family = crate::theme::bold(&normal.font_id.family);
                }
                if code || matches!(event, Event::Code(_)) {
                    format.font_id = FontId::monospace(a.size * 0.9);
                    format.background = a.code_background;
                }
                format.italics = italic;
                if link {
                    format.color = a.accent;
                    format.underline = Stroke::new(1.0_f32, a.accent);
                }
                if strike {
                    format.strikethrough = Stroke::new(1.0_f32, a.foreground);
                }
                append(&mut job, text, start..range.end, &format, find, a);
                offset = range.end;
            }
            _ => {}
        }
    }
    if offset < text.len() {
        markup(&mut job, text, offset..text.len(), &muted, find, a);
    }
    fold_blocks(&mut job, text, &a.folded);
    job
}

fn fold_blocks(job: &mut LayoutJob, text: &str, folded: &[(Range<usize>, f32)]) {
    if folded.is_empty() {
        return;
    }
    let mut sections = vec![];
    for section in &job.sections {
        let mut boundaries = vec![section.byte_range.start, section.byte_range.end];
        for (range, _) in folded {
            for point in [range.start, range.end] {
                if section.byte_range.contains(&point) {
                    boundaries.push(point);
                }
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        for pair in boundaries.windows(2) {
            let range = pair[0]..pair[1];
            let mut piece = section.clone();
            piece.byte_range = range.clone();
            if let Some((block, height)) = folded.iter().find(|(b, _)| b.contains(&range.start)) {
                let lines = text[block.clone()]
                    .chars()
                    .filter(|c| *c == '\n')
                    .count()
                    .max(1);
                piece.format.font_id.size = 0.01;
                piece.format.color = Color32::TRANSPARENT;
                piece.format.background = Color32::TRANSPARENT;
                piece.format.underline = Stroke::NONE;
                piece.format.strikethrough = Stroke::NONE;
                piece.format.line_height = Some(height / lines as f32);
            }
            sections.push(piece);
        }
    }
    job.sections = sections;
}

fn markup(
    job: &mut LayoutJob,
    text: &str,
    range: Range<usize>,
    format: &TextFormat,
    find: &Find,
    a: &Appearance,
) {
    if a.active.is_none() || find.open {
        append(job, text, range, format, find, a);
        return;
    }
    let active = a.active.as_ref().unwrap();
    for (offset, ch) in text[range.clone()].char_indices() {
        let start = range.start + offset;
        let mut f = format.clone();
        if !active.contains(&start) && !ch.is_whitespace() {
            // Keep all source characters in the galley so selections, copy and
            // undo retain exact document offsets while markup collapses.
            f.font_id.size = 0.01;
            f.color = Color32::TRANSPARENT;
        }
        job.append(&text[start..start + ch.len_utf8()], 0.0, f);
    }
}

fn append(
    job: &mut LayoutJob,
    text: &str,
    range: Range<usize>,
    format: &TextFormat,
    find: &Find,
    a: &Appearance,
) {
    if !find.open || find.matches.is_empty() {
        job.append(&text[range], 0.0, format.clone());
        return;
    }
    let mut offset = range.start;
    let first = find.matches.partition_point(|m| m.end <= range.start);
    for (index, matched) in find
        .matches
        .iter()
        .enumerate()
        .skip(first)
        .take_while(|(_, m)| m.start < range.end)
    {
        let start = matched.start.max(range.start);
        let end = matched.end.min(range.end);
        if offset < start {
            job.append(&text[offset..start], 0.0, format.clone());
        }
        let mut highlighted = format.clone();
        highlighted.background = if find.current == Some(index) {
            a.accent
        } else if a.dark {
            Color32::from_rgb(99, 75, 20)
        } else {
            Color32::from_rgb(255, 229, 145)
        };
        highlighted.color = if find.current == Some(index) || !a.dark {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
        job.append(&text[start..end], 0.0, highlighted);
        offset = end;
    }
    if offset < range.end {
        job.append(&text[offset..range.end], 0.0, format.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_markup_folds_outside_active_line_without_changing_source_offsets() {
        let text = "# Heading\n\n**bold** and [link](target.md)\n";
        let a = Appearance {
            font: "Segoe UI".into(),
            size: 16.0,
            foreground: Color32::WHITE,
            code_background: Color32::from_gray(25),
            accent: Color32::RED,
            dark: true,
            active: Some(0..10),
            folded: vec![],
        };
        let job = highlight(text, &a, &Find::default());
        assert_eq!(job.text, text);
        assert!(
            job.sections
                .iter()
                .any(|s| s.byte_range.start == 0 && s.format.font_id.size > 1.0)
        );
        assert!(
            job.sections
                .iter()
                .any(|s| s.format.font_id.size < 1.0 && &text[s.byte_range.clone()] == "*")
        );
        let mut find = Find::default();
        find.open = true;
        let job = highlight(text, &a, &find);
        assert!(job.sections.iter().all(|s| s.format.font_id.size > 1.0));
    }
    #[test]
    fn highlighting_preserves_every_character_and_highlights_unicode() {
        let text = "# Café\n\n**café** [link](https://example.com)\n\n```rust\nlet x=1;\n```\n";
        let mut find = Find::default();
        find.open = true;
        find.query = "café".into();
        find.refresh(text);
        let a = Appearance {
            font: "Segoe UI".into(),
            size: 16.0,
            foreground: Color32::WHITE,
            code_background: Color32::from_gray(25),
            accent: Color32::from_rgb(255, 100, 0),
            dark: true,
            active: None,
            folded: vec![],
        };
        let job = highlight(text, &a, &find);
        assert_eq!(job.text, text);
        for section in &job.sections {
            assert!(text.is_char_boundary(section.byte_range.start));
            assert!(text.is_char_boundary(section.byte_range.end));
        }
        assert!(job.sections.iter().any(|s| s.format.background == a.accent));
    }
}
