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

/// Decide whether a transcript would benefit from the (slow) LLM rephrase pass.
/// Conservative: clean speech goes down the instant path; only visible mess
/// (fillers, stutters, a list the rule-based splitter can't segment) pays the
/// LLM latency. Plain questions stay on the fast path on purpose — the small
/// LLM sometimes answers them instead of transcribing.
pub fn needs_rephrase(text: &str) -> bool {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect();

    // Filler words / verbal tics.
    const FILLERS: &[&str] = &["um", "uh", "uhm", "hmm", "basically", "actually", "literally"];
    let filler_hits = words.iter().filter(|w| FILLERS.contains(*w)).count();
    if filler_hits >= 1 && words.len() >= 6 {
        return true;
    }
    if lower.contains("you know") || lower.contains("i mean") || lower.contains("kind of like") {
        return true;
    }

    // Stutters: the same word twice in a row ("the the report").
    if words.windows(2).any(|p| p[0] == p[1] && p[0].len() > 1) {
        return true;
    }

    // Subject-verb agreement slips ("i is", "they was", "he don't", ...).
    // Word pairs that are wrong in (nearly) any context; false positives just
    // cost one LLM pass, false negatives paste bad grammar - so lean inclusive.
    const BAD_BIGRAMS: &[(&str, &str)] = &[
        ("i", "is"), ("i", "are"), ("i", "has"), ("i", "does"), ("i", "be"),
        ("he", "are"), ("he", "have"), ("he", "do"), ("he", "don't"), ("he", "were"),
        ("she", "are"), ("she", "have"), ("she", "do"), ("she", "don't"), ("she", "were"),
        ("it", "are"), ("it", "have"), ("it", "don't"), ("it", "were"),
        ("we", "is"), ("we", "was"), ("we", "has"), ("we", "does"),
        ("they", "is"), ("they", "was"), ("they", "has"), ("they", "does"),
        ("you", "is"), ("you", "was"), ("you", "has"), ("you", "does"),
        ("there", "be"), ("them", "is"), ("them", "was"),
    ];
    if words
        .windows(2)
        .any(|p| BAD_BIGRAMS.iter().any(|(a, b)| p[0] == *a && p[1] == *b))
    {
        return true;
    }

    // List intent that the rule-based splitter couldn't segment (run-on list).
    if has_list_intent(&lower) && bulletize(text) == text {
        return true;
    }

    false
}

fn has_list_intent(lower: &str) -> bool {
    if !contains_word(lower, "list") {
        return false;
    }
    lower.contains("list of")
        || ["make", "create", "write", "give", "note", "add", "start", "want", "need"]
            .iter()
            .any(|v| contains_word(lower, v))
}

