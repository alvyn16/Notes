# Notes implementation contract

## Objective
Create new Rust code for a native Windows egui application matching the supplied
Writer screenshots. Read upstream documentation as behavioral reference only.
No cloning, installing, copying, or running the original app.

## Visual contract
Borderless resizable 1200 × 800 window; 240 px sidebar; 54 px tab strip;
near-black #111111 document; #cfcfcf prose; muted #929292 navigation;
orange #ff6500 links/tasks; rounded gray active rows; thin outline icons;
centered maximum 736 px writing column; generous top inset; bottom-right stats.
Zen-style monochrome minimize, maximize/restore and close controls occupy the
top-right corner, replacing the initial macOS traffic-light controls. The desktop
wallpaper outside the window is not application content. Sidebar tint is painted
as a subtle gradient; true wallpaper blur is a separate Windows integration.

## Behavior
Open folders through the native picker or a CLI path. Create files and folders,
rename and archive through context menus, navigate a collapsible tree, search
filenames and content, use multiple tabs and back/forward history. Render Markdown
with clickable local/web links, task toggles, tables, code, and supported native
diagram types. Use a continuous styled Markdown editor with document-wide
selection and undo; F4 toggles the rendered preview. Find/Replace includes
Unicode-aware literal or regex matches, case/word options, next/previous, highlighting,
match scrolling and undoable replacement transactions. See docs/EDITOR_PLAN.md.
Autosave changed notes, detect external conflicts, preserve draft recovery files.
Persist appearance and session. Settings and command palette follow references.
Seed a separate example vault on first run, never modify an existing vault to
populate examples. Keep all document storage local and make no telemetry calls.
Settings may check the Notes GitHub release API daily; automatic installation is
opt-in. Register file handling per user without overriding Windows UserChoice.

## Commands
`cargo run --release` launches; `cargo run --release -- <folder>` opens a vault.
`cargo test` checks storage/parser behavior; `cargo clippy --all-targets -- -D warnings`
and `cargo fmt --all -- --check` validate code; `cargo build --release --locked`
produces the Windows executable.

## Structure and style
`src/workspace.rs`: disk I/O; `src/markdown.rs`: parsing/rendering;
`src/app.rs`: interactions and settings; `src/main.rs`: entrypoint;
`src/demo.rs`: fresh reference fixture; `tests/`: end-to-end disk behavior.
Rustfmt conventions; errors return `Result` and are visible in the app.

## Verification and completion
Test real files, conflict recovery, path boundaries, Markdown parsing and task
updates. Build the executable and inspect the native rendered window. Record
remaining differences explicitly; compilation alone does not prove 100% visual
or functional parity. Never silently overwrite conflicting disk changes.
