use eframe::egui::{self, Id, Ui};
use mermaid_rs_renderer::{Theme, compute_layout, parse_mermaid_strict, render_svg};
use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};
mod canvas;
pub const HEIGHT: f32 = 480.0;

#[derive(Clone)]
struct Diagram {
    bytes: Arc<[u8]>,
    size: egui::Vec2,
}
type Rendered = Result<Diagram, String>;
#[derive(Clone, Default)]
struct Cache {
    entries: VecDeque<(u64, Rendered)>,
}
impl Cache {
    fn get(&mut self, key: u64) -> Option<Rendered> {
        let index = self.entries.iter().position(|(k, _)| *k == key)?;
        let entry = self.entries.remove(index)?;
        let result = entry.1.clone();
        self.entries.push_back(entry);
        Some(result)
    }
    fn insert(&mut self, key: u64, result: Rendered) {
        if self.entries.len() >= 50 {
            self.entries.pop_front();
        }
        self.entries.push_back((key, result));
    }
}
fn mix(bg: egui::Color32, fg: egui::Color32, amount: f32) -> egui::Color32 {
    let channel = |b: u8, f: u8| (b as f32 + (f as f32 - b as f32) * amount).round() as u8;
    egui::Color32::from_rgb(
        channel(bg.r(), fg.r()),
        channel(bg.g(), fg.g()),
        channel(bg.b(), fg.b()),
    )
}
fn render(source: &str, colors: [egui::Color32; 3], contrast: f32, font: &str) -> Rendered {
    let source = source.trim_start_matches('\u{feff}').trim();
    if source.len() > 50_000 {
        return Err(
            "Diagram exceeds the 50 KB rendering limit. Reduce its source to continue.".into(),
        );
    }
    let [accent, bg, fg] = colors;
    let hex = |c: egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let surface = hex(mix(bg, fg, contrast * 0.10));
    let border = hex(mix(bg, fg, contrast * 0.48));
    let line = hex(mix(bg, fg, 0.45));
    let mut theme = Theme::modern();
    theme.font_family = format!("{font}, Segoe UI, sans-serif");
    theme.font_size = 14.0;
    theme.background = "transparent".into();
    theme.primary_color = surface.clone();
    theme.primary_text_color = hex(fg);
    theme.primary_border_color = border.clone();
    theme.line_color = line.clone();
    theme.text_color = hex(fg);
    theme.secondary_color = hex(mix(bg, fg, contrast * 0.16));
    theme.tertiary_color = surface.clone();
    theme.cluster_background = hex(mix(bg, fg, contrast * 0.13));
    theme.cluster_border = border.clone();
    theme.edge_label_background = hex(bg);
    theme.sequence_actor_fill = surface.clone();
    theme.sequence_actor_border = border.clone();
    theme.sequence_actor_line = line;
    theme.sequence_note_fill = surface.clone();
    theme.sequence_note_border = border.clone();
    theme.sequence_activation_fill = hex(mix(bg, accent, 0.18));
    theme.sequence_activation_border = hex(accent);
    theme.git_colors = std::array::from_fn(|i| {
        hex(mix(
            fg,
            accent,
            [1.0, 0.72, 0.50, 0.35, 0.24, 0.16, 0.10, 0.06][i],
        ))
    });
    theme.git_branch_label_colors = std::array::from_fn(|_| hex(bg));
    theme.git_commit_label_color = hex(fg);
    theme.git_commit_label_background = hex(bg);
    theme.git_tag_label_color = hex(fg);
    theme.git_tag_label_background = surface;
    theme.git_tag_label_border = border;
    theme.pie_colors = std::array::from_fn(|i| hex(mix(fg, accent, (0.72_f32).powi(i as i32))));
    theme.pie_title_text_color = hex(fg);
    theme.pie_legend_text_color = hex(fg);
    theme.pie_section_text_color = hex(bg);
    theme.pie_stroke_color = hex(bg);
    theme.pie_outer_stroke_color = hex(bg);
    let parsed = parse_mermaid_strict(source).map_err(|e| e.to_string())?;
    let mut layout = mermaid_rs_renderer::LayoutConfig::default();
    // The high-level renderer ignores init_config. Honor the common flowchart
    // spacing directives explicitly, with bounds for untrusted note contents.
    if let Some(flowchart) = parsed.init_config.as_ref().and_then(|c| c.get("flowchart")) {
        for (name, target) in [
            ("nodeSpacing", &mut layout.node_spacing),
            ("rankSpacing", &mut layout.rank_spacing),
        ] {
            if let Some(value) = flowchart.get(name).and_then(|v| v.as_f64())
                && value.is_finite()
                && (1.0..=1000.0).contains(&value)
            {
                *target = value as f32;
            }
        }
    }
    let graph = compute_layout(&parsed.graph, &theme, &layout);
    let svg = render_svg(&graph, &theme, &layout);
    if svg.len() > 1_000_000 {
        return Err("Diagram is too complex to display. Simplify its source.".into());
    }
    // Read the root viewBox, never the raster loader's requested texture size.
    let values: Vec<f32> = svg
        .split("viewBox=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let size = match values.as_slice() {
        [_, _, w, h] if w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0 => {
            egui::vec2(*w, *h)
        }
        _ => return Err("Diagram has invalid dimensions.".into()),
    };
    Ok(Diagram {
        bytes: Arc::from(svg.into_bytes()),
        size,
    })
}

