//! Post-processing: the personal dictionary — deterministic, user-controlled
//! corrections applied after ASR. This is how proper nouns (names, product
//! names, jargon) get fixed; no acoustic model can spell "whispr" from audio.
//!
//! Dictionary file format, one rule per line:
//! ```text
//! # comment
//! varum => Varun
//! java script => JavaScript
//! ```
//! Matching is case-insensitive on whole words (multi-word phrases allowed);
//! the replacement is inserted verbatim.

use std::path::Path;

use anyhow::{Context, Result};

pub struct Dictionary {
    rules: Vec<(String, String)>, // (lowercased needle, replacement)
}

impl Dictionary {
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read dictionary {}", path.display()))?;
        let mut rules = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((from, to)) = line.split_once("=>") {
                let from = from.trim().to_lowercase();
                let to = to.trim().to_string();
                if !from.is_empty() && !to.is_empty() {
                    rules.push((from, to));
                }
            }
        }
        // Longest needle first so "java script" wins over a hypothetical "java".
        rules.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        Ok(Self { rules })
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (from, to) in &self.rules {
            out = replace_word_ci(&out, from, to);
        }
        out
    }
}

/// Case-insensitive whole-word replace ("word" boundaries = non-alphanumeric).
fn replace_word_ci(text: &str, needle_lower: &str, replacement: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut search = 0;
    while let Some(rel) = lower[search..].find(needle_lower) {
        let start = search + rel;
        let end = start + needle_lower.len();
        let boundary_before = start == 0
            || !lower[..start].chars().next_back().is_some_and(|c| c.is_alphanumeric());
        let boundary_after =
            end >= lower.len() || !lower[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
        if boundary_before && boundary_after {
            out.push_str(&text[last..start]);
            out.push_str(replacement);
            last = end;
            search = end;
        } else {
            search = end;
        }
        if search >= lower.len() {
            break;
        }
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(rules: &[(&str, &str)]) -> Dictionary {
        let mut d = Dictionary::empty();
        for (f, t) in rules {
            d.rules.push((f.to_lowercase(), t.to_string()));
        }
        d.rules.sort_by_key(|(f, _)| std::cmp::Reverse(f.len()));
        d
    }

    #[test]
    fn whole_word_only() {
        let d = dict(&[("gita", "GitHub")]);
        assert_eq!(d.apply("Gita is great"), "GitHub is great");
        assert_eq!(d.apply("digital"), "digital"); // no substring hits
    }

    #[test]
    fn multi_word_and_case() {
        let d = dict(&[("java script", "JavaScript"), ("varum", "Varun")]);
        assert_eq!(d.apply("I like Java Script, Varum."), "I like JavaScript, Varun.");
    }

    #[test]
    fn punctuation_boundaries() {
        let d = dict(&[("varum", "Varun")]);
        assert_eq!(d.apply("Hi, varum!"), "Hi, Varun!");
    }
}
