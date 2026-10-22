use super::*;
use egui::{Event, FontDefinitions, FontFamily, OutputCommand, RawInput, Rect, vec2};

#[test]
fn unfolding_a_distant_diagram_preserves_its_screen_anchor() {
    let text = format!(
        "{}\n~~~MERMAID title=example\nflowchart LR\nA-->B\n~~~\n{}",
        "Paragraph before.\n\n".repeat(35),
        "Paragraph after.\n\n".repeat(50)
    );
    let mut h = Harness::new(&text);
    let block = crate::markdown::blocks(&text)
        .into_iter()
        .find(|b| matches!(b.kind, crate::markdown::Kind::Code(_)))
        .unwrap();
    let character = char_index(&text, block.range.start);
    h.editor.begin_block(&h.ctx, &h.path, &text, &block);
    h.editor.anchor_at(character, 120.0);
    h.frame(vec![], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    let output = h.frame(vec![], Modifiers::NONE);
    let y = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.pos.y + t.galley.pos_from_cursor(CCursor::new(character)).top())
            }
            _ => None,
        })
        .unwrap();
    assert!((y - 120.0).abs() < 3.0, "unfolded source moved to {y}");
    assert_eq!(h.text, text);
    assert_eq!(selection(&h.ctx, &h.path, &text).head, character);
}

#[test]
fn renamed_editor_keeps_selection_and_undo_redo() {
    let mut h = Harness::new("hello");
    transaction(&h.ctx, &h.path, &mut h.text, |text, _| {
        text.push_str(" world");
        Selection::caret(11)
    });
    let old = h.path.clone();
    let new = PathBuf::from("renamed.md");
    h.editor.remap_path(&old, &new);
    h.editor.apply_path_changes(&h.ctx);
    h.path = new;
    assert_eq!(selection(&h.ctx, &h.path, &h.text).head, 11);
    assert!(TextEditState::load(&h.ctx, id(&old)).is_none());
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, "hello");
    h.key(Key::Y, Modifiers::CTRL);
    assert_eq!(h.text, "hello world");
}

#[test]
fn mermaid_source_selects_the_whole_fence_and_preview_preserves_unicode() {
    let source = "```mermaid\nflowchart LR\nA-->B\n```\nénd\n";
    let mut h = Harness::new(source);
    let block = crate::markdown::blocks(source).remove(0);
    h.editor.begin_block(&h.ctx, &h.path, source, &block);
    assert_eq!(
        selection(&h.ctx, &h.path, source).range(),
        0..char_index(source, block.range.end)
    );
    let output = h.frame(vec![], Modifiers::NONE);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "Preview" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Mermaid source has an inline Preview button");
    for pressed in [true, false] {
        h.frame(
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
    }
    assert!(!h.ctx.memory(|m| m.has_focus(id(&h.path))));
    assert_eq!(h.text, source);
}

struct Harness {
    ctx: Context,
    editor: Editor,
    text: String,
    path: PathBuf,
    time: f64,
    scroll: f32,
}
impl Harness {
    fn new(text: &str) -> Self {
        let ctx = Context::default();
        let mut fonts = FontDefinitions::default();
        let family = fonts.families[&FontFamily::Proportional].clone();
        fonts
            .families
            .insert(FontFamily::Name("test".into()), family.clone());
        fonts
            .families
            .insert(FontFamily::Name("bold".into()), family);
        ctx.set_fonts(fonts);
        let mut h = Self {
            ctx,
            editor: Editor::default(),
            text: text.into(),
            path: PathBuf::from("first.md"),
            time: 0.0,
            scroll: 0.0,
        };
        h.editor.focus();
        h.frame(vec![], Modifiers::NONE);
        h
    }
    fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(780.0, 500.0))),
                time: Some(self.time),
                events,
                modifiers,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.editor.find_bar(ui, &self.path, &mut self.text);
                    let scroll = egui::ScrollArea::vertical()
                        .animated(false)
                        .id_salt("test-scroll")
                        .show(ui, |ui| {
                            self.editor.show(
                                ui,
                                &self.path,
                                &mut self.text,
                                Options {
                                    font: "test".into(),
                                    size: 16.0,
                                    accent: Color32::from_rgb(255, 100, 0),
                                    width: 700.0,
                                },
                            );
                        });
                    self.scroll = scroll.state.offset.y;
                });
            },
        )
    }
    fn key(&mut self, key: Key, mut modifiers: Modifiers) {
        modifiers.command = modifiers.ctrl;
        self.frame(
            vec![
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers,
                },
            ],
            modifiers,
        );
    }
}

