use super::*;
use crate::markdown::{self, Block, Kind};
use std::hash::{Hash, Hasher};

pub(super) struct Fold {
    pub block: Block,
    pub height: f32,
    id: Id,
}
pub(super) fn source_preview(
    editor: &mut Editor,
    ui: &mut Ui,
    path: &Path,
    text: &str,
    output: &egui::text_edit::TextEditOutput,
    active: &Option<std::ops::Range<usize>>,
    width: f32,
) {
    let Some(active) = active else {
        return;
    };
    for block in markdown::cached_blocks(ui.ctx(), path, text).iter() {
        if !matches!(&block.kind, Kind::Code(language) if markdown::is_mermaid(language))
            || active.start >= block.range.end
            || active.end < block.range.start
        {
            continue;
        }
        let cursor = output
            .galley
            .pos_from_cursor(CCursor::new(char_index(text, block.range.start)));
        let rect = egui::Rect::from_min_size(
            output.galley_pos + egui::vec2((width - 82.0).max(0.0), cursor.top()),
            egui::vec2(82.0, 26.0),
        );
        if rect.intersects(ui.clip_rect())
            && ui
                .put(rect, egui::Button::new("Preview"))
                .on_hover_text("Return to diagram")
                .clicked()
        {
            editor.begin_at(
                ui.ctx(),
                path,
                char_index(text, block.range.end) + usize::from(block.range.end < text.len()),
            );
            editor.focus = false;
            editor.reveal = None;
            editor.anchor_at(
                char_index(text, block.range.start),
                output.galley_pos.y + cursor.top(),
            );
            ui.ctx().memory_mut(|m| m.surrender_focus(super::id(path)));
            ui.ctx().request_repaint();
        }
    }
}
pub(super) fn prepare(
    ui: &Ui,
    path: &Path,
    text: &str,
    active: &Option<std::ops::Range<usize>>,
    find: bool,
    size: f32,
) -> Vec<Fold> {
    if find {
        return vec![];
    }
    markdown::cached_blocks(ui.ctx(), path, text)
        .iter()
        .filter(|block| {
            !matches!(block.kind, Kind::Heading(_))
                && !active
                    .as_ref()
                    .is_some_and(|a| a.start < block.range.end && a.end >= block.range.start)
        })
        .cloned()
        .map(|block| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (
                path,
                block.range.start,
                &text[block.range.clone()],
                size.to_bits(),
            )
                .hash(&mut hash);
            let id = Id::new(("live-block-height", hash.finish()));
            let estimate = match block.kind {
                Kind::Table => text[block.range.clone()].lines().count() as f32 * size * 2.0,
                Kind::Code(ref lang) if markdown::is_mermaid(lang) => crate::diagrams::HEIGHT + 4.0,
                _ => size * 2.0,
            };
            let height = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(id))
                .unwrap_or(estimate);
            Fold { block, height, id }
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    editor: &mut Editor,
    ui: &mut Ui,
    path: &Path,
    text: &mut String,
    output: &egui::text_edit::TextEditOutput,
    folds: &[Fold],
    size: f32,
    accent: Color32,
    width: f32,
) -> bool {
    let root = ui
        .ctx()
        .data(|data| data.get_temp::<PathBuf>(Id::new("workspace-root")))
        .unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).to_owned());
    let original = text.clone();
    let mut changed = false;
    for fold in folds {
        let at = output
            .galley
            .pos_from_cursor(CCursor::new(char_index(&original, fold.block.range.start)));
        let position = output.galley_pos + egui::vec2(0.0, at.top());
        let rect = egui::Rect::from_min_size(position, egui::vec2(width, fold.height));
        if !rect.intersects(ui.clip_rect()) {
            continue;
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(fold.id)
                .max_rect(rect)
                .layout(egui::Layout::top_down(Align::LEFT)),
        );
        child.set_width(width);
        child.spacing_mut().item_spacing.y = 0.0;
        let action = markdown::render(
            &mut child,
            &original[fold.block.range.clone()],
            &fold.block.kind,
            size,
            accent,
            (&root, path),
        );
        let height = child.min_rect().height().max(size * 1.55) + 4.0;
        if (height - fold.height).abs() > 1.0 {
            ui.ctx().data_mut(|data| data.insert_temp(fold.id, height));
            ui.ctx().request_repaint();
            editor.reflow = true;
        }
        if action.edit {
            editor.begin_block(ui.ctx(), path, &original, &fold.block);
            editor.anchor_at(char_index(&original, fold.block.range.start), position.y);
        }
        if action.link.is_some() {
            editor.link = action.link;
        }
        let mut toggles = action.toggles;
        if action.toggle {
            toggles.push(3);
        }
        for local in toggles {
            let offset = fold.block.range.start + local;
            changed |= transaction(ui.ctx(), path, text, |text, selected| {
                let checked = text
                    .as_bytes()
                    .get(offset)
                    .is_some_and(|c| *c == b'x' || *c == b'X');
                text.replace_range(offset..offset + 1, if checked { " " } else { "x" });
                selected
            });
        }
    }
    changed
}
