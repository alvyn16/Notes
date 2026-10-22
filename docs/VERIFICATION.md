# Verification and remaining differences

Updated 2026-10-22. This is the current verification record. Older audit claims
are superseded by this file and PARITY_FEATURES.md.

## Automated verification

The suite contains 95 tests. Run `cargo test --locked`,
`cargo clippy --all-targets --locked -- -D warnings`, and
`cargo fmt --all -- --check`. Build/package with `scripts/build.ps1`.

Tests use temporary vaults and real egui keyboard/pointer events. Coverage includes:

- Pinned/recent persistence, path remapping and palette keyboard navigation.
- Regex captures, invalid patterns and zero-width replacements; literal mode
  continues to preserve dollar signs as text.
- Whole-document selection, clipboard, native undo, formatting, multiple
  occurrences, list operations, Find/Replace and tab state.
- Clicked rich-block screen anchoring while unfolding, including uppercase
  Mermaid fences with metadata. Document scroll targets apply immediately;
  measured fold-height changes compensate the visible source position.
- Background note loading, edits during an in-flight save, save-before-close,
  rename-after-save, stale reopen results and write failure/retry. Failed writes
  retain dirty text and cancel closing. External conflicts preserve the latest
  draft; disk/draft/copy resolution also runs on the note I/O worker.
- Reference links defined elsewhere in a document, structured lists/tables,
  escaped pipes, tilde/indented code, headings, wiki links and anchors.
- Dollar/LaTeX math delimiters, bare display environments, starred environment
  adapters, matrices, cases, aligned formulas, equation tags and recoverable errors.
- Native Mermaid families, BOM input, init flowchart spacing, invalid/oversized
  sources, canvas controls, cache eviction, dimensions and both themes.
- Image error/source actions and full-window image zoom/Escape.
- A 10,000-row table lays out 48 visible text runs in the local debug fixture;
  last-row scrolling has a separate regression. Cached block/table structures and
  sampled column fitting avoid repeating full row layout. A measured debug run
  took about 180 ms for the first frame and 41 ms per warm frame; this is not a
  release-build latency guarantee or a comparison benchmark against upstream.
- Real local broker sockets authenticate requests, reuse a workspace owner and
  restart after close. Egui pointer input verifies dirty-tab tear-off waits for save.
- Update metadata rejects downgrades, prereleases, foreign URLs and missing hashes.
  `scripts/test-distribution.ps1` additionally tests real per-user registry writes
  inside a temporary test namespace, quoted file associations, shortcuts, uninstall
  preserving notes, checksum rejection, executable replacement and backup.
- Recovery discovery, standalone gitignore, rename/backlink rollback, stale
  index snapshots, filesystem notifications and a real 10,000-note fixture.

A passing test suite is not proof of complete upstream syntax or visual parity.
The 10,000-note fixture verifies indexing/search behavior, not latency on a
network share or a benchmark of all file operations.

## Native rendering evidence

Debug builds capture the actual egui/Glow window using `NOTES_CAPTURE_PATH`
with an isolated `NOTES_PROFILE_PATH`. Captures under `artifacts/edge-blur/`
verify the progressive bottom blur in both themes. Captures under
`artifacts/style-media-compat/` cover light-theme code/table typography,
dark-theme formulas and error feedback, images and full-window media/diagrams.
Current compatibility-update captures live under `artifacts/io-compat/`.
`artifacts/shipping/` covers native setup, optional library sections and actual
workspace routing. The second native process exited with code 0 after forwarding
its file to the first window; the capture verifies the routed tab and content.

The screenshots verify rendering. Interaction assertions come from headless
egui tests; they are not an external desktop automation certification.
No original app was cloned, installed or embedded. User notes are not test fixtures.

## Current limits

- Exact screenshot equality and full CommonMark/CodeMirror, KaTeX or Mermaid
  compatibility are unproven. Native renderer output can differ from upstream.
  Starred LaTeX display environments use unnumbered aligned rows; precise
  gather/multline layout is not claimed. Mermaid init supports nodeSpacing and
  rankSpacing (1-1000); other init settings are not applied by this adapter.
- Large tables virtualize row layout, with estimated heights for unvisited rows.
  Initial parsing, source hashing and height-prefix construction still scale with
  note size; table columns are not virtualized. More memory/latency profiling is open.
- Ordinary note load/save and conflict resolution run off the UI thread. Startup
  path checks/font loading, creating/deleting/moving notes, backlink rewrites,
  recovery discovery/review and preference persistence still include synchronous
  filesystem work. Slow storage can affect these operations. A serialized note
  worker keeps the UI responsive but a slow operation delays later note requests.
- Rich blocks necessarily change height when source opens. Clicked positions and
  visible text are anchored; this does not promise identical geometry for arbitrary
  document replacement, resizing, images that load late, or an anchor at a scroll limit.
- Regex uses Rust regex syntax: no look-around or pattern backreferences. Undo
  history lasts for the current process only; tear-off saves content but starts a
  new process/undo history. Windows coordinate per workspace; they do not share
  in-memory editor state or merge tabs by dragging between existing windows.
- Native setup, associations and update delivery are implemented, but artifacts
  remain unsigned: the only local code-signing certificate is expired. Signing
  requires an externally supplied valid certificate/private key. Release download
  end-to-end cannot be certified until a release asset is published; local update
  application/checksum/backup behavior is tested. See DISTRIBUTION.md.
- Custom MCP remains unimplemented. Sidebar tint approximates translucency.