#[test]
fn live_widgets_keep_source_and_survive_whole_document_replacement() {
    let original = "# Heading\n\n| A | B |\n| --- | --- |\n| **bold** | link |\n\n$$\\frac{1}{2}$$\n\n- [ ] Task\n";
    let mut h = Harness::new(original);
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(h.text, original);
    h.key(Key::A, Modifiers::CTRL);
    let output = h.frame(vec![Event::Copy], Modifiers::NONE);
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, OutputCommand::CopyText(s) if s == original))
    );
    h.frame(vec![Event::Text("short".into())], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(h.text, "short");
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, original);
}

#[test]
fn nested_numbered_indent_resets_and_outdent_continues_parent() {
    let mut h = Harness::new("1. Parent\n2. Child");
    h.editor.begin_at(&h.ctx, &h.path, h.text.chars().count());
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.text, "1. Parent\n  1. Child");
    h.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(h.text, "1. Parent\n2. Child");
    let mut h = Harness::new("1. Parent\n   - Child");
    h.editor
        .begin_at(&h.ctx, &h.path, "1. Parent\n   - ".chars().count());
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.text, "1. Parent\n- Child");
}

#[test]
fn occurrence_selection_types_at_all_matches_and_undoes_once() {
    let mut h = Harness::new("café and café and café");
    h.editor.begin_at(&h.ctx, &h.path, 1);
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::D, Modifiers::CTRL);
    h.key(Key::D, Modifiers::CTRL);
    h.key(Key::D, Modifiers::CTRL);
    h.frame(vec![Event::Text("tea".into())], Modifiers::NONE);
    assert_eq!(h.text, "tea and tea and tea");
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, "café and café and café");
}

#[test]
fn line_shortcuts_and_list_backspace_use_real_keyboard_dispatch() {
    let mut h = Harness::new("first\nsecond\nthird");
    h.editor.begin_at(&h.ctx, &h.path, 8);
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::ArrowUp, Modifiers::ALT);
    assert_eq!(h.text, "second\nfirst\nthird");
    h.key(Key::ArrowDown, Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(h.text, "second\nsecond\nfirst\nthird");
    h.key(Key::K, Modifiers::CTRL | Modifiers::SHIFT);
    assert_eq!(h.text, "second\nfirst\nthird");
    let mut h = Harness::new("  - item");
    h.editor.begin_at(&h.ctx, &h.path, 4);
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.text, "- item");
    h.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.text, "item");
}

#[test]
fn continuous_selection_clipboard_formatting_and_native_undo() {
    let original = "Alpha\n\nBeta café 🦀";
    let mut h = Harness::new(original);
    h.key(Key::A, Modifiers::CTRL);
    assert_eq!(
        selection(&h.ctx, &h.path, &h.text).range(),
        0..original.chars().count()
    );
    let output = h.frame(vec![Event::Copy], Modifiers::NONE);
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c,OutputCommand::CopyText(s) if s==original))
    );
    h.key(Key::B, Modifiers::CTRL);
    assert_eq!(h.text, format!("**{original}**"));
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, original);
    h.key(Key::Y, Modifiers::CTRL);
    assert_eq!(h.text, format!("**{original}**"));
}

