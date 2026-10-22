mod math_delimiters;
mod prose;
use prose::inline;
mod structured;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, Ui, vec2};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;
pub fn is_mermaid(language: &str) -> bool {
    language.split_whitespace().next().is_some_and(|l| {
        l.trim_matches(['{', '}', '.'])
            .eq_ignore_ascii_case("mermaid")
    })
}
/// Preserve whole-document reference definitions when individual rich blocks
/// are parsed in isolation. Store only definitions, never executable markup.
pub fn prepare_document(ctx: &egui::Context, path: &std::path::Path, text: &str) {
    let key = egui::util::hash(text);
    let id = egui::Id::new(("markdown-references", path));
    if ctx
        .data(|d| d.get_temp::<(u64, std::sync::Arc<String>)>(id))
        .is_some_and(|(cached, _)| cached == key)
    {
        return;
    }
    let parser = Parser::new(text);
    let definitions = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| text[definition.span.clone()].to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    ctx.data_mut(|d| d.insert_temp(id, (key, std::sync::Arc::new(definitions))));
}
fn code_content(source: &str) -> String {
    let mut inside = false;
    Parser::new(source)
        .filter_map(|event| match event {
            Event::Start(Tag::CodeBlock(_)) => {
                inside = true;
                None
            }
            Event::End(TagEnd::CodeBlock) => {
                inside = false;
                None
            }
            Event::Text(text) if inside => Some(text.into_string()),
            _ => None,
        })
        .collect()
}
fn heading_content(source: &str) -> String {
    Parser::new(source)
        .filter_map(|event| match event {
            Event::Text(t) | Event::Code(t) => Some(t.into_string()),
            Event::SoftBreak | Event::HardBreak => Some(" ".into()),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Block {
    pub range: Range<usize>,
    pub kind: Kind,
}
pub fn cached_blocks(
    ctx: &egui::Context,
    path: &std::path::Path,
    text: &str,
) -> std::sync::Arc<Vec<Block>> {
    let key = egui::util::hash(text);
    let id = egui::Id::new(("markdown-blocks", path));
    if let Some((_, blocks)) = ctx
        .data(|d| d.get_temp::<(u64, std::sync::Arc<Vec<Block>>)>(id))
        .filter(|(cached, _)| *cached == key)
    {
        return blocks;
    }
    let blocks = std::sync::Arc::new(blocks(text));
    ctx.data_mut(|d| d.insert_temp(id, (key, blocks.clone())));
    blocks
}
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Heading(usize),
    Paragraph,
    Task(bool),
    List,
    Quote,
    Code(String),
    Table,
    Rule,
}

pub fn blocks(text: &str) -> Vec<Block> {
    structured::parse(text)
        .into_iter()
        .map(|mut node| {
            if matches!(
                node.event,
                Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Indented))
            ) {
                node.range.start = text[..node.range.start].rfind('\n').map_or(0, |at| at + 1);
            }
            let source = &text[node.range.clone()];
            let trimmed = source.trim_end_matches(['\r', '\n']);
            let tail = &source[trimmed.len()..];
            node.range.end = node.range.start
                + trimmed.len()
                + if tail.starts_with("\r\n") {
                    2
                } else if tail.starts_with('\n') {
                    1
                } else {
                    0
                };
            let kind = match node.event {
                Event::Start(Tag::Heading { level, .. }) => Kind::Heading(level as usize),
                Event::Start(Tag::CodeBlock(kind)) => Kind::Code(match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.into_string(),
                    _ => String::new(),
                }),
                Event::Start(Tag::List(_)) => {
                    let source = text[node.range.clone()].trim();
                    if !source.contains('\n') && source.starts_with("- [") {
                        Kind::Task(source.starts_with("- [x]") || source.starts_with("- [X]"))
                    } else {
                        Kind::List
                    }
                }
                Event::Start(Tag::BlockQuote(_)) => Kind::Quote,
                Event::Start(Tag::Table(_)) => Kind::Table,
                Event::Rule => Kind::Rule,
                _ => Kind::Paragraph,
            };
            Block {
                range: node.range,
                kind,
            }
        })
        .collect()
}

