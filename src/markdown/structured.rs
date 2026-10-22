use super::*;
use pulldown_cmark::Alignment;

#[derive(Debug)]
pub(super) struct Node {
    pub event: Event<'static>,
    pub range: Range<usize>,
    pub children: Vec<Node>,
}
pub(super) fn parse(text: &str) -> Vec<Node> {
    let mut stack: Vec<Node> = vec![];
    let mut roots = vec![];
    for (event, range) in Parser::new_ext(
        text,
        Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_MATH,
    )
    .into_offset_iter()
    {
        if matches!(event, Event::End(_)) {
            if let Some(mut node) = stack.pop() {
                node.range.end = node.range.end.max(range.end);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    roots.push(node);
                }
            }
        } else {
            let start = matches!(event, Event::Start(_));
            let node = Node {
                event: event.into_static(),
                range,
                children: vec![],
            };
            if start {
                stack.push(node);
            } else if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                roots.push(node);
            }
        }
    }
    roots
}
pub(super) fn list(
    ui: &mut Ui,
    source: &str,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    for node in parse(source) {
        render_list(ui, source, &node, size, accent, action, location);
    }
}
fn render_list(
    ui: &mut Ui,
    source: &str,
    node: &Node,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    if let Event::Start(Tag::List(start)) = node.event {
        for (i, item) in node.children.iter().enumerate() {
            ui.push_id(item.range.start, |ui| {
                ui.horizontal_top(|ui| {
                    let task = item.children.iter().find_map(|n| match n.event {
                        Event::TaskListMarker(value) => Some(value),
                        _ => n.children.iter().find_map(|n| {
                            if let Event::TaskListMarker(v) = n.event {
                                Some(v)
                            } else {
                                None
                            }
                        }),
                    });
                    if let Some(mut checked) = task {
                        if ui.checkbox(&mut checked, "").changed()
                            && let Some(offset) = source[item.range.clone()].find('[')
                        {
                            action.toggles.push(item.range.start + offset + 1);
                        }
                    } else {
                        ui.add_sized(
                            [28.0, size * 1.5],
                            egui::Label::new(
                                start.map_or("•".into(), |n| format!("{}.", n + i as u64)),
                            ),
                        );
                    }
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width());
                        let mut inline_start = None;
                        let mut inline_end = 0;
                        for child in &item.children {
                            match child.event {
                                Event::Start(Tag::List(_)) => {
                                    flush(
                                        ui,
                                        source,
                                        &mut inline_start,
                                        inline_end,
                                        size,
                                        accent,
                                        action,
                                        location,
                                    );
                                    render_list(ui, source, child, size, accent, action, location);
                                }
                                Event::TaskListMarker(_) => {}
                                Event::Start(Tag::Paragraph) => {
                                    flush(
                                        ui,
                                        source,
                                        &mut inline_start,
                                        inline_end,
                                        size,
                                        accent,
                                        action,
                                        location,
                                    );
                                    let mut range = child.range.clone();
                                    if let Some(marker) = child
                                        .children
                                        .iter()
                                        .find(|n| matches!(n.event, Event::TaskListMarker(_)))
                                    {
                                        range.start = marker.range.end;
                                    }
                                    inline(
                                        ui,
                                        source[range].trim(),
                                        size,
                                        accent,
                                        action,
                                        location,
                                    );
                                }
                                _ => {
                                    inline_start.get_or_insert(child.range.start);
                                    inline_end = child.range.end;
                                }
                            }
                        }
                        flush(
                            ui,
                            source,
                            &mut inline_start,
                            inline_end,
                            size,
                            accent,
                            action,
                            location,
                        );
                    });
                });
            });
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn flush(
    ui: &mut Ui,
    source: &str,
    start: &mut Option<usize>,
    end: usize,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    if let Some(start) = start.take() {
        inline(
            ui,
            source[start..end].trim(),
            size,
            accent,
            action,
            location,
        );
    }
}
pub(super) fn table(
    ui: &mut Ui,
    source: &str,
    size: f32,
    accent: Color32,
    action: &mut Action,
    location: (&std::path::Path, &std::path::Path),
) {
    use std::sync::Arc;
    let source_key = egui::util::hash(source);
    let model_id = ui.id().with("table-model");
    let tables = ui
        .ctx()
        .data(|d| d.get_temp::<(u64, Arc<Vec<Node>>)>(model_id))
        .filter(|(key, _)| *key == source_key)
        .map(|(_, tables)| tables)
        .unwrap_or_else(|| {
            let tables = Arc::new(parse(source));
            ui.ctx()
                .data_mut(|d| d.insert_temp(model_id, (source_key, tables.clone())));
            tables
        });
    for table in tables.iter() {
        let Event::Start(Tag::Table(ref alignments)) = table.event else {
            continue;
        };
        let sizing_key = (
            source_key,
            size.to_bits(),
            ui.available_width().to_bits(),
            ui.style().override_font_id.clone(),
        );
        let sizing_id = ui.id().with("table-sizing");
        let (minimum, initial) = ui
            .ctx()
            .data(|d| d.get_temp::<(SizingKey, Vec<f32>, Vec<f32>)>(sizing_id))
            .filter(|(key, _, _)| key == &sizing_key)
            .map(|(_, min, initial)| (min, initial))
            .unwrap_or_else(|| {
                let (min, initial) =
                    column_widths(ui, source, &table.children, alignments.len(), size);
                ui.ctx().data_mut(|d| {
                    d.insert_temp(sizing_id, (sizing_key, min.clone(), initial.clone()))
                });
                (min, initial)
            });
        let widths_id = ui.id().with("table-column-widths");
        let mut widths = ui
            .ctx()
            .data(|d| d.get_temp::<Vec<f32>>(widths_id))
            .filter(|w| w.len() == initial.len())
            .unwrap_or(initial.clone());
        let height_key = (
            source_key,
            size.to_bits(),
            widths.iter().map(|w| w.to_bits()).collect::<Vec<_>>(),
            ui.style().override_font_id.clone(),
        );
        let heights_id = ui.id().with("table-row-heights");
        let mut heights = ui
            .ctx()
            .data(|d| d.get_temp::<(HeightKey, Arc<Vec<f32>>)>(heights_id))
            .filter(|(key, _)| key == &height_key)
            .map(|(_, heights)| heights)
            .unwrap_or_else(|| Arc::new(vec![size * 1.5 + 8.0; table.children.len()]));
        let mut prefix = Vec::with_capacity(heights.len() + 1);
        prefix.push(0.0);
        for height in heights.iter() {
            prefix.push(prefix.last().copied().unwrap_or(0.0) + height);
        }
        let top = ui.cursor().top();
        let visible = if table.children.len() > 128 {
            visible_rows(
                &prefix,
                ui.clip_rect().top() - top,
                ui.clip_rect().bottom() - top,
            )
        } else {
            0..table.children.len()
        };
        let mut changed_height = false;
        let mut headers = vec![];
        let mut table_rect = egui::Rect::NOTHING;
        egui::ScrollArea::horizontal()
            .id_salt("table-scroll")
            .show(ui, |ui| {
                let table_width =
                    widths.iter().sum::<f32>() + 16.0 * widths.len().saturating_sub(1) as f32;
                let (full_rect, _) = ui.allocate_exact_size(
                    vec2(table_width, prefix.last().copied().unwrap_or(0.0)),
                    Sense::hover(),
                );
                let mut grid_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("virtual-table-content")
                        .max_rect(egui::Rect::from_min_size(
                            full_rect.min + vec2(0.0, prefix[visible.start]),
                            vec2(
                                table_width,
                                (prefix[visible.end] - prefix[visible.start]).max(1.0),
                            ),
                        )),
                );
                let response = egui::Grid::new(ui.id().with("table"))
                    .start_row(visible.start)
                    .striped(true)
                    .spacing([16.0, 8.0])
                    .show(&mut grid_ui, |ui| {
                        for row_index in visible.clone() {
                            let row = &table.children[row_index];
                            let mut row_height = size * 1.5;
                            for (col, cell) in row.children.iter().enumerate() {
                                let align = alignments.get(col).unwrap_or(&Alignment::None);
                                let layout = match align {
                                    Alignment::Right => egui::Layout::top_down(egui::Align::RIGHT),
                                    Alignment::Center => {
                                        egui::Layout::top_down(egui::Align::Center)
                                    }
                                    _ => egui::Layout::top_down(egui::Align::LEFT),
                                };
                                let response = ui.allocate_ui_with_layout(
                                    vec2(widths.get(col).copied().unwrap_or(140.0), size * 1.5),
                                    layout,
                                    |ui| {
                                        ui.set_max_width(widths.get(col).copied().unwrap_or(140.0));
                                        let text = &source[cell.range.clone()];
                                        let text =
                                            if matches!(row.event, Event::Start(Tag::TableHead)) {
                                                format!("**{}**", text.trim())
                                            } else {
                                                text.trim().to_owned()
                                            };
                                        inline(ui, &text, size, accent, action, location);
                                    },
                                );
                                row_height = row_height.max(response.response.rect.height());
                                if row_index == 0 {
                                    headers.push(response.response.rect);
                                }
                            }
                            let measured = row_height + 8.0;
                            if (heights[row_index] - measured).abs() > 1.0 {
                                Arc::make_mut(&mut heights)[row_index] = measured;
                                changed_height = true;
                            }
                            ui.end_row();
                        }
                    });
                let _ = response;
                table_rect = full_rect;
                for (index, header) in headers.iter().enumerate() {
                    let handle = egui::Rect::from_min_max(
                        egui::pos2(header.right() + 4.0, table_rect.top()),
                        egui::pos2(header.right() + 12.0, table_rect.bottom()),
                    );
                    let response = ui
                        .interact(
                            handle,
                            ui.id().with(("resize-column", index)),
                            Sense::click_and_drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                        .on_hover_text("Drag to resize · Double-click to fit · Arrow keys adjust");
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::DragValue,
                            true,
                            format!("Column {} width", index + 1),
                        )
                    });
                    if response.clicked() {
                        response.request_focus();
                    }
                    let keyboard = if response.has_focus() {
                        ui.input_mut(|i| {
                            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                                16.0
                            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                                -16.0
                            } else {
                                0.0
                            }
                        })
                    } else {
                        0.0
                    };
                    if response.dragged() || keyboard != 0.0 {
                        widths[index] = (widths[index]
                            + if response.dragged() {
                                ui.input(|i| i.pointer.delta().x)
                            } else {
                                0.0
                            }
                            + keyboard)
                            .clamp(minimum[index], 1200.0);
                    }
                    if response.double_clicked() {
                        widths[index] = initial[index];
                    }
                    if response.hovered() || response.dragged() || response.has_focus() {
                        ui.painter().vline(
                            handle.center().x,
                            table_rect.y_range(),
                            Stroke::new(1.0_f32, accent),
                        );
                    }
                }
            });
        ui.ctx().data_mut(|d| {
            d.insert_temp(widths_id, widths);
            d.insert_temp(heights_id, (height_key, heights));
        });
        if changed_height {
            ui.ctx().request_repaint();
        }
    }
}
fn column_widths(
    ui: &Ui,
    source: &str,
    rows: &[Node],
    count: usize,
    size: f32,
) -> (Vec<f32>, Vec<f32>) {
    let font = ui
        .style()
        .override_font_id
        .clone()
        .unwrap_or(FontId::proportional(size));
    let mut minimum = vec![80.0_f32; count];
    let mut desired = minimum.clone();
    for row in rows.iter().take(128) {
        for (col, cell) in row.children.iter().enumerate().take(count) {
            let text: String = Parser::new(&source[cell.range.clone()])
                .filter_map(|e| match e {
                    Event::Text(t) | Event::Code(t) => Some(t.into_string()),
                    _ => None,
                })
                .collect();
            let measure = |text: &str| {
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(text.into(), font.clone(), ui.visuals().text_color())
                        .size()
                        .x
                })
            };
            for word in text.split_whitespace() {
                minimum[col] = minimum[col].max(measure(word) + 12.0).min(1200.0);
            }
            desired[col] = desired[col]
                .max((measure(&text) + 16.0).min(360.0))
                .max(minimum[col]);
        }
    }
    let room = (ui.available_width() - (count.saturating_sub(1) as f32) * 16.0).max(1.0);
    let min_total: f32 = minimum.iter().sum();
    let desired_total: f32 = desired.iter().sum();
    if desired_total > room && desired_total > min_total {
        let ratio = ((room - min_total) / (desired_total - min_total)).clamp(0.0, 1.0);
        for (width, min) in desired.iter_mut().zip(&minimum) {
            *width = *min + (*width - *min) * ratio;
        }
    }
    (minimum, desired)
}

type SizingKey = (u64, u32, u32, Option<FontId>);
type HeightKey = (u64, u32, Vec<u32>, Option<FontId>);
fn visible_rows(prefix: &[f32], top: f32, bottom: f32) -> std::ops::Range<usize> {
    let count = prefix.len().saturating_sub(1);
    if count == 0 || bottom < 0.0 || top > prefix[count] {
        return 0..0;
    }
    let first = prefix
        .partition_point(|y| *y < top)
        .saturating_sub(2)
        .min(count);
    let last = (prefix.partition_point(|y| *y <= bottom) + 2).min(count);
    first..last.max(first)
}
