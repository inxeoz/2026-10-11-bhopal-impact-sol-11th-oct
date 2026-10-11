use crate::corpus::{tokenize, Corpus, Rule};
use crate::llm::Llm;
use crate::models::{Answer, Lang, Source};
use regex::Regex;

pub struct Track1 {
    pub corpus: Corpus,
}

const VERSION_NOTE: &str = "Source: M.P. Bhumi Vikas Rules, 2012, consolidated to 6 March 2025. \
Rule 2(5)(c) was amended on 28 August 2026 (plots up to 500 sq m) and is NOT reflected in this answer.";

const PARSE_SYSTEM: &str = "You normalise a question about the M.P. Bhumi Vikas Rules, 2012. \
Return ONLY JSON: {\"english_query\": \"<the question rewritten as English search keywords>\", \
\"out_of_scope\": <true if it is not about these building rules>}. Keep any numbers. \
Rewrite Hindi questions in English. Do not answer the question.";

const TRANSLATE_SYSTEM: &str = "Translate the JSON fields 'answer' and 'detail' into Hindi \
(Devanagari). Keep EVERY number, unit, rule number and page number exactly as written. \
Return the same JSON keys and nothing else.";

/// Topics the rulebook does not cover. A keyed blocklist keeps the tool honest
/// even when the model is offline. `ponytail:` grow this list if the sample set
/// shows other out-of-scope questions leaking through.
const OUT_OF_SCOPE: &[&str] = &[
    "property tax",
    "tax online",
    "market rate",
    "market value",
    "circle rate",
    "stamp duty",
    "valuation",
    "registration fee",
];

struct Band {
    lo: f64,
    hi: Option<f64>,
    val: String,
    quote: String,
}

impl Track1 {
    pub fn load(path: &str) -> std::io::Result<Self> {
        Ok(Track1 { corpus: Corpus::load(path)? })
    }

    fn rule(&self, no: &str) -> Option<&Rule> {
        self.corpus.rules.iter().find(|r| r.rule == no)
    }

    pub async fn solve(&self, question: &str, lang: Lang, llm: &Llm) -> Answer {
        let q = question.to_lowercase();

        if OUT_OF_SCOPE.iter().any(|k| q.contains(k)) {
            return Answer::abstain(
                "bhumi-vikas",
                question,
                lang,
                "This question is not about the building rules.",
            );
        }

        // The model helps UNDERSTAND the question: normalise it to an English
        // search phrase and flag out-of-scope. It never produces a number.
        let mut english = question.to_string();
        if llm.is_available() {
            if let Some(v) = llm.json(lang, PARSE_SYSTEM, question).await {
                if v["out_of_scope"].as_bool() == Some(true) {
                    return Answer::abstain(
                        "bhumi-vikas",
                        question,
                        lang,
                        "This question is not about the building rules.",
                    );
                }
                if let Some(eq) = v["english_query"].as_str() {
                    if !eq.trim().is_empty() {
                        english = eq.trim().to_string();
                    }
                }
            }
        }

        let nums = Numbers::parse(question);

        // Deterministic table answers first — these are the audited numbers.
        if mentions(&q, &["front", "आगे", "सामने"]) && nums.street_width.is_some() {
            if let Some(a) = self.front_open_space(question, lang, nums.street_width.unwrap()) {
                return a;
            }
        }
        if mentions(&q, &["rear", "पीछे"]) && nums.plot_area.is_some() {
            if let Some(a) = self.rear_open_space(question, lang, nums.plot_area.unwrap()) {
                return a;
            }
        }
        if mentions(&q, &["fee", "फीस", "शुल्क"]) && nums.built_up_area.is_some() {
            return self.residence_fee(question, lang, nums.built_up_area.unwrap());
        }
        if mentions(&q, &["valid", "renew", "revalidat", "duration", "कितने साल", "वैध"]) {
            if let Some(a) = self.prose_rule(question, lang, "23", &["valid", "revalidat", "three"]) {
                return a;
            }
        }
        if mentions(&q, &["not replied", "no reply", "deemed", "miss the deadline", "जवाब नहीं"]) {
            if let Some(a) =
                self.prose_rule(question, lang, "28", &["deemed", "sanction", "refusal"])
            {
                return a;
            }
        }

        // Otherwise: retrieve the best rule and quote the relevant sentences.
        let mut a = self.retrieved_answer(&english, lang);
        a.question = question.into();

        // Say it in the user's language. Numbers and rule numbers are kept exact.
        if lang == Lang::Hi && llm.is_available() && !a.abstained {
            let payload = serde_json::json!({ "answer": a.answer, "detail": a.detail });
            if let Some(v) = llm.json(Lang::Hi, TRANSLATE_SYSTEM, &payload.to_string()).await {
                if let Some(s) = v["answer"].as_str() {
                    a.answer = s.trim().to_string();
                }
                if let Some(s) = v["detail"].as_str() {
                    a.detail = s.trim().to_string();
                }
            }
        }
        a
    }