fn contains_word(lower: &str, word: &str) -> bool {
    let mut search = 0;
    while let Some(rel) = lower[search..].find(word) {
        let start = search + rel;
        let end = start + word.len();
        let before = start == 0
            || !lower[..start].chars().next_back().is_some_and(|c| c.is_alphanumeric());
        let after =
            end >= lower.len() || !lower[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
        if before && after {
            return true;
        }
        search = end;
        if search >= lower.len() {
            break;
        }
    }
    false
}

/// Deterministic list formatting. Finds an enumeration anywhere in the
/// transcript — the text after the last colon, or after the last "anchor" word
/// people use when dictating lists ("...list,", "should have", "add",
/// "the following", "need", "buy") — and, if it splits into short
/// comma/"and"-separated items, keeps the spoken preamble and bullets the
/// items. Conservative: >= 2 items, every item <= 5 words, otherwise the text
/// passes through untouched. Never adds or drops words (unlike a small LLM,
/// measured).
pub fn bulletize(text: &str) -> String {
    let lower = text.to_lowercase();
    // Cheap gate: dictated lists mention "list" or use an explicit colon.
    if !contains_word(&lower, "list") && !lower.contains(':') {
        return text.to_string();
    }

    // Candidate split point: last colon, or the end of the last anchor word.
    const ANCHORS: &[&str] = &[
        "list", "have", "has", "add", "include", "buy", "get", "need", "needs", "following",
        "pack", "bring",
    ];
    // (a colon inside a time like "6:30" doesn't count)
    let mut anchor_end: Option<usize> = lower
        .rfind(':')
        .filter(|&p| !lower[p + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|p| p + 1);
    for a in ANCHORS {
        let mut search = 0;
        while let Some(rel) = lower[search..].find(a) {
            let start = search + rel;
            let end = start + a.len();
            let before = start == 0
                || !lower[..start].chars().next_back().is_some_and(|c| c.is_alphanumeric());
            let after = end >= lower.len()
                || !lower[end..].chars().next().is_some_and(|c| c.is_alphanumeric());
            if before && after && end > anchor_end.unwrap_or(0) {
                // Anchor must actually be followed by material to enumerate.
                if lower[end..].trim_start_matches([':', ',', ';', ' ']).len() > 2 {
                    anchor_end = Some(end);
                }
            }
            search = end;
            if search >= lower.len() {
                break;
            }
        }
    }
    let Some(split_at) = anchor_end else {
        return text.to_string();
    };

    let intro = text[..split_at].trim().trim_end_matches([':', ',', ';']);
    let items_text = text[split_at..].trim_start_matches([':', ',', ';', ' ']).trim();
    if intro.is_empty() || items_text.is_empty() {
        return text.to_string();
    }

    let items: Vec<String> = items_text
        .split([',', ';'])
        .flat_map(split_on_and)
        .map(|s| s.trim().trim_end_matches('.').trim().to_string())
        .filter(|s| !s.is_empty())
        .map(capitalize)
        .collect();
    // Every item must look like an item, not a clause.
    if items.len() < 2 || items.iter().any(|i| i.split_whitespace().count() > 5) {
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

    #[test]
    fn bulletize_conversational_colon() {
        // Real user dictation from live testing (take003).
        let out = bulletize(
            "Okay, fine. Now I'm making my grocery list. Make sure to add the following: apple, mango, grapes, dark chocolate.",
        );
        assert_eq!(
            out,
            "Okay, fine. Now I'm making my grocery list. Make sure to add the following:\n- Apple\n- Mango\n- Grapes\n- Dark chocolate"
        );
    }

    #[test]
    fn bulletize_conversational_should_have() {
        // Real user dictation from live testing (take004).
        let out = bulletize(
            "I want you to make me a grocery list and the grocery list should have apple, mango, grapes, and chocolate.",
        );
        assert_eq!(
            out,
            "I want you to make me a grocery list and the grocery list should have:\n- Apple\n- Mango\n- Grapes\n- Chocolate"
        );
    }

    #[test]
    fn bulletize_ignores_time_colons() {
        let t = "Remind the team at 6:30, not 7:00 and not 8:00.";
        assert_eq!(bulletize(t), t);
    }

    #[test]
    fn rephrase_on_fillers_and_stutters() {
        assert!(needs_rephrase("Um so the meeting I think we should move it to Monday."));
        assert!(needs_rephrase("Send the the report to the professor today."));
        assert!(needs_rephrase("So basically the project needs more time for testing."));
    }

    #[test]
    fn rephrase_on_runon_list_only() {
        // No pauses/commas -> bulletizer can't split -> LLM should handle it.
        assert!(needs_rephrase("Make me a shopping list of apple mango breads"));
        // Pause-separated list is handled instantly by the bulletizer.
        assert!(!needs_rephrase("Make me a shopping list, apples, mangoes and bread."));
    }

    #[test]
    fn rephrase_on_agreement_errors() {
        assert!(needs_rephrase("I is going home today."));
        assert!(needs_rephrase("They was late for the meeting."));
        assert!(needs_rephrase("He don't know the answer."));
        assert!(needs_rephrase("We was thinking about the project."));
        // Correct grammar must NOT trigger the slow path.
        assert!(!needs_rephrase("I am going home today."));
        assert!(!needs_rephrase("They were late for the meeting."));
        assert!(!needs_rephrase("He doesn't know the answer."));
        assert!(!needs_rephrase("I was at home yesterday."));
    }

    #[test]
    fn clean_speech_stays_fast() {
        assert!(!needs_rephrase("Send a message to the team that the demo is ready."));
        assert!(!needs_rephrase("Can you write me a funny joke?"));
        assert!(!needs_rephrase("The waiting list is long."));
    }
}
