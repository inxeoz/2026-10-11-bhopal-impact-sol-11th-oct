use crate::corpus::{tokenize, Corpus, Rule};
use crate::facts::Facts;
use crate::scene::{SceneInput, SPATIAL_WORDS};
use crate::llm::Llm;
use crate::models::{Answer, Details, Lang, Source};
use regex::Regex;

pub struct Track1 {
    pub corpus: Corpus,
    /// Every number the 3D scene uses, read from the rule text at load.
    pub facts: Facts,
}

const VERSION_NOTE: &str = "Source: M.P. Bhumi Vikas Rules, 2012, consolidated to 6 March 2025. \
Rule 2(5)(c) was amended on 28 August 2026 (plots up to 500 sq m) and is NOT reflected in this answer.";

const PARSE_SYSTEM: &str = "You normalise a question about the M.P. Bhumi Vikas Rules, 2012. \
Return ONLY JSON: {\"english_query\": \"<the question rewritten as English search keywords>\", \
\"out_of_scope\": <true if it is not about these building rules>}. Keep any numbers. \
Rewrite Hindi questions in English. Do not answer the question.";

/// Lets the model pick 3D components for a question, from the catalog only.
fn compose_system() -> String {
    let ids: Vec<String> = crate::catalog::CATALOG
        .iter()
        .map(|e| format!("{} ({})", e.id, e.name))
        .collect();
    format!(
        "Pick which of these 3D components a question about a building plot mentions or clearly implies. \
         Components: {}. Use types: residential, business, educational, institutional, assembly, industrial, farm_house. \
         Return ONLY JSON: {{\"components\": [<ids from the list>], \"use_type\": \"<one use type>\"}}. \
         Never output numbers.",
        ids.join(", ")
    )
}

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

pub(crate) struct Band {
    pub lo: f64,
    pub hi: Option<f64>,
    pub val: String,
    pub quote: String,
}