    fn front_open_space(&self, question: &str, lang: Lang, width: f64) -> Option<Answer> {
        let r = self.rule("56")?;
        let re_up = Regex::new(r"(?i)Up to\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?").ok()?;
        let re_bt =
            Regex::new(r"(?i)More than\s+([0-9.]+)\s*meters?\s+and up to\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?")
                .ok()?;
        let re_ab = Regex::new(r"(?i)Above\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?").ok()?;
        let section = section(&r.text, "Front open spaces", "Rear Open Space");

        let mut bands = Vec::new();
        for c in re_up.captures_iter(&section) {
            bands.push(Band {
                lo: 0.0,
                hi: c[1].parse().ok(),
                val: c[2].to_string(),
                quote: c[0].to_string(),
            });
        }
        for c in re_bt.captures_iter(&section) {
            bands.push(Band {
                lo: c[1].parse().ok()?,
                hi: c[2].parse().ok(),
                val: c[3].to_string(),
                quote: c[0].to_string(),
            });
        }
        for c in re_ab.captures_iter(&section) {
            bands.push(Band {
                lo: c[1].parse().ok()?,
                hi: None,
                val: c[2].to_string(),
                quote: c[0].to_string(),
            });
        }
        bands.sort_by(|a, b| a.lo.partial_cmp(&b.lo).unwrap());
        let band = pick(&bands, width)?;
        let val = &band.val;

        let (answer, detail) = match lang {
            Lang::En => (
                format!("Leave {val} m of open space at the front."),
                format!(
                    "For a residential building up to 12.5 m tall on a street {width} m wide, \
                     Rule 56(1)(a) requires a front open space of {val} m. This is the minimum."
                ),
            ),
            Lang::Hi => (
                format!("सामने {val} मीटर खुली जगह छोड़ें।"),
                format!(
                    "{width} मीटर चौड़ी सड़क पर 12.5 मीटर तक ऊँची आवासीय इमारत के लिए नियम 56(1)(क) \
                     के अनुसार सामने कम से कम {val} मीटर खुली जगह आवश्यक है।"
                ),
            ),
        };

        Some(Answer {
            track: "bhumi-vikas".into(),
            question: question.into(),
            lang: lang.code().into(),
            answer,
            detail,
            abstained: false,
            sources: vec![Source {
                label: "Rule 56(1)(a)".into(),
                cite: format!("PDF page {}", r.page_start),
                quote: format!("Front open spaces — {}", band.quote),
            }],
            note: VERSION_NOTE.into(),
            facts: serde_json::json!({
                "street_width_m": width,
                "front_open_space_m": val,
            }),
        })
    }

