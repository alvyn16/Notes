# Notes

Fresh implementation: no original application source, checkout, JavaScript,
Tauri application, or original binary is installed or embedded.

| Module | Responsibility | Depends on |
| --- | --- | --- |
| workspace | Markdown files, gitignore-aware tree, safe writes, conflict detection | — |
| index | Worker inventory/content cache, filesystem events and ranked search | workspace |
| app/file_io | Serial background note load/save and conflict resolution; completion and close guards | workspace |
| links | Indexed note/media resolution, wiki aliases and heading anchors | workspace |
| math | Native LaTeX layout and SVG rendering | — |
| media | Image loading/error feedback and full-window image viewer | links |
| theme / edge_blur | Shared theme colors/font selection and native bottom-edge effect | — |
| windows / broker | Workspace ownership, authenticated file dispatch, sessions and tab tear-off | workspace |
| distribution / updates | Native per-user setup, file registration, verified background downloads and replacement | windows |
| markdown | Styled paragraphs, links, tasks, tables, code and native diagrams | — |
| editor | Continuous document editing, formatting, undo and Find/Replace | — |
| shell | Native window, tabs, history, sidebar selection/moves, search and settings | workspace, index, links, markdown, editor, windows |

Build order: workspace → markdown → shell → native verification.

The supplied screenshots govern appearance. Newer upstream sidebar sections do
not override their simpler folder tree. Windows uses Ctrl in place of Cmd.
