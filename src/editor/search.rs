use std::ops::Range;

#[derive(Default)]
pub struct Find {
    pub open: bool,
    pub replace_open: bool,
    pub query: String,
    pub replacement: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub error: Option<String>,
    pub current: Option<usize>,
    pub matches: Vec<Range<usize>>,
    pub focus_query: bool,
    cached_text: String,
    cached_options: Option<(String, bool, bool, bool)>,
}

impl Find {
    pub fn refresh(&mut self, text: &str) -> bool {
        let options = (
            self.query.clone(),
            self.match_case,
            self.whole_word,
            self.regex,
        );
        if self.cached_options.as_ref() == Some(&options) && self.cached_text == text {
            return false;
        }
        self.cached_options = Some(options);
        self.cached_text = text.to_owned();
        self.error = None;
        self.matches = match self.expression() {
            Ok(Some(re)) => re.find_iter(text).map(|m| m.range()).collect(),
            Ok(None) => vec![],
            Err(error) => {
                self.error = Some(error.to_string());
                vec![]
            }
        };
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(self.current.unwrap_or(0).min(self.matches.len() - 1))
        };
        true
    }
    fn expression(&self) -> Result<Option<regex::Regex>, regex::Error> {
        if self.query.is_empty() {
            return Ok(None);
        }
        let pattern = if self.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        let pattern = if self.whole_word {
            format!(r"\b(?:{pattern})\b")
        } else {
            pattern
        };
        regex::RegexBuilder::new(&pattern)
            .case_insensitive(!self.match_case)
            .size_limit(2 * 1024 * 1024)
            .build()
            .map(Some)
    }
    pub fn active(&self) -> Option<Range<usize>> {
        self.current.and_then(|n| self.matches.get(n)).cloned()
    }
    pub fn next(&mut self, forward: bool) -> Option<Range<usize>> {
        let n = self.matches.len();
        if n == 0 {
            self.current = None;
            return None;
        }
        self.current = Some(match (self.current, forward) {
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
            (None, true) => 0,
            (None, false) => n - 1,
        });
        self.active()
    }
    pub fn select_from(&mut self, byte: usize) -> Option<Range<usize>> {
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(
                self.matches
                    .iter()
                    .position(|r| r.start >= byte)
                    .unwrap_or(0),
            )
        };
        self.active()
    }
    pub fn replace_one(&mut self, text: &mut String) -> bool {
        self.refresh(text);
        let Some(range) = self.active() else {
            return false;
        };
        let replacement = if self.regex {
            self.expression()
                .ok()
                .flatten()
                .and_then(|re| re.captures_at(text, range.start))
                .filter(|caps| caps.get(0).is_some_and(|m| m.range() == range))
                .map(|caps| {
                    let mut out = String::new();
                    caps.expand(&self.replacement, &mut out);
                    out
                })
                .unwrap_or_else(|| self.replacement.clone())
        } else {
            self.replacement.clone()
        };
        let mut next = range.start + replacement.len();
        let empty = range.is_empty();
        text.replace_range(range, &replacement);
        if empty {
            next += text[next..].chars().next().map_or(1, char::len_utf8);
        }
        self.refresh(text);
        self.select_from(next);
        true
    }
    pub fn replace_all(&mut self, text: &mut String) -> usize {
        self.refresh(text);
        let count = self.matches.len();
        if self.regex {
            if let Ok(Some(re)) = self.expression() {
                *text = re.replace_all(text, self.replacement.as_str()).into_owned();
            }
        } else {
            for range in self.matches.iter().rev() {
                text.replace_range(range.clone(), &self.replacement);
            }
        }
        self.current = None;
        self.refresh(text);
        count
    }
}

pub(super) fn matches(
    text: &str,
    query: &str,
    match_case: bool,
    whole_word: bool,
) -> Vec<Range<usize>> {
    if query.is_empty() {
        return vec![];
    }
    let literal = regex::escape(query);
    let pattern = if whole_word {
        format!(r"\b(?:{literal})\b")
    } else {
        literal
    };
    regex::RegexBuilder::new(&pattern)
        .case_insensitive(!match_case)
        .build()
        .map_or_else(
            |_| vec![],
            |re| re.find_iter(text).map(|m| m.range()).collect(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regex_captures_invalid_patterns_and_zero_width_matches() {
        let mut find = Find {
            regex: true,
            query: r"(?P<word>\p{L}+)-(\d+)".into(),
            replacement: "${word}:$2".into(),
            ..Default::default()
        };
        let mut text = "café-12 note-3".to_owned();
        assert_eq!(find.replace_all(&mut text), 2);
        assert_eq!(text, "café:12 note:3");
        find.query = "[".into();
        find.refresh(&text);
        assert!(find.error.is_some() && find.matches.is_empty());
        assert!(!find.replace_one(&mut text));
        find.query = "(?m)^".into();
        find.replacement = "> ".into();
        text = "one\ntwo".into();
        assert_eq!(find.replace_all(&mut text), 2);
        assert_eq!(text, "> one\n> two");
        find.query = r"(one)".into();
        find.replacement = "$1!".into();
        assert!(find.replace_one(&mut text));
        assert_eq!(text, "> one!\n> two");
    }
    #[test]
    fn unicode_case_and_whole_words_preserve_byte_ranges() {
        let text = "CAFÉ café cafétéria 🦀";
        let ranges = matches(text, "café", false, true);
        assert_eq!(ranges.len(), 2);
        assert_eq!(&text[ranges[0].clone()], "CAFÉ");
        assert_eq!(&text[ranges[1].clone()], "café");
        assert_eq!(matches(text, "café", true, true).len(), 1);
        assert_eq!(matches(text, "café", false, false).len(), 3);
        assert_eq!(matches(text, "🦀", false, false).len(), 1);
    }
    #[test]
    fn literal_replacement_and_wraparound() {
        let mut find = Find {
            query: "one".into(),
            replacement: "$1\\two".into(),
            ..Default::default()
        };
        let mut text = "one ONE one".to_owned();
        find.refresh(&text);
        assert_eq!(find.next(false), Some(8..11));
        assert_eq!(find.next(true), Some(0..3));
        assert_eq!(find.replace_all(&mut text), 3);
        assert_eq!(text, "$1\\two $1\\two $1\\two");
        assert!(find.matches.is_empty());
    }
    #[test]
    fn replace_skips_inserted_matches_and_empty_query_is_safe() {
        let mut find = Find {
            query: "a".into(),
            replacement: "aa".into(),
            ..Default::default()
        };
        let mut text = "a a".into();
        assert!(find.replace_one(&mut text));
        assert_eq!(text, "aa a");
        assert_eq!(find.active(), Some(3..4));
        find.query.clear();
        assert_eq!(find.replace_all(&mut text), 0);
        assert_eq!(text, "aa a");
    }
    #[test]
    fn changing_options_and_empty_replacement() {
        let mut find = Find {
            query: "foo".into(),
            ..Default::default()
        };
        let mut text = "foo Foo food".to_owned();
        find.refresh(&text);
        assert_eq!(find.matches.len(), 3);
        find.whole_word = true;
        find.refresh(&text);
        assert_eq!(find.matches.len(), 2);
        assert_eq!(find.replace_all(&mut text), 2);
        assert_eq!(text, "  food");
    }
}