    fn rear_open_space(&self, question: &str, lang: Lang, area: f64) -> Option<Answer> {
        let r = self.rule("56")?;
        let re_up = Regex::new(r"(?i)Up to\s+([0-9.]+)\s+(Nil)").ok()?;
        let re_bt = Regex::new(r"(?i)Above\s+([0-9.]+)\s+and up to\s+([0-9.]+)\s+([0-9.]+)").ok()?;
        let re_ab = Regex::new(r"(?i)Above\s+([0-9.]+)\s+([0-9.]+)").ok()?;
        let section = section(&r.text, "Rear Open Space", "Side Open");

        let mut bands = Vec::new();
        for c in re_up.captures_iter(&section) {
            bands.push(Band { lo: 0.0, hi: c[1].parse().ok(), val: c[2].to_string(), quote: c[0].to_string() });
        }
        for c in re_bt.captures_iter(&section) {
            bands.push(Band { lo: c[1].parse().ok()?, hi: c[2].parse().ok(), val: c[3].to_string(), quote: c[0].to_string() });
        }
        for c in re_ab.captures_iter(&section) {
            // Skip rows already captured by re_bt ("Above X and up to Y Z").
            if c[0].contains("and up to") {
                continue;
            }
            bands.push(Band { lo: c[1].parse().ok()?, hi: None, val: c[2].to_string(), quote: c[0].to_string() });
        }
        bands.sort_by(|a, b| a.lo.partial_cmp(&b.lo).unwrap());
        let band = pick(&bands, area)?;
        let val = &band.val;

        let (answer, detail) = match lang {
            Lang::En => (
                format!("Leave {val} m of open space at the rear."),
                format!(
                    "For a residential plot of {area} sq m, Rule 56(2)(a) requires a minimum rear \
                     open space of {val} m."
                ),
            ),
            Lang::Hi => (
                format!("पीछे {val} मीटर खुली जगह छोड़ें।"),
                format!(
                    "{area} वर्ग मीटर के आवासीय भूखंड पर नियम 56(2)(क) के अनुसार पीछे कम से कम \
                     {val} मीटर खुली जगह आवश्यक है।"
                ),
            ),
        };

        Some(Answer {
            track: "bhumi-vikas".into(),
            question: question.into(),
            lang: lang.code().into(),
            answer,
            detail,
            abstained: false,
            sources: vec![Source {
                label: "Rule 56(2)(a)".into(),
                cite: format!("PDF page {}", r.page_start),
                quote: format!("Rear Open Space — {}", band.quote),
            }],
            note: VERSION_NOTE.into(),
            facts: serde_json::json!({ "plot_area_sqm": area, "rear_open_space_m": val }),
        })
    }

    fn residence_fee(&self, question: &str, lang: Lang, area: f64) -> Answer {
        let r = match self.rule("21") {
            Some(r) => r,
            None => return Answer::abstain("bhumi-vikas", question, lang, "Rule 21 not loaded."),
        };
        let section = section(&r.text, "exclusively as residence", "2. A building intended");
        let re_row = Regex::new(r"([0-9][0-9,]*)\s+([0-9][0-9,]*)\s+([0-9][0-9,]*)").unwrap();
        let re_above = Regex::new(r"(?i)above\s+([0-9,]+)\s+([0-9,]+)").unwrap();

        let mut bands: Vec<(f64, Option<f64>, String)> = Vec::new();
        for c in re_row.captures_iter(&section) {
            let lo: f64 = c[1].replace(',', "").parse().unwrap_or(0.0);
            let hi: f64 = c[2].replace(',', "").parse().unwrap_or(0.0);
            let val = format!("Rs. {}", c[3].to_string());
            bands.push((lo, Some(hi), val));
        }
        for c in re_above.captures_iter(&section) {
            let lo: f64 = c[1].replace(',', "").parse().unwrap_or(0.0);
            bands.push((lo, None, format!("Rs. {}", c[2].to_string())));
        }
        bands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let fee = bands
            .iter()
            .find(|(lo, hi, _)| (area > *lo || *lo == 0.0) && hi.map(|h| area <= h).unwrap_or(true))
            .map(|(_, _, v)| v.clone());

        let fee = match fee {
            Some(f) => f,
            None => return Answer::abstain("bhumi-vikas", question, lang, "No fee band matched."),
        };

        let (answer, detail) = match lang {
            Lang::En => (
                format!("Permission fee for a residential building of {area} sq m: {fee}."),
                format!(
                    "Rule 21(3)(b) gives the building-permission fee by built-up area. For {area} sq m \
                     the band is {fee}. There is also an application fee of Rs. 1.00 per sq m of \
                     built-up area (Rule 21(2)(ii))."
                ),
            ),
            Lang::Hi => (
                format!("{area} वर्ग मीटर आवासीय इमारत की अनुमति शुल्क: {fee}।"),
                format!(
                    "नियम 21(3)(ख) निर्मित क्षेत्र के अनुसार भवन-अनुमति शुल्क देता है। {area} वर्ग मीटर \
                     के लिए यह {fee} है। साथ ही आवेदन शुल्क 1.00 रुपये प्रति वर्ग मीटर (नियम 21(2)(ii)) है।"
                ),
            ),
        };

        Answer {
            track: "bhumi-vikas".into(),
            question: question.into(),
            lang: lang.code().into(),
            answer,
            detail,
            abstained: false,
            sources: vec![Source {
                label: "Rule 21(3)(b)".into(),
                cite: format!("PDF page {}", r.page_start),
                quote: "Building permission fee, residence: built-up area band → fee charged".into(),
            }],
            note: VERSION_NOTE.into(),
            facts: serde_json::json!({ "built_up_area_sqm": area, "permission_fee": fee }),
        }
    }

