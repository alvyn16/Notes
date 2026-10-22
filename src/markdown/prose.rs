use super::*;
use egui::{TextFormat, text::LayoutJob};

#[derive(Default)]
struct Run {
    job: LayoutJob,
    links: Vec<(Range<usize>, String)>,
}
fn flush(ui: &mut Ui, run: &mut Run, action: &mut Action) {
    if run.job.text.is_empty() {
        return;
    }
    run.job.wrap.max_width = ui.available_width();
    run.job.halign = ui.layout().horizontal_align();
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(run.job.clone()));
    let response = ui.add(
        egui::Label::new(run.job.clone())
            .selectable(true)
            .sense(Sense::click()),
    );
    if let Some(pointer) = response.hover_pos() {
        let cursor = galley.cursor_from_pos(pointer - response.rect.min);
        let byte = crate::editor::commands::byte_index(&run.job.text, cursor.index);
        if let Some((_, url)) = run.links.iter().find(|(range, _)| range.contains(&byte)) {
            response.clone().on_hover_text(url);
            if response.clicked() {
                action.link = Some(url.clone());
            }
        } else if response.clicked() {
            action.edit = true;
        }
    }
    *run = Run::default();
}
pub(super) fn inline(
    ui: &mut Ui,
    text: &str,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    let text = super::math_delimiters::normalize(text);
    if Parser::new_ext(&text, Options::ENABLE_MATH)
        .any(|event| matches!(event, Event::InlineMath(_)))
    {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), size * 1.7),
            egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
            |ui| tokens(ui, &text, size, accent, action, location),
        );
    } else {
        tokens(ui, &text, size, accent, action, location);
    }
}
fn tokens(
    ui: &mut Ui,
    text: &str,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    let mut text = crate::links::wiki_markdown(text);
    if let Some((_, definitions)) = ui.ctx().data(|d| {
        d.get_temp::<(u64, std::sync::Arc<String>)>(egui::Id::new((
            "markdown-references",
            location.1,
        )))
    }) && !definitions.is_empty()
    {
        text.push_str("\n\n");
        text.push_str(&definitions);
    }
    let mut run = Run::default();
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let mut image = false;
    let mut link = None::<String>;
    for event in Parser::new_ext(&text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_MATH) {
        match event {
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Strikethrough) => strike = true,
            Event::End(TagEnd::Strikethrough) => strike = false,
            Event::Start(Tag::Link { dest_url, .. }) => link = Some(dest_url.into_string()),
            Event::End(TagEnd::Link) => link = None,
            Event::Start(Tag::Image { dest_url, .. }) => {
                flush(ui, &mut run, action);
                image = true;
                action.edit |= crate::media::show(ui, &dest_url, location);
            }
            Event::End(TagEnd::Image) => image = false,
            Event::InlineMath(ref source) | Event::DisplayMath(ref source) => {
                flush(ui, &mut run, action);
                if crate::math::show(ui, source, size, matches!(event, Event::DisplayMath(_)))
                    .clicked()
                {
                    action.edit = true;
                }
            }
            Event::Text(ref value)
            | Event::Code(ref value)
            | Event::Html(ref value)
            | Event::InlineHtml(ref value) => {
                if image {
                    continue;
                }
                let code = matches!(event, Event::Code(_));
                let mut format = TextFormat {
                    font_id: FontId::new(
                        size,
                        if bold {
                            crate::theme::bold(
                                &ui.style()
                                    .override_font_id
                                    .as_ref()
                                    .map(|f| f.family.clone())
                                    .unwrap_or(egui::FontFamily::Proportional),
                            )
                        } else {
                            ui.style()
                                .override_font_id
                                .as_ref()
                                .map(|f| f.family.clone())
                                .unwrap_or(egui::FontFamily::Proportional)
                        },
                    ),
                    color: ui.visuals().text_color(),
                    italics: italic,
                    ..Default::default()
                };
                if code {
                    format.font_id = FontId::monospace(size * 0.9);
                    format.background = ui.visuals().code_bg_color;
                }
                if strike {
                    format.strikethrough = Stroke::new(1.0_f32, format.color);
                }
                if let Some(url) = &link {
                    format.color = accent;
                    format.underline = Stroke::new(1.0_f32, accent);
                    run.links.push((
                        run.job.text.len()..run.job.text.len() + value.len(),
                        url.clone(),
                    ));
                }
                run.job.append(value, 0.0, format);
            }
            Event::SoftBreak => {
                run.job.append(
                    " ",
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(size),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }
            Event::HardBreak => {
                run.job.append(
                    "\n",
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(size),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }
            _ => {}
        }
    }
    flush(ui, &mut run, action);
}