#[test]
fn list_enter_and_tab_use_the_real_keyboard_dispatch() {
    let mut h = Harness::new("- first");
    let len = h.text.chars().count();
    h.editor.begin_at(&h.ctx, &h.path, len);
    h.frame(vec![], Modifiers::NONE);
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.text, "- first\n- ");
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.text, "- first\n  - ");
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.text, "- first\n- ");
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.text, "- first\n");
}

#[test]
fn find_enter_and_replace_enter_preserve_field_focus() {
    let mut h = Harness::new("café\n\ncafé\nCAFÉ");
    h.editor.open_find(&h.ctx, &h.path, &h.text, true);
    h.editor.find.query = "café".into();
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(h.editor.find.current, Some(0));
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.editor.find.current, Some(1));
    assert!(h.ctx.memory(|m| m.has_focus(Id::new("find-query"))));
    h.key(Key::Enter, Modifiers::SHIFT);
    assert_eq!(h.editor.find.current, Some(0));
    h.editor.find.replacement = "$1".into();
    h.ctx
        .memory_mut(|m| m.request_focus(Id::new("replace-text")));
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.text, "$1\n\ncafé\nCAFÉ");
    assert!(h.ctx.memory(|m| m.has_focus(Id::new("replace-text"))));
}

#[test]
fn replace_all_is_one_native_undo_transaction() {
    let original = "one\n\nONE café one";
    let mut h = Harness::new(original);
    h.editor.find.query = "one".into();
    h.editor.find.replacement = "🦀".into();
    transaction(&h.ctx, &h.path, &mut h.text, |text, current| {
        assert_eq!(h.editor.find.replace_all(text), 3);
        current
    });
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(h.text, "🦀\n\n🦀 café 🦀");
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, original);
    h.key(Key::Y, Modifiers::CTRL);
    assert_eq!(h.text, "🦀\n\n🦀 café 🦀");
}

#[test]
fn formatting_after_undo_discards_the_old_redo_branch() {
    let mut h = Harness::new("word");
    h.key(Key::A, Modifiers::CTRL);
    h.key(Key::B, Modifiers::CTRL);
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, "word");
    h.key(Key::I, Modifiers::CTRL);
    assert_eq!(h.text, "*word*");
    h.key(Key::Y, Modifiers::CTRL);
    assert_eq!(
        h.text, "*word*",
        "A fresh edit must invalidate the earlier bold redo"
    );
}

#[test]
fn tab_switch_keeps_each_documents_selection_and_undo() {
    let mut h = Harness::new("Alpha beta");
    h.editor.select(
        &h.ctx,
        &h.path,
        Selection {
            anchor: 6,
            head: 10,
        },
    );
    h.key(Key::B, Modifiers::CTRL);
    assert_eq!(h.text, "Alpha **beta**");
    let first = h.text.clone();
    h.path = "second.md".into();
    h.text = "Second document".into();
    h.editor.focus();
    h.frame(vec![], Modifiers::NONE);
    h.path = "first.md".into();
    h.text = first;
    h.editor.focus();
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(selection(&h.ctx, &h.path, &h.text).range(), 8..12);
    h.key(Key::Z, Modifiers::CTRL);
    assert_eq!(h.text, "Alpha beta");
}

#[test]
fn find_next_scrolls_to_an_offscreen_match_without_editor_focus() {
    let text = format!(
        "needle\n{}\nneedle",
        (0..150).map(|n| format!("Line {n}\n")).collect::<String>()
    );
    let mut h = Harness::new(&text);
    h.editor.open_find(&h.ctx, &h.path, &h.text, false);
    h.editor.find.query = "needle".into();
    h.frame(vec![], Modifiers::NONE);
    h.editor.navigate(true);
    h.frame(vec![], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    h.frame(vec![], Modifiers::NONE);
    assert_eq!(h.editor.find.current, Some(1));
    assert!(
        h.scroll > 1000.0,
        "Find should reveal the distant match, scroll was {}",
        h.scroll
    );
    assert!(h.ctx.memory(|m| m.has_focus(Id::new("find-query"))));
}
