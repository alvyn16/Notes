use std::{fs, io, path::Path};

pub fn seed(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    for folder in [
        "Archive",
        "Areas",
        "Daily Notes",
        "Projects/CLI Tool",
        "Projects/Website Redesign",
        "Resources",
        "Templates",
    ] {
        fs::create_dir_all(root.join(folder))?;
    }
    let notes = [
        ("Daily Notes/Monday, April 28, 2026.md", DAILY),
        (
            "Daily Notes/Tuesday, April 29, 2026.md",
            "# Tuesday, April 29, 2026\n\n## Morning Pages\n\nA little space to think clearly.\n\n## Tasks\n\n- [ ] Keep writing\n",
        ),
        (
            "Daily Notes/Wednesday, April 30, 2026.md",
            "# Wednesday, April 30, 2026\n\n## Morning Pages\n\nStart with one good idea.\n",
        ),
        ("Projects/Website Redesign/Website Redesign.md", WEBSITE),
        (
            "Projects/Website Redesign/Content Migration Plan.md",
            MIGRATION,
        ),
        (
            "Projects/Website Redesign/Accessibility Audit.md",
            "# Accessibility Audit\n\n- [ ] Keyboard navigation\n- [ ] Screen reader labels\n- [x] Color contrast\n",
        ),
        (
            "Projects/Website Redesign/Design System.md",
            "# Design System\n\n## Principles\n\nQuiet surfaces. Clear typography. One warm accent.\n",
        ),
        (
            "Projects/Website Redesign/Tech Stack Decisions.md",
            "# Tech Stack Decisions\n\n## Open Questions\n\nWhich headless CMS? Evaluate content models and preview support.\n",
        ),
        (
            "Projects/CLI Tool/CLI Tool.md",
            "# CLI Tool\n\nA small tool with a composable [architecture](Architecture Notes.md).\n\n- [ ] Design the command interface\n- [x] Write the project brief\n",
        ),
        (
            "Projects/CLI Tool/Architecture Notes.md",
            "# Architecture Notes\n\nUse a plugin system instead of hardcoding commands.\n\n```rust\ntrait Command {\n    fn run(&self) -> Result<(), String>;\n}\n```\n",
        ),
        ("Resources/Mermaid Diagram Test.md", DIAGRAMS),
        (
            "Resources/Workout Log.md",
            "# Workout Log\n\n- [x] Monday: 30 minute run\n- [ ] Wednesday: strength training\n",
        ),
        (
            "Resources/Async Patterns.md",
            "# Async Patterns\n\n`Promise.allSettled()` waits for every result, including rejections.\n",
        ),
        (
            "Resources/DDIA.md",
            "# DDIA\n\n## Reading Notes\n\nChapter 5: Replication\n",
        ),
        (
            "Home.md",
            "# Home\n\nWelcome to your writing space.\n\nUse **Ctrl+P** to find a note or run a command. Click any paragraph to edit it. Press **F4** for the complete Markdown source.\n\nYour notes are ordinary files. Open your own folder from **Library**.\n",
        ),
    ];
    for (name, text) in notes {
        let path = root.join(name);
        if !path.exists() {
            fs::write(path, text)?;
        }
    }
    Ok(())
}

const DAILY: &str = "# Monday, April 28, 2026\n\n## Morning Pages\n\nWoke up early today. Clear head. Want to focus on the [website redesign](../Projects/Website%20Redesign/Website%20Redesign.md) — specifically the navigation component.\n\n## Tasks\n\n- [x] Review PR for auth module\n- [x] Respond to design feedback on [design system](../Projects/Website%20Redesign/Design%20System.md)\n- [ ] Read chapter 5 of [DDIA](../Resources/DDIA.md)\n- [x] 30 min run — see [workout log](../Resources/Workout%20Log.md)\n\n## Notes\n\nHad a good conversation with Sarah about the [CLI tool](../Projects/CLI%20Tool/CLI%20Tool.md) architecture. She suggested using a plugin system instead of hardcoding commands. That's a much better approach — see [architecture notes](../Projects/CLI%20Tool/Architecture%20Notes.md).\n\n#til — Learned about `Promise.allSettled()` vs `Promise.all()`. The former doesn't short-circuit on rejection. Filed under [async patterns](../Resources/Async%20Patterns.md).\n\n## Gratitude\n\n- Good weather for the run\n- Time to focus on meaningful work\n";

const MIGRATION: &str = "# Content Migration Plan\n\n#project\n\nPart of the [Website Redesign](Website%20Redesign.md) project.\n\n## Scope\n\n- 237 blog posts (WordPress → Sanity)\n- 42 landing pages (manual rebuild)\n- 15 case studies (redesign + migrate)\n- Asset library (~2GB of images and PDFs)\n\n## Strategy\n\n### Phase 1: Audit (Week 1)\n\n- Export all WordPress content via WP REST API\n- Categorize posts: keep, update, archive, delete\n- Map WordPress categories/tags to new taxonomy\n\n### Phase 2: Transform (Week 2)\n\n- Convert HTML to portable text (Sanity's format)\n- Clean up legacy shortcodes and embeds\n- Resize and optimize images (WebP + AVIF)\n\n### Phase 3: Validate (Week 3)\n\n- [ ] Verify redirects\n- [ ] Review migrated content\n";

const WEBSITE: &str = "# Website Redesign\n\nA faster, clearer home for our work.\n\n## Project Documents\n\n- [Content Migration Plan](Content%20Migration%20Plan.md)\n- [Design System](Design%20System.md)\n\n## Timeline\n\n| Phase | Dates | Status |\n| --- | --- | --- |\n| Discovery & Research | Mar 15 – Mar 31 | Done |\n| Design System | Apr 1 – Apr 15 | Done |\n| Mockups & Prototypes | Apr 15 – Apr 30 | In Progress |\n| Development | May 1 – May 20 | Not Started |\n| QA & Testing | May 20 – May 28 | Not Started |\n| Launch | Jun 1 | Not Started |\n\n## Open Questions\n\n- [x] Which headless CMS? Leaning toward Sanity — see [tech stack notes](Tech%20Stack%20Decisions.md)\n- [ ] Do we need i18n from day one or can it wait?\n- [ ] Blog migration strategy — 200+ posts to move\n\n## Related\n\n- [DDIA](../../Resources/DDIA.md) — relevant for the API layer decisions\n- [Learning Roadmap](../../Home.md) — Next.js deep dive\n";

const DIAGRAMS: &str = "# Mermaid Diagram Test\n\n## Entity relationship\n\n```mermaid\nerDiagram\n    FILE ||--o{ TAB : opens\n    FILE {\n        string path\n        string content\n    }\n    TAB {\n        string id\n        int cursorPos\n    }\n```\n\n## Gantt chart\n\n```mermaid\ngantt\n    title Feature rollout\n    dateFormat YYYY-MM-DD\n    section Planning\n    Research :a1, 2026-04-01, 8d\n    Design :a2, after a1, 6d\n    section Development\n    Implementation :a3, after a2, 12d\n    Testing :a4, after a3, 5d\n```\n\n## Flowchart\n\n```mermaid\nflowchart TD\n    A[Write a note] --> B[Save to disk]\n    B --> C[Keep thinking]\n```\n";
