use super::*;
#[test]
fn pinned_and_recent_notes_persist_and_follow_renames() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let path = workspace::create(&app.prefs.root, "Pinned", false).unwrap();
    app.open(path.clone(), true);
    app.toggle_pin(&path);
    assert!(app.prefs.show_library);
    app.open(path.clone(), true);
    assert_eq!(app.prefs.recent.len(), 1);
    let renamed = app.rename_note(&path, "Renamed").unwrap();
    assert_eq!(app.prefs.pinned.as_slice(), std::slice::from_ref(&renamed));
    assert_eq!(app.prefs.recent.as_slice(), std::slice::from_ref(&renamed));
    let encoded = serde_json::to_string(&app.prefs).unwrap();
    let decoded: Preferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.pinned.as_slice(), std::slice::from_ref(&renamed));
    app.toggle_pin(&renamed);
    assert!(app.prefs.pinned.is_empty());
}
#[test]
fn controls_and_inline_code_adapt_to_both_themes() {
    let ctx = egui::Context::default();
    let mut app = notes();
    for light in [false, true] {
        app.prefs.light = light;
        app.style(&ctx);
        let style = ctx.style();
        let bg = style.visuals.panel_fill;
        let fg = style.visuals.text_color();
        assert_eq!(
            style.visuals.code_bg_color,
            crate::theme::code_background(bg, fg)
        );
        let fill = style.visuals.widgets.inactive.bg_fill;
        if light {
            assert!(fill.r() > 200 && fg.r() < 70);
        } else {
            assert!(fill.r() < 70 && fg.r() > 150);
        }
    }
    app.prefs.light = true;
    app.prefs.light_background = [245, 248, 252];
    app.prefs.light_foreground = [25, 35, 45];
    app.style(&ctx);
    assert_eq!(
        ctx.style().visuals.panel_fill,
        Color32::from_rgb(245, 248, 252)
    );
    assert_eq!(
        ctx.style().visuals.text_color(),
        Color32::from_rgb(25, 35, 45)
    );
}

#[test]
fn note_renames_keep_extensions_support_case_changes_and_update_backlinks() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let note = workspace::create(&app.prefs.root, "first.markdown", false).unwrap();
    let reader = workspace::create(&app.prefs.root, "Reader", false).unwrap();
    fs::write(&reader, "[[first|Alias]] [link](first.markdown#part)").unwrap();
    app.open(note.clone(), true);
    app.open(reader.clone(), true);
    let renamed = app.rename_note(&note, "New name").unwrap();
    assert!(renamed.ends_with("New name.markdown"));
    assert_eq!(
        app.docs[1].text,
        "[[New name.markdown|Alias]] [link](New%20name.markdown#part)"
    );
    assert_eq!(fs::read_to_string(&reader).unwrap(), app.docs[1].text);
    let changed = app.rename_note(&renamed, "NEW NAME.markdown").unwrap();
    assert_eq!(app.docs[0].path, changed);
    let listed = fs::read_dir(&app.prefs.root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    assert!(listed.contains(&std::ffi::OsString::from("NEW NAME.markdown")));
    assert_eq!(
        app.rename_note(&changed, "NEW NAME.markdown").unwrap(),
        changed
    );
    assert!(app.rename_note(&changed, "hidden.txt").is_err());
}

#[test]
fn saving_a_conflict_copy_closes_the_conflict_without_touching_original() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let note = workspace::create(&app.prefs.root, "Note", false).unwrap();
    app.open(note.clone(), true);
    app.docs[0].text = "draft".into();
    fs::write(&note, "disk").unwrap();
    assert!(!app.save_all());
    assert!(
        app.save_conflict_copy(&app.prefs.root.join("Missing/Copy.md"))
            .is_err()
    );
    assert_eq!(app.docs.len(), 1);
    assert!(app.docs[0].conflict);
    let copy = app.prefs.root.join("Copy.md");
    app.save_conflict_copy(&copy).unwrap();
    assert!(app.docs.is_empty());
    assert!(app.save_all());
    assert_eq!(fs::read_to_string(note).unwrap(), "disk");
    assert_eq!(fs::read_to_string(copy).unwrap(), "draft");
}