    fn prose_rule(&self, question: &str, lang: Lang, no: &str, keys: &[&str]) -> Option<Answer> {
        let r = self.rule(no)?;
        let quote = best_sentences(&r.text, keys, 2).join(" ");
        if quote.trim().is_empty() {
            return None;
        }
        let (answer, detail) = match lang {
            Lang::En => (
                format!("Rule {} — {}", r.rule, r.title),
                "This is the rule that applies. The exact text is quoted below.".to_string(),
            ),
            Lang::Hi => (
                format!("नियम {} — {}", r.rule, r.title),
                "यह लागू नियम है। सटीक पाठ नीचे उद्धृत है।".to_string(),
            ),
        };
        Some(Answer {
            track: "bhumi-vikas".into(),
            question: question.into(),
            lang: lang.code().into(),
            answer,
            detail,
            abstained: false,
            sources: vec![Source {
                label: format!("Rule {}", r.rule),
                cite: format!("PDF page {}", r.page_start),
                quote,
            }],
            note: VERSION_NOTE.into(),
            facts: serde_json::json!({ "rule": r.rule }),
        })
    }

    fn retrieved_answer(&self, question: &str, lang: Lang) -> Answer {
        let hits = self.corpus.search(question, 3);
        // Require a real match: top score above threshold and 2+ distinct terms.
        let qterms: Vec<String> = {
            let mut t = tokenize(question);
            t.sort();
            t.dedup();
            t
        };
        let good = hits.first().filter(|(i, score)| {
            if *score < 4.0 {
                return false;
            }
            let r = &self.corpus.rules[*i];
            let text = format!("{} {} {}", r.rule, r.title, r.text).to_lowercase();
            qterms.iter().filter(|t| text.contains(t.as_str())).count() >= 2
        });

        match good {
            None => Answer::abstain(
                "bhumi-vikas",
                question,
                lang,
                "No rule matched this question closely enough.",
            ),
            Some((i, _)) => {
                let r = &self.corpus.rules[*i];
                let quote = best_sentences(&r.text, &qterms, 2).join(" ");
                let (answer, detail) = match lang {
                    Lang::En => (
                        format!("Rule {} — {}", r.rule, r.title),
                        "Best match in the rulebook. Read the exact text quoted below and confirm."
                            .to_string(),
                    ),
                    Lang::Hi => (
                        format!("नियम {} — {}", r.rule, r.title),
                        "नियम-पुस्तिका में निकटतम मेल। नीचे दिया सटीक पाठ पढ़कर पुष्टि करें।"
                            .to_string(),
                    ),
                };
                Answer {
                    track: "bhumi-vikas".into(),
                    question: question.into(),
                    lang: lang.code().into(),
                    answer,
                    detail,
                    abstained: false,
                    sources: vec![Source {
                        label: format!("Rule {}", r.rule),
                        cite: format!("PDF pages {}-{}", r.page_start, r.page_end),
                        quote,
                    }],
                    note: VERSION_NOTE.into(),
                    facts: serde_json::json!({ "rule": r.rule, "title": r.title }),
                }
            }
        }
    }
}

