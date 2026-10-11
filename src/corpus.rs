use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Deserialize, Debug)]
pub struct Rule {
    pub rule: String,
    pub title: String,
    pub part: String,
    pub page_start: u32,
    pub page_end: u32,
    pub text: String,
}

const STOP: &[&str] = &[
    "the", "a", "an", "of", "to", "in", "for", "and", "or", "is", "are", "be", "on", "at", "by",
    "with", "as", "my", "i", "how", "much", "what", "which", "do", "does", "can", "will", "shall",
    "it", "this", "that", "leave", "have", "has", "am", "not", "no", "if", "from", "than", "then",
];

pub fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Minimal BM25 over the rule texts. Good enough to find the right rule;
/// everything downstream is deterministic.
pub struct Corpus {
    pub rules: Vec<Rule>,
    df: HashMap<String, usize>,
    len: Vec<usize>,
    avg_len: f64,
    n: f64,
}

impl Corpus {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let rules: Vec<Rule> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("bad rule jsonl"))
            .collect();

        let mut df: HashMap<String, usize> = HashMap::new();
        let mut len = Vec::with_capacity(rules.len());
        for r in &rules {
            let toks = tokenize(&format!("{} {} {}", r.rule, r.title, r.text));
            let mut seen: Vec<String> = toks.clone();
            seen.sort();
            seen.dedup();
            for t in seen {
                *df.entry(t).or_insert(0) += 1;
            }
            len.push(toks.len());
        }
        let n = rules.len() as f64;
        let avg_len = len.iter().sum::<usize>() as f64 / n.max(1.0);

        Ok(Corpus { rules, df, len, avg_len, n })
    }

    /// BM25 scores, highest first. Returns (rule, score) pairs.
    pub fn search(&self, query: &str, k: usize) -> Vec<(usize, f64)> {
        let q = tokenize(query);
        if q.is_empty() {
            return vec![];
        }
        let k1 = 1.5;
        let b = 0.75;
        let mut scored: Vec<(usize, f64)> = self
            .rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let toks = tokenize(&format!("{} {} {} {}", r.rule, r.title, r.part, r.text));
                let mut tf: HashMap<&str, usize> = HashMap::new();
                for t in &toks {
                    *tf.entry(t.as_str()).or_insert(0) += 1;
                }
                let dl = self.len[i] as f64;
                let mut score = 0.0;
                for term in &q {
                    let f = *tf.get(term.as_str()).unwrap_or(&0) as f64;
                    if f == 0.0 {
                        continue;
                    }
                    let df = *self.df.get(term).unwrap_or(&0) as f64;
                    let idf = ((self.n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    score += idf * (f * (k1 + 1.0)) / (f + k1 * (1.0 - b + b * dl / self.avg_len));
                }
                (i, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(k);
        scored
    }
}
