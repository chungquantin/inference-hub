//! Tags news items with the organizations they mention, by matching names.
//!
//! The rules are deliberately simple and predictable: an organization matches
//! when its name or one of its aliases appears as a whole word. Names that are
//! also ordinary words ("Meta", "Lambda") are matched case-sensitively, and
//! exclude phrases ("AWS Lambda") are blanked out before matching.

use uuid::Uuid;

/// Where a mention was found. Title mentions are the strong signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    Title,
    Body,
}

/// The matching rules of one organization.
#[derive(Debug, Clone)]
pub struct OrgRule {
    pub id: Uuid,
    /// The name and aliases.
    pub terms: Vec<String>,
    pub exclude_phrases: Vec<String>,
    pub match_case: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Tagger {
    rules: Vec<OrgRule>,
}

impl Tagger {
    pub fn new(rules: Vec<OrgRule>) -> Self {
        let rules = rules
            .into_iter()
            .map(|mut r| {
                r.terms.retain(|t| !t.trim().is_empty());
                r
            })
            .filter(|r| !r.terms.is_empty())
            .collect();
        Self { rules }
    }

    /// The organizations mentioned in an item, each with the strongest place it was found.
    pub fn tag(&self, title: &str, body: &str) -> Vec<(Uuid, MatchKind)> {
        self.rules
            .iter()
            .filter_map(|rule| {
                if rule.mentioned_in(title) {
                    Some((rule.id, MatchKind::Title))
                } else if rule.mentioned_in(body) {
                    Some((rule.id, MatchKind::Body))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl OrgRule {
    fn mentioned_in(&self, text: &str) -> bool {
        let text = blank_out(text, &self.exclude_phrases);
        let haystack = if self.match_case {
            text
        } else {
            text.to_ascii_lowercase()
        };
        self.terms.iter().any(|term| {
            let needle = if self.match_case {
                term.clone()
            } else {
                term.to_ascii_lowercase()
            };
            contains_word(&haystack, &needle)
        })
    }
}

/// Whether `needle` appears in `haystack` with no letter or digit directly around it.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        // Advance by one character, staying on a char boundary.
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Replaces every case-insensitive occurrence of the phrases with spaces.
fn blank_out(text: &str, phrases: &[String]) -> String {
    let mut out = text.to_owned();
    for phrase in phrases.iter().filter(|p| !p.is_empty()) {
        let lower_phrase = phrase.to_ascii_lowercase();
        loop {
            let lower = out.to_ascii_lowercase();
            let Some(pos) = lower.find(&lower_phrase) else {
                break;
            };
            out.replace_range(
                pos..pos + lower_phrase.len(),
                &" ".repeat(lower_phrase.len()),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(n: u128, terms: &[&str], exclude: &[&str], match_case: bool) -> OrgRule {
        OrgRule {
            id: Uuid::from_u128(n),
            terms: terms.iter().map(|s| s.to_string()).collect(),
            exclude_phrases: exclude.iter().map(|s| s.to_string()).collect(),
            match_case,
        }
    }

    fn tagger() -> Tagger {
        Tagger::new(vec![
            rule(1, &["OpenAI"], &[], false),
            rule(2, &["Meta", "Meta AI"], &["metadata"], true),
            rule(
                3,
                &["Lambda", "Lambda Labs"],
                &["AWS Lambda", "lambda function"],
                true,
            ),
            rule(
                4,
                &["Amazon Web Services", "AWS", "Amazon Bedrock"],
                &[],
                false,
            ),
            rule(5, &["vLLM"], &[], false),
            rule(6, &["", "  "], &[], false),
        ])
    }

    fn ids(found: &[(Uuid, MatchKind)]) -> Vec<u128> {
        found.iter().map(|(id, _)| id.as_u128()).collect()
    }

    #[test]
    fn matches_whole_words_case_insensitively_by_default() {
        let t = tagger();
        assert_eq!(ids(&t.tag("openai ships a model", "")), [1]);
        assert_eq!(ids(&t.tag("Serving with vLLM, fast", "")), [5]);
        assert_eq!(ids(&t.tag("OpenAIish startup", "")), Vec::<u128>::new());
        assert_eq!(ids(&t.tag("The SAWS project", "")), Vec::<u128>::new());
    }

    #[test]
    fn case_sensitive_names_ignore_common_words() {
        let t = tagger();
        assert_eq!(ids(&t.tag("Meta releases Llama", "")), [2]);
        assert_eq!(
            ids(&t.tag("meta thoughts on scaling", "")),
            Vec::<u128>::new()
        );
        assert_eq!(ids(&t.tag("Lambda cuts H100 prices", "")), [3]);
        assert_eq!(ids(&t.tag("the lambda calculus", "")), Vec::<u128>::new());
    }

    #[test]
    fn exclude_phrases_suppress_false_positives_but_keep_real_mentions() {
        let t = tagger();
        assert_eq!(ids(&t.tag("Running models on AWS Lambda", "")), [4]);
        assert_eq!(ids(&t.tag("aws lambda and Lambda GPU cloud", "")), [3, 4]);
    }

    #[test]
    fn title_mentions_outrank_body_mentions() {
        let t = tagger();
        let found = t.tag("OpenAI news", "Also available on AWS.");
        assert_eq!(
            found,
            [
                (Uuid::from_u128(1), MatchKind::Title),
                (Uuid::from_u128(4), MatchKind::Body)
            ]
        );
    }

    #[test]
    fn handles_non_ascii_text_and_punctuation() {
        let t = tagger();
        assert_eq!(ids(&t.tag("«OpenAI» — déjà vu", "")), [1]);
        assert_eq!(ids(&t.tag("Ünïcode only", "")), Vec::<u128>::new());
        assert!(contains_word("é OpenAI", "OpenAI"));
        assert!(!contains_word("éOpenAI", "OpenAI"));
    }
}
