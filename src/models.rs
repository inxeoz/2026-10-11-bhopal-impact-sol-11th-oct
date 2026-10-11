use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Hi,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Hi => "hi",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Hi => "Hindi",
        }
    }
}

/// A citation the jury can open in the source PDF.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Source {
    /// Loud, auditable label: "Rule 56(1)(a)" or "Ganesh 2023".
    pub label: String,
    /// Where to look: "PDF page 77".
    pub cite: String,
    /// Exact text the answer came from.
    pub quote: String,
}

/// One traced evidence point behind a Track 2 rank.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Evidence {
    pub ghat: String,
    pub parameter: String,
    pub phase: String,
    pub value: String,
    pub verdict: Verdict,
    pub cite: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Fail,
    Missing,
    Low,
}

/// One point on a chart. `value` is None where the reading is missing.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Point {
    pub label: String,
    pub value: Option<f64>,
}

/// Verdict tally across all readings for a ghat.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Counts {
    pub pass: usize,
    pub fail: usize,
    pub low: usize,
    pub missing: usize,
}

/// A ghat's risk rank with the reason in plain words.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GhatRisk {
    pub rank: usize,
    pub ghat: String,
    pub water_body: String,
    pub tier: String,
    pub score: f64,
    pub reason: String,
    pub evidence: Vec<Evidence>,
    /// Dissolved oxygen across every test point, in time order (for the trend chart).
    pub do_series: Vec<Point>,
    pub counts: Counts,
}

/// The single response shape for both tracks.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Answer {
    pub track: String,
    pub question: String,
    pub lang: String,
    /// One-line verdict.
    pub answer: String,
    /// Worked steps / reasoning, plain prose.
    pub detail: String,
    /// False when the source is silent — never guess.
    pub abstained: bool,
    pub sources: Vec<Source>,
    /// Version / data caveat.
    pub note: String,
    /// Structured numbers the jury audits. Produced by code, never the model.
    pub facts: serde_json::Value,
}

impl Answer {
    pub fn abstain(track: &str, question: &str, lang: Lang, why: &str) -> Self {
        let (a, d) = match lang {
            Lang::En => (
                "Not found in the rules.",
                format!(
                    "This tool only answers from the M.P. Bhumi Vikas Rules, 2012. {why} \
                     The clerk should refer the citizen to the department."
                ),
            ),
            Lang::Hi => (
                "नियमों में नहीं मिला।",
                format!(
                    "यह उपकरण केवल म.प्र. भूमि विकास नियम, 2012 से उत्तर देता है। {why} \
                     कृपया नागरिक को विभाग से संपर्क करने के लिए कहें।"
                ),
            ),
        };
        Answer {
            track: track.into(),
            question: question.into(),
            lang: lang.code().into(),
            answer: a.into(),
            detail: d,
            abstained: true,
            sources: vec![],
            note: String::new(),
            facts: serde_json::json!({}),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AskRequest {
    pub track: String,
    pub question: String,
    pub lang: Option<Lang>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GhatsResponse {
    pub ghats: Vec<GhatRisk>,
    pub note: String,
    pub brief_en: String,
    pub brief_hi: String,
}
