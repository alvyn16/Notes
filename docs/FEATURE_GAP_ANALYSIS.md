# Current feature gaps

Updated 2026-10-22. The original 2026-10-20 audit described an earlier working
tree. It is superseded by this status; it must not be used as a current missing
feature checklist. The original application was not installed or cloned.

## Implemented since the audit

- Continuous editing, document-wide selection/undo and Find/Replace.
- Background scanning/search, ordinary note loading/saving and conflict resolution.
- Structured lists/tables, image embeds/viewer, wiki/reference/heading links,
  native math and interactive Mermaid canvases.
- Sidebar multiselect/moves, active-tab reveal, independent workspace windows.
- Current recovery drafts, standalone gitignore, consistent paths after renames,
  backlink updates and guarded save-before-close behavior.
- Theme-aware styling, resizable tables and rich-block scroll anchoring.

## Work still open

1. Full Markdown/LaTeX/Mermaid syntax and pixel compatibility. Current native
   renderers support a tested subset; unsupported syntax exposes errors/source.
2. Broader large-note/memory profiling and column virtualization. Row layout is now virtualized.
3. Move/create/delete/backlink/recovery operations still contain synchronous
   filesystem work. Network-share latency and failure scenarios need wider tests.
4. Cross-process undo transfer and merging tabs into existing windows. File routing and tear-off are implemented.
5. Recent workspace management. Pinned/recent notes and regex Find/Replace are implemented.
6. Valid code signing and published-release end-to-end update verification. Setup, optional file registration and automatic update delivery are implemented.
7. Accessibility and multi-monitor/DPI coverage beyond existing targeted checks.

See [PARITY_FEATURES.md](PARITY_FEATURES.md) for implemented features and
[VERIFICATION.md](VERIFICATION.md) for evidence and precise limits. This local
status is not a newly refreshed audit of the upstream application's releases.