pub(super) fn notes() -> Notes {
    Notes {
        prefs: Preferences::default(),
        entries: vec![],
        docs: vec![],
        active: 0,
        expanded: BTreeSet::new(),
        history: vec![],
        history_pos: 0,
        settings: false,
        view: view::View::default(),
        palette: true,
        query: String::new(),
        palette_index: 0,
        focus_palette: true,
        dialog: None,
        input: String::new(),
        error: None,
        notice: String::new(),
        last_edit: Instant::now(),
        last_scan: Instant::now(),
        editor: editor::Editor::default(),
        palette_cache: vec![],
        index: None,
        search_query: String::new(),
        compact: false,
        selected_paths: BTreeSet::new(),
        selection_anchor: None,
        startup_open: false,
        recovery: recovery::Recovery::default(),
        edge_blur: crate::edge_blur::EdgeBlur::default(),
        io: file_io::FileIo::default(),
        broker: None,
        updater: crate::updates::Updater::default(),
    }
}

#[test]
fn sidebar_drag_moves_a_note_and_updates_its_open_tab() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let src = app.prefs.root.join("Source");
    let dest = app.prefs.root.join("Destination");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&dest).unwrap();
    let path = workspace::create(&src, "Move", false).unwrap();
    app.entries = workspace::scan(&app.prefs.root);
    app.expanded.insert(src);
    let ctx = egui::Context::default();
    let frame = |app: &mut Notes, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 500.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| app.tree(ui, app.prefs.root.clone(), 0));
            },
        )
    };
    let output = frame(&mut app, vec![]);
    let point = |name: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    (t.galley.text() == name).then_some(t.pos + vec2(10.0, 7.0))
                } else {
                    None
                }
            })
            .unwrap()
    };
    let start = point("Move");
    let end = point("Destination");
    app.open(path.clone(), true);
    frame(
        &mut app,
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
    frame(
        &mut app,
        vec![egui::Event::PointerMoved(start + vec2(12.0, 0.0))],
    );
    frame(&mut app, vec![egui::Event::PointerMoved(end)]);
    frame(
        &mut app,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(dest.join("Move.md").exists());
    assert!(!path.exists());
    assert_eq!(app.docs[0].path, dest.join("Move.md"));
}

#[test]
fn stale_snapshots_cannot_revert_local_saves_or_renamed_search_paths() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let path = workspace::create(&app.prefs.root, "Note", false).unwrap();
    app.open(path.clone(), true);
    let stale = Instant::now();
    app.docs[0].text = "local saved version".into();
    assert!(app.save_all());
    let ctx = egui::Context::default();
    app.apply_index_update(
        &ctx,
        crate::index::Update::Snapshot(crate::index::Snapshot {
            entries: vec![],
            files: [(path.clone(), std::sync::Arc::from("stale version"))]
                .into_iter()
                .collect(),
            started: stale,
            paths: std::sync::Arc::new(vec![]),
        }),
    );
    assert_eq!(app.docs[0].text, "local saved version");
    app.search_query = "note".into();
    app.apply_index_update(
        &ctx,
        crate::index::Update::Search("note".into(), vec![(path.clone(), "stale".into())], stale),
    );
    assert!(app.palette_cache.is_empty());
    app.apply_index_update(
        &ctx,
        crate::index::Update::Snapshot(crate::index::Snapshot {
            entries: app.entries.clone(),
            files: [(path.clone(), std::sync::Arc::from("external version"))]
                .into_iter()
                .collect(),
            started: Instant::now(),
            paths: std::sync::Arc::new(vec![path]),
        }),
    );
    assert_eq!(app.docs[0].text, "external version");
}

#[test]
fn selected_folder_moves_prune_children_and_reject_colliding_batches() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let folder = app.prefs.root.join("Folder");
    let dest = app.prefs.root.join("Dest");
    fs::create_dir(&folder).unwrap();
    fs::create_dir(&dest).unwrap();
    let note = workspace::create(&folder, "One", false).unwrap();
    app.open(note.clone(), true);
    app.move_selected(&[folder.clone(), note], &dest).unwrap();
    assert!(dest.join("Folder/One.md").exists());
    assert_eq!(app.docs[0].path, dest.join("Folder/One.md"));
    assert!(
        app.move_selected(std::slice::from_ref(&dest), &dest.join("Folder"))
            .is_err()
    );
    let a = workspace::create(&app.prefs.root, "Same", false).unwrap();
    let b = workspace::create(&dest, "Same", false).unwrap();
    fs::create_dir(app.prefs.root.join("Target")).unwrap();
    assert!(
        app.move_selected(&[a.clone(), b.clone()], &app.prefs.root.join("Target"))
            .is_err()
    );
    assert!(a.exists() && b.exists());
}

#[test]
fn control_and_shift_selection_are_persistent() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let paths: Vec<_> = ["One", "Two", "Three"]
        .iter()
        .map(|name| workspace::create(&app.prefs.root, name, false).unwrap())
        .collect();
    app.entries = paths
        .iter()
        .map(|p| Entry {
            path: p.clone(),
            directory: false,
        })
        .collect();
    app.select_path(&paths[0], egui::Modifiers::NONE);
    app.select_path(&paths[2], egui::Modifiers::SHIFT);
    assert_eq!(app.selected_paths.len(), 3);
    app.select_path(&paths[1], egui::Modifiers::CTRL);
    assert_eq!(app.selected_paths.len(), 2);
}

