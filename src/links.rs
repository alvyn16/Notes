use std::path::{Path, PathBuf};
pub mod rewrite;

pub fn decode(text: &str) -> String {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(s) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(value) = u8::from_str_radix(s, 16)
        {
            out.push(value);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
#[cfg(test)]
pub fn resolve(root: &Path, note: &Path, target: &str, notes: bool) -> Option<PathBuf> {
    resolve_indexed(
        root,
        note,
        target,
        notes,
        &crate::workspace::inventory(root).1,
    )
}
pub fn resolve_indexed(
    root: &Path,
    note: &Path,
    target: &str,
    notes: bool,
    paths: &[PathBuf],
) -> Option<PathBuf> {
    let target = decode(
        target
            .trim_matches(['<', '>'])
            .trim_start_matches("wiki:")
            .split('#')
            .next()
            .unwrap_or(""),
    )
    .replace('\\', "/");
    let target = target.split('|').next().unwrap_or(&target);
    if target.is_empty() {
        return Some(note.to_owned());
    }
    if target.contains("://") {
        return None;
    }
    for base in [note.parent().unwrap_or(root), root] {
        let path = base.join(target);
        for candidate in [
            path.clone(),
            path.with_extension("md"),
            path.with_extension("markdown"),
        ] {
            let mut normalized = PathBuf::new();
            for component in candidate.components() {
                match component {
                    std::path::Component::ParentDir => {
                        normalized.pop();
                    }
                    std::path::Component::CurDir => {}
                    _ => normalized.push(component.as_os_str()),
                }
            }
            if let Some(path) = paths.iter().find(|path| {
                path.to_string_lossy()
                    .eq_ignore_ascii_case(&normalized.to_string_lossy())
            }) && (!notes || crate::workspace::is_markdown(path))
            {
                return Some(path.clone());
            }
            if !notes {
                break;
            }
        }
    }
    let wanted = Path::new(target)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    let mut matches: Vec<_> = paths
        .iter()
        .filter(|path| !notes || crate::workspace::is_markdown(path))
        .filter(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                == wanted
                || (notes
                    && path
                        .file_stem()
                        .is_some_and(|s| s.to_string_lossy().to_lowercase() == wanted))
        })
        .cloned()
        .collect();
    matches.sort_by_key(|path| (path.components().count(), path.clone()));
    matches.into_iter().next()
}
pub fn wiki_markdown(text: &str) -> String {
    // Inline rendering only: saved source is never rewritten.
    let re = regex::Regex::new(r"(!?)\[\[([^\]\n]+)\]\]").expect("wiki pattern");
    let protected: Vec<_> = pulldown_cmark::Parser::new(text)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(
                event,
                pulldown_cmark::Event::Code(_)
                    | pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(_))
            )
            .then_some(range)
        })
        .collect();
    re.replace_all(text, |c: &regex::Captures<'_>| {
        if protected
            .iter()
            .any(|range| range.contains(&c.get(0).unwrap().start()))
        {
            return c[0].to_owned();
        }
        let value = c[2].replace("\\|", "|");
        let (target, label) = value.split_once('|').unwrap_or((&value, &value));
        format!("{}[{}](<wiki:{}>)", &c[1], label, target)
    })
    .into_owned()
}
pub fn heading_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect()
}
pub fn heading_offset(text: &str, anchor: &str) -> Option<usize> {
    let wanted = decode(anchor).to_lowercase();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for (event, range) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        if matches!(
            event,
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { .. })
        ) {
            let heading: String = pulldown_cmark::Parser::new(&text[range.clone()])
                .filter_map(|e| match e {
                    pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                        Some(t.to_string())
                    }
                    _ => None,
                })
                .collect();
            let slug = heading_slug(&heading);
            let number = seen.entry(slug.clone()).or_default();
            let unique = if *number == 0 {
                slug
            } else {
                format!("{slug}-{number}")
            };
            *number += 1;
            if unique == wanted || heading.to_lowercase() == wanted {
                return Some(range.start);
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_wiki_aliases_images_and_duplicate_headings() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("sub")).unwrap();
        std::fs::write(root.path().join("sub/Other Note.md"), "note").unwrap();
        std::fs::write(root.path().join("sub/image.png"), "image").unwrap();
        let note = root.path().join("note.md");
        assert!(
            resolve(root.path(), &note, "Other%20Note", true)
                .unwrap()
                .ends_with("Other Note.md")
        );
        assert!(
            resolve(root.path(), &note, "wiki:image.png", false)
                .unwrap()
                .ends_with("image.png")
        );
        assert_eq!(
            wiki_markdown("[[Other Note|Alias]]"),
            "[Alias](<wiki:Other Note>)"
        );
        let text = "# Héllo\n\n## Topic\n\n## Topic\n";
        assert_eq!(
            heading_offset(text, "topic-1"),
            Some(text.rfind("## Topic").unwrap())
        );
    }
}