fn mentions(q: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| q.contains(k))
}

/// Slice a rule's text between `start` and `end` markers so table regexes only
/// see the right table.
fn section(text: &str, start: &str, end: &str) -> String {
    let s = text.to_lowercase();
    let i = s.find(&start.to_lowercase()).map(|i| i).unwrap_or(0);
    let rest = &text[i..];
    let j = rest
        .to_lowercase()
        .find(&end.to_lowercase())
        .unwrap_or(rest.len());
    rest[..j].to_string()
}

fn pick(bands: &[Band], v: f64) -> Option<&Band> {
    bands
        .iter()
        .find(|b| (v > b.lo || b.lo == 0.0) && b.hi.map(|h| v <= h).unwrap_or(true))
}

/// Score sentences by keyword overlap; return the top `n`, in original order.
fn best_sentences<S: AsRef<str>>(text: &str, keys: &[S], n: usize) -> Vec<String> {
    let norm = text.replace('\n', " ");
    let mut scored: Vec<(usize, f64, String)> = Vec::new();
    for (idx, s) in norm.split(". ").enumerate() {
        let low = s.to_lowercase();
        let score: f64 = keys
            .iter()
            .filter(|k| {
                let ks: &str = (**k).as_ref();
                low.contains(&ks.to_lowercase())
            })
            .count() as f64;
        if score > 0.0 {
            scored.push((idx, score, s.trim().to_string()));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    scored.truncate(n);
    scored.sort_by_key(|(i, _, _)| *i);
    scored.into_iter().map(|(_, _, s)| s).collect()
}

/// Numbers pulled from the question by unit and context, no model involved.
#[derive(Default, Debug)]
pub struct Numbers {
    pub street_width: Option<f64>,
    pub plot_area: Option<f64>,
    pub height: Option<f64>,
    pub built_up_area: Option<f64>,
}

impl Numbers {
    pub fn parse(q: &str) -> Numbers {
        let mut out = Numbers::default();
        let re = Regex::new(
            r"(?i)([0-9]+(?:\.[0-9]+)?)\s*(square\s*met(?:er|re)s?|sq\.?\s*m|sqm|वर्ग\s*मीटर|met(?:er|re)s?|मीटर|m)\b",
        )
        .unwrap();
        for c in re.captures_iter(q) {
            let val: f64 = c[1].parse().unwrap_or(0.0);
            let unit = c[2].to_lowercase();
            let is_area = unit.contains("square")
                || unit.contains("sq")
                || unit.contains("वर्ग");
            if is_area {
                if out.plot_area.is_none() {
                    out.plot_area = Some(val);
                }
                continue;
            }
            // Length: classify by nearby words.
            let start = c.get(0).unwrap().start();
            let win_start = start.saturating_sub(24);
            let win = q[win_start..(start + c[0].len() + 20).min(q.len())].to_lowercase();
            if win.contains("tall") || win.contains("high") || win.contains("height")
                || win.contains("floor") || win.contains("ऊँच") || win.contains("मंजिल")
                || win.contains("मंज़िल")
            {
                if out.height.is_none() {
                    out.height = Some(val);
                }
            } else if win.contains("road") || win.contains("street") || win.contains("सड़क")
                || win.contains("चौड़")
            {
                if out.street_width.is_none() {
                    out.street_width = Some(val);
                }
            }
        }
        // Built-up area: a number near "built" / "construct".
        let re_b = Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)\s*(square\s*met(?:er|re)s?|sq\.?\s*m|sqm|वर्ग\s*मीटर)").unwrap();
        for c in re_b.captures_iter(q) {
            let start = c.get(0).unwrap().start();
            let win_start = start.saturating_sub(30);
            let win = q[win_start..].to_lowercase();
            if win.contains("built") || win.contains("construct") || win.contains("निर्मित") {
                out.built_up_area = c[1].parse().ok();
            }
        }
        out
    }
}
