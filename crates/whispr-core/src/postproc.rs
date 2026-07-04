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

/// Deterministic list formatting: if the transcript opens with a list intent
/// ("make me a shopping list ...", "create a to-do list ...", "list of ..."),
/// split the items on commas/semicolons/" and " and emit them as `- ` bullet
/// lines. Conservative by design — needs >= 2 items, otherwise the text passes
/// through untouched. Never adds or drops words (unlike a small LLM, measured).
pub fn bulletize(text: &str) -> String {
    let lower = text.to_lowercase();
    let Some(list_pos) = lower.find("list") else {
        return text.to_string();
    };
    // "list" must end at a word boundary ("playlist," ok; "listened" not).
    let after = list_pos + "list".len();
    if lower[after..].chars().next().is_some_and(|c| c.is_alphanumeric()) {
        return text.to_string();
    }
    // Intent must appear early (an opener, not a mention later in the sentence).
    let intro_words = lower[..list_pos].split_whitespace().count();
    if intro_words > 8 {
        return text.to_string();
    }
    let has_verb = ["make", "create", "write", "give", "note", "add", "start"]
        .iter()
        .any(|v| lower[..list_pos].contains(v));
    let is_list_of = lower[list_pos..].starts_with("list of");
    if !has_verb && !is_list_of {
        return text.to_string();
    }

    // Items start after "list" (+ optional "of <noun phrase>" up to a separator).
    let mut items_start = list_pos + "list".len();
    if is_list_of {
        // keep "of <words>" in the intro until the first separator
        let rest = &text[items_start..];
        let sep = rest.find([',', ':', ';']).unwrap_or(0);
        items_start += sep;
    }
    let intro = text[..items_start].trim().trim_end_matches([':', ',', ';']);
    let items_text = text[items_start..].trim_start_matches([':', ',', ';']).trim();
    if items_text.is_empty() {
        return text.to_string();
    }

    let items: Vec<String> = items_text
        .split([',', ';'])
        .flat_map(|chunk| split_on_and(chunk))
        .map(|s| s.trim().trim_end_matches('.').trim().to_string())
        .filter(|s| !s.is_empty())
        .map(capitalize)
        .collect();
    if items.len() < 2 {
        return text.to_string();
    }

    let mut out = String::new();
    out.push_str(intro);
    out.push_str(":\n");
    for item in &items {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Split a chunk on the word "and" (case-insensitive, whole word).
fn split_on_and(chunk: &str) -> Vec<&str> {
    let lower = chunk.to_lowercase();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut search = 0;
    while let Some(rel) = lower[search..].find("and") {
        let start = search + rel;
        let end = start + 3;
        let before_ok = start == 0
            || !lower[..start].chars().next_back().is_some_and(|c| c.is_alphanumeric());
        let after_ok =
            end >= lower.len() || !lower[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
        if before_ok && after_ok {
            parts.push(&chunk[last..start]);
            last = end;
        }
        search = end;
        if search >= lower.len() {
            break;
        }
    }
    parts.push(&chunk[last..]);
    parts
}

fn capitalize(s: String) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
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

    #[test]
    fn bulletize_comma_list() {
        let out = bulletize("Make me a grocery shopping list, eggs, milk, bread and some butter.");
        assert_eq!(
            out,
            "Make me a grocery shopping list:\n- Eggs\n- Milk\n- Bread\n- Some butter"
        );
    }

    #[test]
    fn bulletize_list_of() {
        let out = bulletize("List of things to pack, charger, notebook and headphones.");
        assert_eq!(out, "List of things to pack:\n- Charger\n- Notebook\n- Headphones");
    }

    #[test]
    fn bulletize_leaves_non_lists_alone() {
        let t = "I listened to the new album and liked it.";
        assert_eq!(bulletize(t), t);
        let t2 = "The waiting list is long.";
        assert_eq!(bulletize(t2), t2);
        // list intent but only one item -> unchanged
        let t3 = "Make me a list, groceries.";
        assert_eq!(bulletize(t3), t3);
    }

    #[test]
    fn bulletize_and_only() {
        let out = bulletize("Create a to-do list, finish the report and call mom.");
        assert_eq!(out, "Create a to-do list:\n- Finish the report\n- Call mom");
    }
}