impl Track1 {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let corpus = Corpus::load(path)?;
        let facts = Facts::load(&corpus);
        Ok(Track1 { corpus, facts })
    }

    pub(crate) fn rule(&self, no: &str) -> Option<&Rule> {
        self.corpus.rules.iter().find(|r| r.rule == no)
    }

    pub async fn solve(&self, question: &str, lang: Lang, llm: &Llm) -> Answer {
        self.solve_with(question, lang, llm, &Details::default()).await
    }

    /// `solve`, with plot details the user filled in after asking (they win over the question).
    pub async fn solve_with(&self, question: &str, lang: Lang, llm: &Llm, details: &Details) -> Answer {
        let mut a = self.answer(question, lang, llm, details).await;
        // A plot-shaped question also gets a 3D scene, built from the same rules.
        if !a.abstained {
            let nums = Numbers::parse(question).with_details(details);
            let q = question.to_lowercase();
            let mut input = SceneInput::from_question(question, &nums);
            input.apply_details(details);
            let named = input.components.as_deref().is_some_and(|c| !c.is_empty()) || input.use_type.is_some();
            if nums.is_spatial() || named || mentions(&q, SPATIAL_WORDS) {
                // The model may add catalog ids the keyword match missed (e.g. "a place to park
                // my car under the house" -> stilt_parking). It picks from a closed list only.
                if llm.is_available() {
                    if let Some(v) = llm.json(lang, &compose_system(), question).await {
                        let ids: Vec<String> = v["components"]
                            .as_array()
                            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                            .unwrap_or_default();
                        input.add_components(&ids);
                        if input.use_type.is_none() {
                            input.use_type = v["use_type"].as_str().filter(|u| *u != "residential").map(str::to_string);
                        }
                    }
                }
                if let Ok(v) = serde_json::to_value(self.scene(&input)) {
                    a.facts["scene"] = v;
                }
            }
        }
        a
    }

    async fn answer(&self, question: &str, lang: Lang, llm: &Llm, details: &Details) -> Answer {
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

        let nums = Numbers::parse(question).with_details(details);
        // "frontage" is not "front".
        let qf = q.replace("frontage", "");

        // Plot-shaped questions are answered from the same scene the 3D view draws, so the
        // text never contradicts the picture. Rule 56 front/rear lookups below stay as the
        // audited fast path for plain residential questions.
        let mut input = SceneInput::from_question(question, &nums);
        input.apply_details(details);
        let asked: Vec<String> = input
            .components
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|id| crate::catalog::entry(id).is_some_and(|e| e.not_drawn.is_none()))
            .map(str::to_string)
            .collect();
        let tall = input
            .height
            .or(input.floors.map(|f| f * 3.0))
            .zip(self.facts.v("r56.max_h"))
            .is_some_and(|(h, max)| h > max);
        // Whole words: "residential" must not count as "side".
        let words = |text: &str, keys: &[&str]| keys.iter().any(|k| crate::catalog::contains_word(text, k));
        let open_space_q = words(&qf, &["front", "rear", "side", "sides", "open space", "setback", "mos", "around", "आगे", "सामने", "पीछे", "खुली जगह"]);
        let general_q = words(&qf, &["side", "sides", "open spaces", "around", "setbacks", "दोनों ओर"]);
        let component_q = !asked.is_empty()
            && words(&q, &["allowed", "can i", "can we", "is it", "is the", "how far", "where", "fit", "need", "required", "enough", "क्या", "कहाँ", "कितनी दूर"]);
        let spatial = nums.is_spatial() || input.use_type.is_some();
        // Only with real numbers: a verdict built on assumed defaults would mislead, so a
        // general rule question ("how far from a line?") gets the rule itself below.
        if spatial {
            let scene = self.scene(&input);
            let failing = scene.components.iter().any(|c| !c.ok && asked.iter().any(|a| a == c.id));
            if failing
                || component_q
                || ((tall || input.use_type.is_some()) && open_space_q)
                || (general_q && spatial)
            {
                let focus = if component_q || failing { asked.clone() } else { vec![] };
                if let Some(mut a) = self.scene_answer(question, lang, &scene, &focus, open_space_q) {
                    if lang == Lang::Hi && llm.is_available() {
                        let payload = serde_json::json!({ "answer": a.answer, "detail": a.detail });
                        if let Some(v) = llm.json(Lang::Hi, TRANSLATE_SYSTEM, &payload.to_string()).await {
                            if let Some(t) = v["answer"].as_str() {
                                a.answer = t.trim().to_string();
                            }
                            if let Some(t) = v["detail"].as_str() {
                                a.detail = t.trim().to_string();
                            }
                        }
                    }
                    return a;
                }
            }
        }

        // Deterministic table answers first — these are the audited numbers.
        if mentions(&qf, &["front", "आगे", "सामने"]) && nums.street_width.is_some() {
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

    /// Rule 56(1)(a) table row for a street width.
    pub(crate) fn front_band(&self, width: f64) -> Option<Band> {
        let r = self.rule("56")?;
        let re_up = Regex::new(r"(?i)Up to\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?").ok()?;
        let re_bt =
            Regex::new(r"(?i)More than\s+([0-9.]+)\s*meters?\s+and up to\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?")
                .ok()?;
        let re_ab = Regex::new(r"(?i)Above\s+([0-9.]+)\s*meters?\s+([0-9.]+)\s*meters?").ok()?;
        let section = section(&r.text, "Front open spaces", "Rear Open Space");

        let mut bands = Vec::new();
        for c in re_up.captures_iter(&section) {
            // Skip the tail of "More than X and up to Y" rows; re_bt reads those whole.
            if section[..c.get(0).unwrap().start()].trim_end().to_lowercase().ends_with("and") {
                continue;
            }
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
        take(bands, width)
    }

    /// An answer read off the composed scene: open spaces, plus a verdict for each
    /// component the question asked about (`focus`). A failing component leads.
    fn scene_answer(
        &self,
        question: &str,
        lang: Lang,
        s: &crate::scene::Scene,
        focus: &[String],
        open_space_q: bool,
    ) -> Option<Answer> {
        let hi = lang == Lang::Hi;
        let _ = open_space_q;
        let (front, rear) = (s.front.as_ref()?, s.rear.as_ref()?);
        let (l, r) = (s.side_left.as_ref().map(|x| x.m), s.side_right.as_ref().map(|x| x.m));
        let sides = match (l, r, hi) {
            (Some(a), Some(b), false) if a == b => format!("{a} m on each side"),
            (Some(a), Some(b), false) => format!("{a} m and {b} m on the two sides"),
            (Some(a), None, false) => format!("{a} m on one side"),
            (_, _, false) => "no side open space".into(),
            (Some(a), Some(b), true) if a == b => format!("दोनों ओर {a} मीटर"),
            (Some(a), Some(b), true) => format!("एक ओर {a} मीटर और दूसरी ओर {b} मीटर"),
            (Some(a), None, true) => format!("एक ओर {a} मीटर"),
            (_, _, true) => "बगल में खुली जगह आवश्यक नहीं".into(),
        };
        let open = if hi {
            format!("सामने {} मीटर, पीछे {} मीटर, {sides} खुली जगह छोड़ें।", front.m, rear.m)
        } else {
            format!("Leave {} m at the front, {} m at the rear and {sides}.", front.m, rear.m)
        };

        // Components that are checked against something else lead (line vs balcony, water vs building).
        const FIRST: &[&str] = &["electric_line", "water_body", "septic_tank", "garage", "stilt_parking", "courtyard", "staircase", "lift"];
        let rank = |id: &str| FIRST.iter().position(|x| *x == id).unwrap_or(FIRST.len());
        let mut picked: Vec<&crate::scene::Component> =
            s.components.iter().filter(|c| focus.iter().any(|f| f == c.id)).collect();
        picked.sort_by_key(|c| (c.ok, rank(c.id)));
        let line = |c: &crate::scene::Component| {
            let verdict = match (c.ok, hi) {
                (true, false) => "",
                (false, false) => "not allowed as asked — ",
                (true, true) => "",
                (false, true) => "जैसा पूछा वैसा अनुमत नहीं — ",
            };
            format!("{}: {verdict}{}", c.name, c.notes.first().cloned().unwrap_or_default())
        };
        let failing = picked.iter().find(|c| !c.ok);
        let ql = question.to_lowercase();
        let explicit_open = mentions(&ql, &["open space", "setback", "how much", "खुली जगह", "कितनी जगह"]);
        // "Can I build …?" gets yes/no first when the scene breaks a rule.
        let permission_q = ["can i", "can we", "allowed", "permitted", "possible", "क्या"]
            .iter()
            .any(|k| crate::catalog::contains_word(&ql, k));
        let answer = match (failing, picked.first(), open_space_q && explicit_open) {
            (Some(c), _, _) => line(c),
            _ if permission_q && !s.permitted && !s.notes.is_empty() => {
                if hi {
                    format!("जैसा बताया वैसा नहीं: {}", s.notes[0])
                } else {
                    format!("Not as described: {}", s.notes[0])
                }
            }
            (None, Some(c), false) => line(c),
            _ => open.clone(),
        };

        let mut detail = vec![format!("{}.", s.envelope)];
        if answer != open {
            detail.push(open);
        }
        for c in &picked {
            let l = line(c);
            if l != answer {
                detail.push(l);
            }
        }
        detail.extend(s.notes.iter().chain(&s.info).filter(|n| !picked.iter().any(|c| n.contains(c.notes.first().map(String::as_str).unwrap_or("\u{0}")))).cloned());
        if !s.assumed.is_empty() {
            let a: Vec<&str> = s.assumed.iter().map(|a| a.text.as_str()).collect();
            detail.push(format!("Assumed: {}.", a.join(", ")));
        }

        let mut sources: Vec<Source> = vec![];
        let mut add = |x: Source| {
            if !sources.iter().any(|y| y.quote == x.quote) {
                sources.push(x);
            }
        };
        for c in &picked {
            for src in &c.sources {
                add(src.clone());
            }
        }
        for sb in [Some(front), Some(rear), s.side_left.as_ref()].into_iter().flatten() {
            add(Source { label: sb.label.clone(), cite: sb.cite.clone(), quote: sb.quote.clone() });
        }
        // Open-space questions lead with the open-space sources.
        if picked.is_empty() {
            sources.sort_by_key(|x| !x.label.starts_with("Front"));
        }

        Some(Answer {
            track: "bhumi-vikas".into(),
            question: question.into(),
            lang: lang.code().into(),
            answer,
            detail: detail.join(" "),
            abstained: false,
            sources,
            note: VERSION_NOTE.into(),
            facts: serde_json::json!({
                "front_open_space_m": front.m,
                "rear_open_space_m": rear.m,
                "side_open_space_m": [l, r],
                "envelope": s.envelope,
                "permitted": s.permitted,
                "verdicts": picked.iter().map(|c| serde_json::json!({ "id": c.id, "ok": c.ok })).collect::<Vec<_>>(),
            }),
        })
    }

    fn front_open_space(&self, question: &str, lang: Lang, width: f64) -> Option<Answer> {
        let r = self.rule("56")?;
        let band = self.front_band(width)?;
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

    /// Rule 56(2)(a) table row for a plot area.
    pub(crate) fn rear_band(&self, area: f64) -> Option<Band> {
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
        take(bands, area)
    }

    fn rear_open_space(&self, question: &str, lang: Lang, area: f64) -> Option<Answer> {
        let r = self.rule("56")?;
        let band = self.rear_band(area)?;
        let val = &band.val;

        let (answer, detail) = match lang {
            Lang::En if val.eq_ignore_ascii_case("nil") => (
                "No rear open space is required (Nil).".to_string(),
                format!("For a residential plot of {area} sq m, Rule 56(2)(a) sets the rear open space as Nil."),
            ),
            Lang::Hi if val.eq_ignore_ascii_case("nil") => (
                "पीछे खुली जगह आवश्यक नहीं (शून्य)।".to_string(),
                format!("{area} वर्ग मीटर के आवासीय भूखंड पर नियम 56(2)(क) के अनुसार पीछे की खुली जगह शून्य है।"),
            ),
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

pub(crate) fn mentions(q: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| q.contains(k))
}

/// Slice a rule's text between `start` and `end` markers so table regexes only
/// see the right table.
pub(crate) fn section(text: &str, start: &str, end: &str) -> String {
    let s = text.to_lowercase();
    let i = s.find(&start.to_lowercase()).map(|i| i).unwrap_or(0);
    let rest = &text[i..];
    let j = rest
        .to_lowercase()
        .find(&end.to_lowercase())
        .unwrap_or(rest.len());
    rest[..j].to_string()
}

fn take(bands: Vec<Band>, v: f64) -> Option<Band> {
    bands
        .into_iter()
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
    pub frontage: Option<f64>,
    pub floors: Option<f64>,
}

impl Numbers {
    /// True when the question gives anything that shapes the plot or building.
    pub fn is_spatial(&self) -> bool {
        self.street_width.is_some()
            || self.plot_area.is_some()
            || self.height.is_some()
            || self.frontage.is_some()
            || self.floors.is_some()
    }

    /// Fill in (and override with) what the user gave outside the question.
    pub fn with_details(mut self, d: &Details) -> Numbers {
        let pos = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);
        self.plot_area = pos(d.plot_area).or(self.plot_area);
        self.frontage = pos(d.frontage).or(self.frontage);
        self.street_width = pos(d.street_width).or(self.street_width);
        if let Some(h) = pos(d.height) {
            self.height = Some(h);
            self.floors = None; // a stated height replaces "3 floors x 3 m"
        }
        self
    }

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
            // Length: classify by the NEAREST context word, so "40 m frontage, on an
            // 18 m road" gives frontage 40 and road 18.
            let m = c.get(0).unwrap();
            let before = q[floor_char(q, m.start().saturating_sub(30))..m.start()].to_lowercase();
            let after = q[m.end()..floor_char(q, (m.end() + 24).min(q.len()))].to_lowercase();
            // "1.5 m above the road" is a level difference, not a size (see scene::parse_rise).
            if ["above", "below", "higher than", "lower than", "sea level", "amsl", "msl", "ऊपर", "नीचे", "ऊँचा", "ऊंचा", "नीचा"]
                .iter()
                .any(|k| after.trim_start().starts_with(k))
            {
                continue;
            }
            let dist = |keys: &[&str]| {
                keys.iter()
                    .filter_map(|k| {
                        let b = before.rfind(k).map(|i| before.len() - i - k.len());
                        let a = after.find(k);
                        b.into_iter().chain(a).min()
                    })
                    .min()
            };
            let classes = [
                (dist(&["frontage", "plot width", "wide plot", "फ्रंटेज"]), 0),
                (dist(&["tall", "high", "height", "floor", "tower", "multistor", "building", "house", "apartment", "bungalow", "factory", "hall", "ऊँच", "ऊंच", "मंजिल", "मंज़िल", "बहुमंजिल", "मकान", "इमारत", "भवन"]), 1),
                (dist(&["road", "street", "lane", "सड़क", "चौड़"]), 2),
            ];
            let slot = match classes.iter().filter_map(|(d, i)| d.map(|d| (d, *i))).min() {
                Some((_, 0)) => &mut out.frontage,
                Some((_, 1)) => &mut out.height,
                Some((_, 2)) => &mut out.street_width,
                _ => continue,
            };
            if slot.is_none() {
                *slot = Some(val);
            }
        }
        // Plot in hectares ("0.5 hectare", "1 ha", "2 हेक्टेयर") — farm houses are sized this way.
        if out.plot_area.is_none() {
            let re_ha = Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)\s*(?:hectares?|hectre|ha\b|हेक्टेयर)").unwrap();
            if let Some(c) = re_ha.captures(q) {
                out.plot_area = c[1].parse::<f64>().ok().map(|h| h * 10_000.0);
            }
        }
        // Floor count: "3 floors", "G+2", "3 मंजिल".
        let re_f = Regex::new(r"(?i)([0-9]+)\s*-?\s*(?:floors?|stor(?:e)?ys?|stories|मंजिल|मंज़िल|मंजिला)").unwrap();
        let re_g = Regex::new(r"(?i)\bG\s*\+\s*([0-9]+)").unwrap();
        if let Some(c) = re_f.captures(q) {
            out.floors = c[1].parse().ok();
        } else if let Some(c) = re_g.captures(q) {
            out.floors = c[1].parse::<f64>().ok().map(|n| n + 1.0);
        }
        // Built-up area: a number near "built" / "construct".
        let re_b = Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)\s*(square\s*met(?:er|re)s?|sq\.?\s*m|sqm|वर्ग\s*मीटर)").unwrap();
        for c in re_b.captures_iter(q) {
            let start = c.get(0).unwrap().start();
            let win = q[floor_char(q, start.saturating_sub(30))..].to_lowercase();
            if win.contains("built") || win.contains("construct") || win.contains("निर्मित") {
                out.built_up_area = c[1].parse().ok();
            }
        }
        out
    }
}

/// Largest char boundary at or below `i` — byte offsets into Hindi text must not split a character.
fn floor_char(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
