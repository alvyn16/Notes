# Implemented feature areas - 2026-10-22

Fresh native Rust implementation; the original Writer application was not cloned,
installed or run. These changes cover the eight areas requested in the comparison.

## Implemented behavior

1. **Live editing and selection:** source markup folds outside the active
   selection. Inactive blocks render as native widgets over source-preserving
   layout regions. Clicking unfolds a block; keyboard navigation and Find expose
   source. Full-document copy and undo keep original Markdown characters. The
   rendered view supports text selection across paragraphs.
2. **Media and links:** Markdown images and Obsidian image embeds, local/remote
   images, wiki aliases, extensionless notes and heading anchors (including
   duplicate heading suffixes). Native LaTeX SVG rendering uses LaTeX-Rust and
   embedded STIX Two Math; parse failures display editable source. Wiki syntax
   inside code is left alone. Media paths come from the background inventory.
3. **Lists/tables:** structured Markdown parsing drives ordered/nested lists,
   task items, rich table cells, escaped pipes and column alignment. Table wiki
   aliases use an escaped pipe, e.g. `[[Note\|Alias]]`, as required by Markdown
   table syntax. Rendering never rewrites stored text.
4. **Shortcuts:** Ctrl+D occurrence selections and simultaneous typing/copy/cut;
   line move/copy/delete/insert; bracket indentation; nested list Backspace;
   numbered indentation starts a sub-list at 1 and outdent continues the parent.
   Programmatic changes share native undo. Navigation or a pointer click clears
   extra carets; IME input returns to the primary caret.
5. **Organization:** Ctrl/Shift sidebar selection, drag/drop, folder picker move
   action, descendant pruning and preflight collision checks. Moves synchronize
   open documents, history, expanded folders, session and search paths. A filesystem
   failure during a batch reports an error; successful earlier moves remain applied.
6. **Windows/files:** opening workspaces creates independent native processes
   with separate workspace profiles. File arguments open compact windows. Native
   file picker and OS file drops can open notes in new windows. Ctrl+Shift+N opens
   the workspace picker; Ctrl+1–9 selects workspace tabs. Active tabs reveal on
   navigation without a permanent visible scrollbar.
7. **Diagrams:** 480-point clipped canvas with subtle theme colors, hover/focus
   corner icons, drag/keyboard pan, cursor-anchored pinch or Ctrl-wheel zoom,
   fit/reset and fullscreen (Escape closes). Small diagrams retain natural size.
   Edit code selects the entire fence; the source has a Preview button. Errors
   retain the same frame and source controls. A bounded 50-entry LRU includes
   source, theme colors, font and contrast; errors are cached too.
8. **Filesystem performance:** workers own ordinary note loading/saving and
   conflict resolution, scanning, content reads, incremental
   metadata caches and search. Filesystem notifications are coalesced, with a
   periodic reconciliation fallback. Old worker receivers are dropped with the
   workspace; snapshots predating local writes/moves are rejected. Search runs
   on the worker, ranks filename matches first and returns at most 200 snippets.

## Verification

The suite contains 95 tests. New tests exercise real egui keyboard dispatch,
multi-occurrence typing/undo, block-fold source preservation and replacement,
paragraph selection/copy, table label order/aliases, actual sidebar pointer
drag/drop, move collision handling, diagram button input/theme colors, links,
math, watcher events and stale snapshot rejection. Existing recovery, rename,
ignore, save and Find/Replace tests remain enabled.

A real temporary 10,000-note vault indexed and searched in approximately
0.8–0.9 seconds in the debug test run on this machine, off the UI thread. This
does not characterize slow disks, network shares or peak memory use.

Debug builds can capture the actual egui/Glow window through egui's screenshot
command using `NOTES_CAPTURE_PATH`. Isolated native captures under
`artifacts/parity/` cover compact file opening, workspace mode, live rich blocks,
images/math/tables and the diagram canvas. Fixtures and profiles are separate
from the user's vault. The external computer-use service was unavailable, so
pointer/keyboard interaction evidence comes from headless egui tests; native
captures verify rendering rather than the entire end-to-end workflow.

## Limits

This is not a pixel-equality certification or complete CommonMark/CodeMirror,
KaTeX or upstream Mermaid compatibility claim. Diagram types and mathematical
commands depend on the native renderers. Rich tables use content-aware, resizable
column widths, horizontal overflow and visible-row virtualization. Unvisited row
heights are estimated; initial parsing and prefix construction remain linear.
Clicked rich-block positions and visible text are anchored during unfolding;
source and rendered layouts still have different heights. Each workspace window is a
separate process, coordinated by an authenticated per-user local broker. Files
route to the matching workspace, and tabs can tear off after saving. Undo history
is not transferred between processes.
Ordinary note loading/saving and conflict resolution run on a serial note I/O
worker. New edits remain dirty until their own snapshot saves; close/move/rename
actions wait for pending saves. Other filesystem transactions and recovery review
still include synchronous work; see VERIFICATION.md. Recovery discovery on
restart is implemented. Native setup, file registration and verified update
delivery are implemented; valid code signing still requires a current certificate.
See DISTRIBUTION.md for release prerequisites and checks. Font licenses and the LaTeX-Rust notices ship with the
portable package.