#[test]
fn autosave_refreshes_conflicted_draft_including_return_to_original_text() {
    let root = tempfile::tempdir().unwrap();
    let path = workspace::create(root.path(), "Note", false).unwrap();
    let mut app = notes();
    app.docs.push(Document::load(path.clone()).unwrap());
    app.docs[0].text = "initial draft".into();
    fs::write(&path, "external version").unwrap();
    assert!(!app.save_all());
    app.error = None;
    let original = app.docs[0].saved.clone();
    for text in ["later edits", original.as_str()] {
        app.docs[0].text = text.into();
        assert!(!app.save_all());
        assert!(app.error.is_none());
        assert_eq!(
            fs::read_to_string(path.with_extension("md.writer-recovery")).unwrap(),
            text
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "external version");
    }
    app.docs[0].refresh().unwrap();
    assert_eq!(app.docs[0].text, original);
    assert!(app.docs[0].conflict);
    // A failed backup is reported and still prevents closing/switching workspace.
    let recovery = path.with_extension("md.writer-recovery");
    fs::remove_file(&recovery).unwrap();
    fs::create_dir(&recovery).unwrap();
    app.docs[0].text = "must keep".into();
    assert!(!app.save_all());
    assert!(app.error.as_deref().unwrap().contains("Could not preserve"));
    assert_eq!(app.docs[0].text, "must keep");
}

#[test]
fn folder_rename_and_archive_keep_navigation_and_session_paths_current() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let old = app.prefs.root.join("Projects");
    fs::create_dir_all(old.join("nested")).unwrap();
    let first = workspace::create(&old, "First", false).unwrap();
    let nested = workspace::create(&old.join("nested"), "Second", false).unwrap();
    let unrelated = workspace::create(&app.prefs.root, "Projects-other", false).unwrap();
    app.open(first.clone(), true);
    app.open(nested.clone(), true);
    app.open(unrelated.clone(), true);
    app.navigate(-1);
    app.prefs.tabs = app.docs.iter().map(|d| d.path.clone()).collect();
    app.expanded = [old.clone(), old.join("nested"), app.prefs.root.clone()]
        .into_iter()
        .collect();
    app.palette_cache = vec![
        (first, "first".into()),
        (nested, "second".into()),
        (unrelated.clone(), "other".into()),
    ];
    let renamed = app.rename_note(&old, "Renamed").unwrap();
    assert!(!old.exists());
    assert_eq!(app.history_pos, 1);
    let new_first = renamed.join("First.md");
    let new_second = renamed.join("nested/Second.md");
    assert_eq!(
        app.history,
        [new_first.clone(), new_second.clone(), unrelated.clone()]
    );
    assert_eq!(app.prefs.tabs, app.history);
    assert!(app.expanded.contains(&renamed.join("nested")));
    assert!(app.expanded.contains(&renamed));
    assert!(!app.expanded.contains(&old));
    assert_eq!(app.palette_cache[0].0, new_first);
    assert_eq!(app.palette_cache[1].0, new_second);
    app.navigate(-1);
    assert_eq!(app.docs[app.active].path, new_first);
    app.navigate(1);
    assert_eq!(app.docs[app.active].path, new_second);
    app.archive(&new_second);
    let archived = app.prefs.root.join("Archive/Second.md");
    assert!(archived.exists());
    app.navigate(-1);
    app.navigate(1);
    assert_eq!(app.docs[app.active].path, archived);
    assert!(app.prefs.tabs.contains(&archived));
    assert_eq!(app.palette_cache[1].0, archived);
    app.navigate(1);
    assert_eq!(app.docs[app.active].path, unrelated);
    assert!(app.error.is_none());
}

#[test]
fn failed_rename_keeps_navigation_and_files_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let mut app = notes();
    app.prefs.root = root.path().canonicalize().unwrap();
    let first = workspace::create(&app.prefs.root, "First", false).unwrap();
    let second = workspace::create(&app.prefs.root, "Second", false).unwrap();
    app.open(first.clone(), true);
    assert!(app.rename_note(&first, "Second.md").is_err());
    assert_eq!(app.history.as_slice(), std::slice::from_ref(&first));
    assert_eq!(app.docs[0].path, first);
    assert!(second.exists());
    assert!(app.rename_note(&first, "../Outside.md").is_err());
    assert!(first.exists());
}
