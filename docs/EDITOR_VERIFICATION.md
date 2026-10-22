# Current editor behavior and verification

Updated 2026-10-22. See VERIFICATION.md for the current suite and remaining limits.

Notes open rendered. Clicking prose or a rich block opens one continuous native
egui TextEdit at that block; F4 toggles the view and Escape returns to reading.
The visible Preview/F4 footer control is removed; the F4 shortcut remains.
Find temporarily opens editing and restores the prior view when dismissed.

Inactive paragraphs, lists, tables, images, math and diagrams render in place.
The active source region exposes Markdown. Folding preserves every original
buffer character; selection, copying and undo span the whole document. Rendered
prose also supports selection across paragraphs. Per-document cursor and undo
state survive tab switches and renames during the session.

Formatting and replacements share the native undo stack. Supported operations
include emphasis, links/code, headings, task/bullet/numbered lists, continuation,
indentation, line move/copy/delete/insert and multiple occurrence editing.
IME input returns to the primary caret. Find/Replace supports Unicode-aware
literal matches, case/whole-word options, navigation, highlights and undoable
replacement. Regex mode adds captures and pattern errors; literal mode keeps replacement
strings literal. Rust regex syntax does not support look-around/backreferences.

Rich-block unfolding anchors the clicked source position to its screen height.
Fold measurements compensate layout changes above visible text. The document
scroll area applies targets immediately so animated caret centering cannot
compete with anchoring. Explicit Find/navigation still reveals its target.
Diagrams accept mixed-case Mermaid fence labels and source metadata; opening
source selects the complete fence and exposes an inline Preview action.

Tests exercise real egui keyboard/pointer dispatch, selection/clipboard, undo,
replacements, list commands, path remapping, rich widget replacement and distant
diagram anchoring. Native captures verify rendered appearance; no claim of full
CodeMirror keymap, accessibility certification or desktop automation parity is made.

Background save snapshots never replace newer typing. Closing waits for the
latest save; failed writes leave the draft open. Delete moves notes to the OS
Recycle Bin after saving. Conflicts require explicit disk/draft/copy resolution.
Undo is not persisted across restarts. Large tables virtualize visible row layout. Arbitrary list-subtree reordering
and perfect layout invariance remain outside coverage.