pub fn show(ui: &mut Ui, source: &str) -> bool {
    let colors = [
        ui.visuals().hyperlink_color,
        ui.visuals().panel_fill,
        ui.visuals().text_color(),
    ];
    let (contrast, font) = ui
        .ctx()
        .data(|d| d.get_temp::<(f32, String)>(Id::new("diagram-appearance")))
        .unwrap_or((0.6, "Segoe UI".into()));
    let mut hash = DefaultHasher::new();
    (source, colors, contrast.to_bits(), &font).hash(&mut hash);
    let key = hash.finish();
    let cached = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(Id::new("mermaid-cache"))
            .get(key)
    });
    let result = cached.unwrap_or_else(|| {
        let result = render(source, colors, contrast, &font);
        ui.ctx().data_mut(|d| {
            d.get_temp_mut_or_default::<Cache>(Id::new("mermaid-cache"))
                .insert(key, result.clone())
        });
        result
    });
    let id = ui.id().with("diagram-canvas");
    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<canvas::Canvas>(id).clone());
    let mut edit = canvas::show(ui, &result, key, &mut state, HEIGHT, false);
    #[cfg(debug_assertions)]
    if std::env::var("NOTES_CAPTURE_SCENE").as_deref() == Ok("diagram-viewer")
        && !ui.ctx().data(|d| {
            d.get_temp::<bool>(id.with("capture-opened"))
                .unwrap_or(false)
        })
    {
        state.fullscreen = true;
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("capture-opened"), true));
    }
    if state.fullscreen {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            state.fullscreen = false;
        }
        let screen = ui.ctx().content_rect().shrink(12.0);
        let mut close = false;
        let response = egui::Modal::new(id.with("fullscreen"))
            .area(egui::Area::new(id.with("fullscreen-area")).fixed_pos(screen.min))
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .corner_radius(10)
                    .inner_margin(12),
            )
            .show(ui.ctx(), |ui| {
                ui.set_width((screen.width() - 24.0).max(1.0));
                ui.horizontal(|ui| {
                    ui.label("Diagram");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close · Esc").clicked() {
                            close = true;
                        }
                    });
                });
                let height = (screen.height() - 76.0).max(100.0);
                edit |= canvas::show(ui, &result, key, &mut state, height, true);
            });
        state.fullscreen &= !close && !response.should_close() && !edit;
    }
    if edit {
        state.reset();
        state.fullscreen = false;
    }
    if state.fullscreen {
        ui.ctx()
            .data_mut(|d| d.insert_temp(Id::new("diagram-fullscreen-open"), true));
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, state));
    edit
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flowchart_init_spacing_changes_the_native_layout() {
        let colors = [
            egui::Color32::RED,
            egui::Color32::BLACK,
            egui::Color32::WHITE,
        ];
        let compact = render(
            "%%{init: {\"flowchart\": {\"rankSpacing\": 20}}}%%\nflowchart LR\nA-->B-->C",
            colors,
            0.6,
            "Segoe UI",
        )
        .unwrap();
        let wide = render(
            "%%{init: {\"flowchart\": {\"rankSpacing\": 200}}}%%\nflowchart LR\nA-->B-->C",
            colors,
            0.6,
            "Segoe UI",
        )
        .unwrap();
        assert!(
            wide.size.x > compact.size.x + 5.0,
            "compact {:?}, wide {:?}, init {:?}",
            compact.size,
            wide.size,
            parse_mermaid_strict(
                "%%{init: {\"flowchart\": {\"rankSpacing\": 200}}}%%\nflowchart LR\nA-->B-->C"
            )
            .unwrap()
            .init_config
        );
        assert!(
            render(
                "---\ntitle: Notes\n---\nflowchart LR\nA-->B",
                colors,
                0.6,
                "Segoe UI"
            )
            .is_ok()
        );
    }
    #[test]
    fn extended_mermaid_families_and_bom_render_natively() {
        for source in [
            "\u{feff}flowchart LR\nWrite-->Save",
            "classDiagram\nclass Note\nclass Folder\nFolder --> Note",
            "stateDiagram-v2\n[*] --> Draft\nDraft --> Saved",
            "pie\n\"Notes\" : 7\n\"Ideas\" : 3",
            "xychart-beta\nx-axis [Mon, Tue]\nbar [3, 8]",
            "quadrantChart\nDraft: [0.3, 0.6]",
            "timeline\n2026 : Notes released",
            "journey\nsection Writing\nDraft: 4: Writer",
            "mindmap\nroot((Notes))\n  Ideas\n  Projects",
            "gitGraph\ncommit\nbranch draft\ncommit",
            "requirementDiagram\nrequirement saving {\nid: 1\ntext: Save notes\nrisk: low\nverifymethod: test\n}",
            "C4Context\nPerson(writer, \"Writer\")\nSystem(notes, \"Notes\")\nRel(writer, notes, \"Writes\")",
            "sankey-beta\nIdeas,Drafts,8\nDrafts,Published,5",
            "zenuml\nWriter->Notes: Save",
            "block-beta\nDraft --> Saved",
            "packet-beta\n0-7: \"Kind\"\n8-15: \"Size\"",
            "kanban\ndraft[Drafts]\n  note[New note]",
            "architecture-beta\ngroup notes(cloud)[Notes]\nservice app(server)[App] in notes",
            "radar-beta\naxis Speed, Clarity, Focus\ncurve Notes {4,5,5}",
            "treemap-beta\nNotes: 10\n  Drafts: 6\n  Published: 4",
        ] {
            let diagram = render(
                source,
                [
                    egui::Color32::from_rgb(255, 101, 0),
                    egui::Color32::from_gray(17),
                    egui::Color32::from_gray(207),
                ],
                0.6,
                "Segoe UI",
            )
            .unwrap_or_else(|e| panic!("{}: {e}", source.lines().next().unwrap()));
            assert!(diagram.size.is_finite() && diagram.size.x > 0.0 && diagram.size.y > 0.0);
        }
    }
    #[test]
    fn real_diagrams_have_natural_dimensions_in_both_themes() {
        for (bg, fg) in [
            (egui::Color32::BLACK, egui::Color32::WHITE),
            (egui::Color32::WHITE, egui::Color32::BLACK),
        ] {
            for source in [
                "flowchart TD\nA[Write]-->B[Save]",
                "sequenceDiagram\nAlice->>Bob: Hello",
                "erDiagram\nFILE ||--o{ TAB : opens",
                "gantt\ntitle Rollout\ndateFormat YYYY-MM-DD\nsection Work\nWrite :2026-04-01, 8d",
            ] {
                let diagram =
                    render(source, [egui::Color32::RED, bg, fg], 0.6, "Segoe UI").unwrap();
                assert!(diagram.size.x > 0.0 && diagram.size.y > 0.0);
                let svg = std::str::from_utf8(&diagram.bytes).unwrap();
                assert!(
                    svg.contains(&format!(
                        "fill=\"{}\"",
                        if bg == egui::Color32::BLACK {
                            "#0f0f0f"
                        } else {
                            "#f0f0f0"
                        }
                    )) || !source.starts_with("flowchart")
                );
            }
        }
    }
    #[test]
    fn invalid_and_oversized_source_are_recoverable_errors() {
        for source in ["not a diagram".into(), "x".repeat(50_001)] {
            assert!(render(&source, [egui::Color32::BLACK; 3], 0.6, "Segoe UI").is_err());
        }
    }
    #[test]
    fn cache_keeps_recently_used_diagrams_and_errors() {
        let mut cache = Cache::default();
        for key in 0..50 {
            cache.insert(key, Err("syntax".into()));
        }
        assert!(cache.get(0).is_some());
        cache.insert(50, Err("syntax".into()));
        assert!(cache.get(1).is_none());
        assert!(cache.get(0).is_some());
        assert_eq!(cache.entries.len(), 50);
    }
}
