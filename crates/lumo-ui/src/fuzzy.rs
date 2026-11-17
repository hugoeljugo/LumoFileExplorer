//! Fuzzy matching for the command palette: the query's characters must
//! appear in order in the candidate (case-insensitive); matches at word
//! starts and in consecutive runs score higher.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    /// Character indices (not byte offsets) of the matched characters in the
    /// candidate, for highlighting.
    pub indices: Vec<usize>,
}

const MATCH: i32 = 16;
const WORD_START: i32 = 14;
const CONSECUTIVE: i32 = 10;
const PREFIX: i32 = 24;
const MAX_GAP_PENALTY: i32 = 8;

/// Returns `None` if `query` does not match. An empty (or blank) query
/// matches everything with score 0. (Callers matching many candidates
/// should prepare a [`Query`] once instead.)
#[cfg(test)]
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<Match> {
    Query::new(query).matches(candidate)
}

/// A query prepared once and matched against many candidates.
#[derive(Debug, Clone)]
pub struct Query {
    chars: Vec<char>,
}

impl Query {
    pub fn new(query: &str) -> Self {
        Self {
            chars: query
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(fold)
                .collect(),
        }
    }

    pub fn matches(&self, candidate: &str) -> Option<Match> {
        if self.chars.is_empty() {
            return Some(Match {
                score: 0,
                indices: Vec::new(),
            });
        }
        // Cheap, allocation-free rejection first: most candidates don't match.
        if !is_subsequence(&self.chars, candidate) {
            return None;
        }
        score_match(&self.chars, candidate)
    }
}

fn is_subsequence(query: &[char], candidate: &str) -> bool {
    let mut remaining = query.iter().peekable();
    for c in candidate.chars() {
        match remaining.peek() {
            Some(&&q) if fold(c) == q => {
                remaining.next();
            }
            Some(_) => {}
            None => break,
        }
    }
    remaining.peek().is_none()
}

fn score_match(query: &[char], candidate: &str) -> Option<Match> {
    let chars: Vec<char> = candidate.chars().collect();
    let folded: Vec<char> = chars.iter().copied().map(fold).collect();

    let mut indices = Vec::with_capacity(query.len());
    let mut from = 0;
    for &q in query {
        // Prefer an occurrence at a word start if one exists before the
        // next run would otherwise be split; fall back to the first one.
        let first = (from..folded.len()).find(|&i| folded[i] == q)?;
        let at_word_start = (first..folded.len())
            .find(|&i| folded[i] == q && is_word_start(&chars, i))
            .filter(|&i| {
                // Only jump ahead if the skipped stretch has no other match
                // for the following query char (keeps highlights intuitive).
                i == first || !indices.last().is_some_and(|&last| last + 1 == first)
            });
        let pick = at_word_start.unwrap_or(first);
        indices.push(pick);
        from = pick + 1;
    }

    let mut score = 0;
    let mut previous: Option<usize> = None;
    for &i in &indices {
        score += MATCH;
        if is_word_start(&chars, i) {
            score += WORD_START;
        }
        match previous {
            Some(p) if i == p + 1 => score += CONSECUTIVE,
            Some(p) => score -= (i - p - 1).min(MAX_GAP_PENALTY as usize) as i32,
            None => score -= i.min(MAX_GAP_PENALTY as usize) as i32,
        }
        previous = Some(i);
    }
    if indices.iter().enumerate().all(|(n, &i)| n == i) {
        score += PREFIX;
    }
    // Prefer shorter candidates when everything else is equal.
    score -= (chars.len() / 8) as i32;
    Some(Match { score, indices })
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// First character, or one after a separator, or a lower→upper case change
/// (`camelCase`).
fn is_word_start(chars: &[char], i: usize) -> bool {
    let Some(prev) = i.checked_sub(1).map(|p| chars[p]) else {
        return true;
    };
    matches!(prev, ' ' | '-' | '_' | '.' | '/' | '\\' | ':' | '(' | '[')
        || (prev.is_lowercase() && chars[i].is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(q: &str, c: &str) -> i32 {
        fuzzy_match(q, c).map_or(i32::MIN, |m| m.score)
    }

    #[test]
    fn requires_characters_in_order() {
        assert!(fuzzy_match("abc", "a-b-c").is_some());
        assert!(fuzzy_match("cba", "abc").is_none());
        assert!(fuzzy_match("x", "abc").is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_match("  ", "anything").map(|m| m.score), Some(0));
    }

    #[test]
    fn is_case_insensitive_and_ignores_spaces() {
        assert!(fuzzy_match("NEW TAB", "New tab").is_some());
        assert!(fuzzy_match("newtab", "New tab").is_some());
    }

    #[test]
    fn word_starts_beat_scattered_letters() {
        assert!(score("rf", "report-final.pdf") > score("rf", "rarefied.txt"));
        assert!(score("nt", "New tab") > score("nt", "Cantina"));
    }

    #[test]
    fn prefix_and_consecutive_runs_rank_highest() {
        assert!(score("down", "Downloads") > score("down", "Go down a level"));
        assert!(score("ref", "Refresh") > score("ref", "Prefer"));
    }

    #[test]
    fn highlights_word_starts() {
        let m = fuzzy_match("rf", "report-final.pdf").unwrap();
        assert_eq!(m.indices, [0, 7]);
        let m = fuzzy_match("ht", "Show hidden items").unwrap();
        assert_eq!(m.indices, [5, 13]);
    }

    #[test]
    fn camel_case_counts_as_word_start() {
        let m = fuzzy_match("fe", "FileExplorer").unwrap();
        assert_eq!(m.indices, [0, 4]);
    }

    #[test]
    fn indices_are_char_positions_not_bytes() {
        let m = fuzzy_match("é", "café").unwrap();
        assert_eq!(m.indices, [3]);
    }
}
