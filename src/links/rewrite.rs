use super::*;
use pulldown_cmark::{Event, LinkType, Parser, Tag};
use std::ops::Range;

pub fn moved(path: &Path, old: &Path, new: &Path) -> PathBuf {
    path.strip_prefix(old).map_or_else(
        |_| path.to_owned(),
        |suffix| {
            if suffix.as_os_str().is_empty() {
                new.to_owned()
            } else {
                new.join(suffix)
            }
        },
    )
}
fn relative(base: &Path, target: &Path) -> String {
    let a: Vec<_> = base.components().collect();
    let b: Vec<_> = target.components().collect();
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut parts = vec!["..".to_string(); a.len() - common];
    parts.extend(
        b[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}
fn encode(path: &str, wiki: bool) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) || wiki && b == b' ' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
// Return only the URL, leaving angle brackets, titles, labels and whitespace alone.
fn destination(text: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut start = start;
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    if start >= end {
        return None;
    }
    if bytes[start] == b'<' {
        return Some(start + 1..start + 1 + text[start + 1..end].find('>')?);
    }
    let mut at = start;
    let mut nesting = 0;
    while at < end {
        match bytes[at] {
            b'\\' if at + 1 < end => {
                at += 2;
                continue;
            }
            b'(' => nesting += 1,
            b')' if nesting == 0 => break,
            b')' => nesting -= 1,
            b if b.is_ascii_whitespace() => break,
            _ => {}
        }
        at += 1;
    }
    (start < at).then_some(start..at)
}
pub fn rewrite(
    text: &str,
    note: &Path,
    root: &Path,
    paths: &[PathBuf],
    old: &Path,
    new: &Path,
) -> String {
    let after = moved(note, old, new);
    let replace = |url: &str, wiki: bool, image: bool| -> Option<String> {
        if url.is_empty()
            || url.starts_with('#')
            || url.contains("://")
            || url.starts_with("mailto:")
            || url.starts_with("data:")
        {
            return None;
        }
        let (path, fragment) = url
            .split_once('#')
            .map_or((url, None), |(a, b)| (a, Some(b)));
        let resolved = resolve_indexed(root, note, path, !image, paths)?;
        let target = moved(&resolved, old, new);
        if target == resolved && note == after {
            return None;
        }
        let mut value = encode(&relative(after.parent().unwrap_or(root), &target), wiki);
        if let Some(fragment) = fragment {
            value.push('#');
            value.push_str(fragment);
        }
        Some(value)
    };
    let mut edits = Vec::<(Range<usize>, String)>::new();
    let parser = Parser::new(text);
    for (_, definition) in parser.reference_definitions().iter() {
        let span = definition.span.clone();
        if let Some(colon) = text[span.clone()].find("]:")
            && let Some(range) = destination(text, span.start + colon + 2, span.end)
            && let Some(value) = replace(&definition.dest, false, true)
        {
            edits.push((range, value));
        }
    }
    let mut protected = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => protected.push(range),
            Event::Start(Tag::Link {
                link_type: LinkType::Inline,
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type: LinkType::Inline,
                dest_url,
                ..
            }) => {
                let image = text[range.clone()].starts_with('!');
                if let Some(value) = replace(&dest_url, false, image)
                    && let Some(at) = text[range.clone()].rfind("](")
                    && let Some(dest) = destination(text, range.start + at + 2, range.end)
                {
                    edits.push((dest, value));
                }
            }
            _ => {}
        }
    }
    static WIKI: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    for captures in WIKI
        .get_or_init(|| regex::Regex::new(r"(!?)\[\[([^\]\n]+)\]\]").unwrap())
        .captures_iter(text)
    {
        let whole = captures.get(0).unwrap();
        if protected.iter().any(|range| range.contains(&whole.start())) {
            continue;
        }
        let inner = captures.get(2).unwrap();
        let target = inner
            .as_str()
            .split('|')
            .next()
            .unwrap()
            .trim_end_matches('\\');
        if let Some(value) = replace(target, true, !captures[1].is_empty()) {
            edits.push((inner.start()..inner.start() + target.len(), value));
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut result = text.to_owned();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range, &value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewrites_links_references_wiki_and_images_but_not_code_or_web_urls() {
        let root = Path::new("/vault");
        let note = root.join("Reader.md");
        let old = root.join("Folder");
        let new = root.join("New Folder");
        let paths = vec![note.clone(), old.join("Target.md"), old.join("image.png")];
        let source = "[[Folder/Target#part|Alias]] ![[Folder/image.png]]\n[Note](<Folder/Target.md#part> \"Title\") ![Pic](Folder/image.png)\n[Ref][id]\n\n[id]: Folder/Target.md \"Title\"\n\n`[[Folder/Target]]`\n```md\n[no](Folder/Target.md)\n```\n[web](https://example.com/Folder/Target.md)";
        let result = rewrite(source, &note, root, &paths, &old, &new);
        assert!(result.contains("[[New Folder/Target.md#part|Alias]]"));
        assert!(result.contains("![[New Folder/image.png]]"));
        assert!(result.contains("[Note](<New%20Folder/Target.md#part> \"Title\")"));
        assert!(result.contains("![Pic](New%20Folder/image.png)"));
        assert!(result.contains("[id]: New%20Folder/Target.md \"Title\""));
        assert!(result.contains("`[[Folder/Target]]`"));
        assert!(result.contains("[no](Folder/Target.md)"));
        assert!(result.contains("https://example.com/Folder/Target.md"));
    }
    #[test]
    fn moved_notes_keep_relative_links_to_unmoved_assets() {
        let root = Path::new("/vault");
        let old = root.join("Note.md");
        let new = root.join("Archive/Note.md");
        let paths = vec![old.clone(), root.join("picture.png"), root.join("Other.md")];
        assert_eq!(
            rewrite(
                "![p](picture.png) [[Other]] [local](#part)",
                &old,
                root,
                &paths,
                &old,
                &new
            ),
            "![p](../picture.png) [[../Other.md]] [local](#part)"
        );
    }
}