#[derive(Default)]
pub struct Action {
    pub edit: bool,
    pub toggle: bool,
    pub toggles: Vec<usize>,
    pub link: Option<String>,
}

pub fn render(
    ui: &mut Ui,
    source: &str,
    kind: &Kind,
    size: f32,
    accent: Color32,
    location: (&std::path::Path, &std::path::Path),
) -> Action {
    let mut action = Action::default();
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            match kind {
                Kind::Heading(level) => {
                    let text = heading_content(source);
                    ui.add_space(if *level == 1 { 0.0 } else { 13.0 });
                    let font = if *level == 1 {
                        size * 1.65
                    } else if *level == 2 {
                        size * 1.38
                    } else {
                        size * 1.1
                    };
                    let display = if *level == 1 {
                        format!("   {text}")
                    } else {
                        text.clone()
                    };
                    let r = ui.add(
                        egui::Label::new(
                            RichText::new(display).font(FontId::new(
                                font,
                                crate::theme::bold(
                                    &ui.style()
                                        .override_font_id
                                        .as_ref()
                                        .map(|f| f.family.clone())
                                        .unwrap_or(egui::FontFamily::Proportional),
                                ),
                            )),
                        )
                        .selectable(true)
                        .sense(Sense::click()),
                    );
                    if *level == 1 {
                        ui.painter().text(
                            r.rect.left_top(),
                            egui::Align2::LEFT_TOP,
                            "#",
                            FontId::proportional(font),
                            Color32::from_gray(115),
                        );
                    }
                    action.edit = r.clicked();
                    ui.add_space(if *level == 1 { 9.0 } else { 18.0 });
                }
                Kind::Task(checked) => {
                    ui.horizontal_top(|ui| {
                        let mut value = *checked;
                        let response = ui.add_sized(
                            [25.0, size * 1.55],
                            egui::Checkbox::without_text(&mut value),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                true,
                                *checked,
                                source.trim_end().get(6..).unwrap_or_default(),
                            )
                        });
                        let r =
                            egui::Rect::from_center_size(response.rect.center(), vec2(17.0, 17.0));
                        ui.painter().rect_filled(
                            r,
                            4,
                            if *checked {
                                accent
                            } else {
                                ui.visuals().code_bg_color
                            },
                        );
                        if *checked {
                            ui.painter().add(egui::Shape::line(
                                vec![
                                    r.left_center() + vec2(4.0, 0.0),
                                    r.center() + vec2(-1.0, 3.0),
                                    r.right_top() + vec2(-4.0, 5.0),
                                ],
                                Stroke::new(2.2_f32, Color32::WHITE),
                            ));
                        } else {
                            ui.painter().rect_stroke(
                                r,
                                4,
                                Stroke::new(1.4_f32, Color32::from_gray(135)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.changed() {
                            action.toggle = true;
                        }
                        inline(
                            ui,
                            source.trim_end().get(6..).unwrap_or_default(),
                            size,
                            accent,
                            &mut action,
                            location,
                        );
                    });
                }
                Kind::List => structured::list(ui, source, size, accent, &mut action, location),
                Kind::Quote => {
                    let quoted = source
                        .lines()
                        .map(|line| {
                            line.trim_start()
                                .strip_prefix('>')
                                .unwrap_or(line)
                                .trim_start()
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(12, 5))
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .show(ui, |ui| {
                            inline(ui, &quoted, size, accent, &mut action, location);
                        });
                    ui.add_space(12.0);
                }
                Kind::Code(language) => {
                    let content = code_content(source);
                    if is_mermaid(language) {
                        action.edit = crate::diagrams::show(ui, &content);
                    } else {
                        egui::Frame::new()
                            .fill(ui.visuals().code_bg_color)
                            .corner_radius(5)
                            .inner_margin(12)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(RichText::new(language).small().color(Color32::GRAY));
                                egui::ScrollArea::horizontal()
                                    .id_salt("code-overflow")
                                    .show(ui, |ui| {
                                        let r = ui.add(
                                            egui::Label::new(
                                                RichText::new(content)
                                                    .font(FontId::monospace(size * 0.88)),
                                            )
                                            .wrap_mode(egui::TextWrapMode::Extend)
                                            .selectable(true)
                                            .sense(Sense::click()),
                                        );
                                        action.edit = r.clicked();
                                    });
                            });
                    }
                    ui.add_space(16.0);
                }
                Kind::Table => {
                    structured::table(ui, source, size, accent, &mut action, location);
                    ui.add_space(12.0);
                }
                Kind::Rule => {
                    ui.separator();
                }
                Kind::Paragraph => {
                    inline(ui, source.trim_end(), size, accent, &mut action, location);
                    ui.add_space(12.0);
                }
            }
        })
        .response;
    response.context_menu(|ui| {
        if ui.button("Edit Markdown").clicked() {
            action.edit = true;
            ui.close();
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_table_only_lays_out_visible_rows() {
        let ctx = context();
        let mut text = "| Row | Value |\n| --- | --- |\n".to_owned();
        for n in 0..10000 {
            text.push_str(&format!("| Row {n} | **value {n}** |\n"));
        }
        let started = std::time::Instant::now();
        let output = render_frame(&ctx, &text, vec![]);
        let first_frame = started.elapsed();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Row 0"));
        assert!(!labels.contains(&"Row 9999"));
        assert!(
            labels.len() < 160,
            "{} text layouts for a 10000-row table",
            labels.len()
        );
        let started = std::time::Instant::now();
        for _ in 0..20 {
            render_frame(&ctx, &text, vec![]);
        }
        eprintln!(
            "10000-row table: first frame {:?}; warm frame average {:?}; {} visible text layouts",
            first_frame,
            started.elapsed() / 20,
            labels.len()
        );
    }
    #[test]
    fn virtual_table_can_scroll_to_its_last_rows() {
        let ctx = context();
        let mut text = "| Row | Value |\n| --- | --- |\n".to_owned();
        for n in 0..1000 {
            text.push_str(&format!("| Row {n} | value {n} |\n"));
        }
        let frame = |offset: Option<f32>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(800.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let scroll = egui::ScrollArea::vertical()
                            .id_salt("large-table")
                            .animated(false);
                        let scroll = if let Some(offset) = offset {
                            scroll.vertical_scroll_offset(offset)
                        } else {
                            scroll
                        };
                        scroll.show(ui, |ui| {
                            render(
                                ui,
                                &text,
                                &Kind::Table,
                                16.0,
                                Color32::RED,
                                (std::path::Path::new("."), std::path::Path::new("note.md")),
                            );
                        });
                    });
                },
            )
        };
        frame(None);
        frame(Some(1_000_000.0));
        frame(None);
        let output = frame(None);
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Row 999"))
        );
        assert!(output.shapes.len() < 300);
    }
    #[test]
    fn code_fences_and_headings_follow_markdown_syntax() {
        assert_eq!(
            code_content("~~~~rust\nlet café = 1;\n~~~~"),
            "let café = 1;\n"
        );
        assert_eq!(code_content("    first\n    second\n"), "first\nsecond\n");
        assert_eq!(code_content("````md\n```inside\n````"), "```inside\n");
        assert_eq!(
            heading_content("Setext title\n============\n"),
            "Setext title"
        );
        assert_eq!(heading_content("## A **bold** `title` ##"), "A bold title");
        let source = "Before\n\n    // first line\n    let note = 1;\n\nAfter\n";
        let block = blocks(source)
            .into_iter()
            .find(|b| matches!(b.kind, Kind::Code(_)))
            .unwrap();
        assert_eq!(
            code_content(&source[block.range]),
            "// first line\nlet note = 1;\n"
        );
    }
    fn render_frame(ctx: &egui::Context, text: &str, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(800.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                prepare_document(ctx, std::path::Path::new("note.md"), text);
                egui::CentralPanel::default().show(ctx, |ui| {
                    for block in cached_blocks(ctx, std::path::Path::new("note.md"), text).iter() {
                        render(
                            ui,
                            &text[block.range.clone()],
                            &block.kind,
                            16.0,
                            Color32::RED,
                            (std::path::Path::new("."), std::path::Path::new("note.md")),
                        );
                    }
                });
            },
        )
    }
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            egui::FontFamily::Name("bold".into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        ctx
    }
    #[test]
    fn reference_links_resolve_across_blocks_without_showing_definitions() {
        let ctx = context();
        let text = "A [reference][KEY] and [shortcut].\n\n- [list link][key]\n\n[key]: https://example.com \"Title\"\n[shortcut]: other.md\n";
        let output = render_frame(&ctx, text, vec![]);
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert!(
            labels
                .iter()
                .any(|t| t.galley.text().contains("A reference and shortcut."))
        );
        assert!(labels.iter().any(|t| t.galley.text().contains("list link")));
        let linked_sections = labels
            .iter()
            .flat_map(|t| &t.galley.job.sections)
            .filter(|s| s.format.underline.width > 0.0)
            .count();
        assert_eq!(linked_sections, 3);
        assert!(
            labels
                .iter()
                .all(|t| !t.galley.text().contains("https://example.com"))
        );
        assert!(is_mermaid("{.MERMAID} title=Example"));
        assert!(!is_mermaid("rust"));
    }
    #[test]
    fn rich_table_keeps_escaped_pipes_aliases_and_ltr_word_order() {
        let text =
            "| Left | Right |\n| :--- | ---: |\n| café \\| pipe | [[Other Note\\|Open note]] |\n";
        let ctx = context();
        let output = render_frame(&ctx, text, vec![]);
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    Some((t.galley.text(), t.pos.x))
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.iter().any(|(s, _)| s.contains("café | pipe")));
        assert!(
            labels.iter().any(|(s, _)| *s == "Open note"),
            "right alignment must preserve normal reading order"
        );
        assert!(labels.iter().all(|(s, _)| !s.contains("[[")));
    }
    #[test]
    fn rendered_selection_copies_across_paragraphs() {
        let ctx = context();
        let text = "Alpha\n\nBeta\n";
        let output = render_frame(&ctx, text, vec![]);
        let rect = |name: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        (t.galley.text() == name)
                            .then_some(egui::Rect::from_min_size(t.pos, t.galley.size()))
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        let first = rect("Alpha");
        let last = rect("Beta");
        let start = first.left_center();
        let end = last.right_center();
        render_frame(
            &ctx,
            text,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render_frame(&ctx, text, vec![egui::Event::PointerMoved(end)]);
        render_frame(
            &ctx,
            text,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let output = render_frame(&ctx, text, vec![egui::Event::Copy]);
        assert!(output.platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s.contains("Alpha") && s.contains("Beta"))));
    }
    #[test]
    fn blocks_preserve_utf8_ranges_and_fences() {
        let text = "# Héllo\n\n- [x] café\n\n```mermaid\nflowchart TD\nA --> B\n```\n";
        let b = blocks(text);
        assert_eq!(b.len(), 3);
        assert_eq!(b[1].kind, Kind::Task(true));
        assert_eq!(&text[b[1].range.clone()], "- [x] café\n");
        assert_eq!(b[2].kind, Kind::Code("mermaid".into()));
    }
    #[test]
    fn tables_and_multiline_paragraphs() {
        let b = blocks("One\ntwo\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n");
        assert_eq!(b.len(), 2);
        assert_eq!(b[1].kind, Kind::Table);
    }
}
