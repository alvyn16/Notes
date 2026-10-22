# Continuous editor and Find/Replace - original design

Implemented and extended. This file records the original design, not a pending
task list. See EDITOR_VERIFICATION.md for current live rich blocks and behavior.

Implement one native egui TextEdit for each complete Markdown document. A stable
document ID retains native selection and undo history across focus/tab changes;
programmatic formatting and replacements join the same undo history. Keep F4
as a rendered preview. Editing shows Markdown syntax with heading, emphasis,
link and code styling; it does not hide source characters from the caret.

Add Ctrl+B/I/K/E, strikethrough, headings, list/quote formatting, list continuation,
and Tab/Shift+Tab indentation. Standard selection, clipboard, undo and redo apply
across the entire document. No per-paragraph edit widgets remain.

Find/Replace: Ctrl+F and Ctrl+H, literal Unicode-aware search, match case, whole
words, next/previous with wraparound (F3/Shift+F3 and Ctrl+G/Ctrl+Shift+G), visible
active match and scrolling, replace selected match and replace all as undoable
transactions. Empty queries cannot replace. Replacement strings are literal.

Verification: command/search unit tests including Unicode, multiline selection,
CRLF, empty queries, replacement offsets and undo boundaries; headless egui
keyboard tests; native editing, navigation, replacement and persistence checks;
strict Clippy, formatting and release build. Do not alter user notes for tests.
