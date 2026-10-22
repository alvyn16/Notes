# Notes feature parity implementation

Implement the eight capabilities requested from the comparison, preserving the
native Rust/egui architecture and the existing recovery/rename/ignore fixes.
No upstream application code is cloned or installed.

## Build order and contracts

1. Background workspace index: scan/search/clean-file reads run on a worker;
   cancel stale generations after workspace changes; filesystem events request
   debounced refreshes. Verify a 10,000-note fixture and stale-result rejection.
2. Windows: open a note directly, compact single-file mode, new workspace
   windows with independent profiles. Verify argument and session isolation.
3. Markdown: structured lists/tables, wiki/heading links, image embeds and
   native math rendering; retain original source on disk.
4. Live editing: syntax folds away from the active selection without removing
   buffer characters; document-wide copy/selection/undo remain available.
5. Shortcuts: line move/copy/delete/insert, occurrence selection and editing,
   list Backspace and indentation/numbering.
6. Organization: sidebar selection and moves with safe destinations/collisions,
   context move action and drag/drop; synchronize all path-based state.
7. Diagrams: bounded canvas, pan/zoom/fit/source controls and theme colors.
8. Final checks: focused regression tests, full suite, strict Clippy, formatting,
   release build, native checks where service is available, portable package.

## Verification

Use real temporary files and egui keyboard/pointer tests. Compare source bytes
before/after rendering and exercise actual app routes. Native screenshot checks
remain distinct from headless tests. Do not claim exact upstream parity from a
successful build. Keep any residual limitations explicit.
