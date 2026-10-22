/// Normalize common LaTeX delimiters for preview only; saved text is untouched.
pub(super) fn normalize(source: &str) -> String {
    let protected = protected_ranges(source);
    static ENVIRONMENTS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let source = ENVIRONMENTS.get_or_init(|| regex::Regex::new(
        r"(?s)\\begin\{(equation\*?|align\*?|gather\*?|multline\*?|displaymath)\}.*?\\end\{([^}]+)\}"
    ).unwrap()).replace_all(source, |c: &regex::Captures<'_>| {
        let range = c.get(0).unwrap().range();
        if c[1] != c[2] || is_protected(source, &range, &protected) { return c[0].to_owned(); }
        format!("$${}$$", &c[0])
    });
    let source = source.as_ref();
    let protected = protected_ranges(source);
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN
        .get_or_init(|| regex::Regex::new(r"(?s)\\\((.*?)\\\)|\\\[(.*?)\\\]").unwrap())
        .replace_all(source, |c: &regex::Captures<'_>| {
            let range = c.get(0).unwrap().range();
            if is_protected(source, &range, &protected) {
                return c[0].to_string();
            }
            if let Some(body) = c.get(1) {
                format!("${}$", body.as_str())
            } else {
                format!("$${}$$", c.get(2).unwrap().as_str())
            }
        })
        .into_owned()
}
fn protected_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    pulldown_cmark::Parser::new_ext(source, pulldown_cmark::Options::ENABLE_MATH)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(
                event,
                pulldown_cmark::Event::Code(_)
                    | pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(_))
                    | pulldown_cmark::Event::InlineMath(_)
                    | pulldown_cmark::Event::DisplayMath(_)
            )
            .then_some(range)
        })
        .collect()
}
fn is_protected(
    source: &str,
    range: &std::ops::Range<usize>,
    protected: &[std::ops::Range<usize>],
) -> bool {
    let escapes = source[..range.start]
        .bytes()
        .rev()
        .take_while(|b| *b == b'\\')
        .count();
    escapes % 2 == 1
        || protected
            .iter()
            .any(|p| p.start < range.end && range.start < p.end)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bare_environments_render_without_nesting_existing_math_or_code() {
        let environment = r"\begin{align*}a&=b\\c&=d\end{align*}";
        assert_eq!(normalize(environment), format!("$${environment}$$"));
        for source in [
            format!("$${environment}$$"),
            format!("`{environment}`"),
            format!("```tex\n{environment}\n```"),
            r"\begin{align}x\end{equation}".to_owned(),
        ] {
            assert_eq!(normalize(&source), source);
        }
        assert_eq!(normalize(r"$\text{\(literal\)}$"), r"$\text{\(literal\)}$");
    }
    #[test]
    fn latex_delimiters_are_supported_without_rewriting_code() {
        let input =
            r"Inline \(x^2\), display \[\frac{1}{2}\], code `\(literal\)` and \\(escaped\\).";
        let output = normalize(input);
        assert!(output.contains("$x^2$"));
        assert!(output.contains(r"$$\frac{1}{2}$$"));
        assert!(output.contains(r"`\(literal\)`"));
        assert!(output.contains(r"\\(escaped\\)"));
        assert_eq!(
            normalize("```tex\n\\[raw\\]\n```"),
            "```tex\n\\[raw\\]\n```"
        );
    }
}
