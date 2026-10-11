use crate::llm::Llm;
use crate::models::{Evidence, GhatRisk, GhatsResponse, Verdict};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Reading {
    year: i32,
    festival: String,
    site: String,
    water_body: String,
    phase: String,
    parameter: String,
    value: Option<f64>,
    printed: String,
    flag: String,
    page: String,
}

pub struct Track2 {
    readings: Vec<Reading>,
}

const NOTE: &str = "Ranked against CPCB Class B (outdoor bathing): pH 6.5–8.5, Dissolved oxygen ≥5 mg/l, \
BOD ≤3 mg/l. The reports print no units; standard units are assumed. 'not sampled' and '*' mean missing \
data, never clean. Immersion-ghat data covers 2023–2025 only.";

impl Track2 {
    pub fn load(csv_path: &str) -> std::io::Result<Self> {
        let mut rdr = csv::Reader::from_path(csv_path)?;
        let mut readings = Vec::new();
        for rec in rdr.records() {
            let rec = rec?;
            let get = |i: usize| rec.get(i).unwrap_or("").trim().to_string();
            readings.push(Reading {
                year: get(0).parse().unwrap_or(0),
                festival: get(1),
                site: get(2),
                water_body: get(3),
                phase: get(4),
                parameter: get(5),
                value: get(6).parse().ok(),
                printed: get(9),
                flag: get(8),
                page: get(13),
            });
        }
        Ok(Track2 { readings })
    }

    pub async fn rank(&self, llm: &Llm) -> GhatsResponse {
        let mut by_ghat: BTreeMap<String, Vec<&Reading>> = BTreeMap::new();
        for r in &self.readings {
            by_ghat.entry(r.site.clone()).or_default().push(r);
        }

        let mut ghats: Vec<GhatRisk> = by_ghat
            .into_iter()
            .map(|(site, rs)| self.score_ghat(&site, &rs))
            .collect();

        ghats.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap()
                .then(a.ghat.cmp(&b.ghat))
        });
        for (i, g) in ghats.iter_mut().enumerate() {
            g.rank = i + 1;
        }

        let brief_en = self.brief(&ghats, true, llm).await;
        let brief_hi = self.brief(&ghats, false, llm).await;

        GhatsResponse {
            ghats,
            note: NOTE.into(),
            brief_en,
            brief_hi,
        }
    }

    fn score_ghat(&self, site: &str, rs: &[&Reading]) -> GhatRisk {
        let water_body = rs.first().map(|r| r.water_body.clone()).unwrap_or_default();
        let mut score = 0.0f64;
        let mut evidence = Vec::new();
        let mut years_failing: BTreeMap<i32, ()> = BTreeMap::new();

        for r in rs {
            let verdict = classify(&r.parameter, r.value, &r.flag, &r.printed);
            if verdict == Verdict::Fail {
                score += weight(&r.parameter, r.value);
                if r.parameter == "Dissolved oxygen" {
                    years_failing.insert(r.year, ());
                }
            }
            // Keep only actionable rows as evidence: breaches and (very) low DO.
            let show = match verdict {
                Verdict::Fail => true,
                Verdict::Low => r.parameter == "Dissolved oxygen",
                _ => false,
            };
            if show {
                evidence.push(Evidence {
                    ghat: site.into(),
                    parameter: r.parameter.clone(),
                    phase: r.phase.clone(),
                    value: if r.printed.is_empty() {
                        r.value.map(fmt_n).unwrap_or_else(|| "—".into())
                    } else {
                        r.printed.clone()
                    },
                    verdict,
                    cite: format!("{} {} · PDF p.{}", r.festival, r.year, r.page),
                });
            }
        }

        // Persistence: a dissolved-oxygen breach seen across several years is worse.
        score += (years_failing.len() as f64 - 1.0).max(0.0) * 1.5;

        // Show the most severe rows first, capped so the page stays readable.
        evidence.sort_by(|a, b| {
            severity(b)
                .partial_cmp(&severity(a))
                .unwrap()
                .then(a.cite.cmp(&b.cite))
        });
        evidence.truncate(10);

        let tier = if score <= 0.0 {
            "No breach"
        } else if score >= 25.0 {
            "High"
        } else if score >= 12.0 {
            "Medium"
        } else {
            "Low"
        };

        let reason = reason_for(site, &evidence, score);

        GhatRisk {
            rank: 0,
            ghat: site.into(),
            water_body,
            tier: tier.into(),
            score: (score * 10.0).round() / 10.0,
            reason,
            evidence,
        }
    }

    async fn brief(&self, ghats: &[GhatRisk], english: bool, llm: &Llm) -> String {
        let fallback = self.brief_fallback(ghats, english);
        let top: Vec<String> = ghats
            .iter()
            .take(3)
            .map(|g| format!("{}. {} — {} ({})", g.rank, g.ghat, g.tier, g.reason))
            .collect();
        let target = if english { "English" } else { "Hindi (Devanagari)" };
        let system = format!(
            "You write a short briefing for a Bhopal Municipal Corporation officer planning Durga \
             visarjan. Write only in {target}. Use plain words. Do not add facts that are not in the \
             input. 5 sentences maximum. End with one line on which ghat to prioritise."
        );
        let user = format!(
            "Ranked ghats by water-quality risk (CPCB Class B):\n{}",
            top.join("\n")
        );
        let lang = if english { crate::models::Lang::En } else { crate::models::Lang::Hi };
        llm.polish(lang, &system, &user).await.unwrap_or(fallback)
    }

    fn brief_fallback(&self, ghats: &[GhatRisk], english: bool) -> String {
        let top = ghats.first();
        if english {
            let mut s = String::from("Water-quality risk at Bhopal immersion ghats (CPCB Class B, outdoor bathing):\n");
            for g in ghats {
                s.push_str(&format!("{}. {} — {}. {}\n", g.rank, g.ghat, g.tier, g.reason));
            }
            if let Some(t) = top {
                s.push_str(&format!("\nPriority: {}", t.ghat));
            }
            s
        } else {
            let mut s = String::from("भोपाल के विसर्जन घाटों पर जल-गुणवत्ता जोखिम (CPCB वर्ग बी, बाहरी स्नान):\n");
            for g in ghats {
                s.push_str(&format!("{}. {} — {}। {}\n", g.rank, g.ghat, g.tier, g.reason));
            }
            if let Some(t) = top {
                s.push_str(&format!("\nप्राथमिकता: {}", t.ghat));
            }
            s
        }
    }
}

pub fn classify(parameter: &str, value: Option<f64>, flag: &str, printed: &str) -> Verdict {
    let p = printed.to_uppercase();
    if p == "BDL" || p == "ND" {
        return Verdict::Low;
    }
    let v = match value {
        Some(v) => v,
        None => return Verdict::Missing,
    };
    let _ = flag;
    match parameter {
        "Dissolved oxygen" => {
            if v >= 5.0 {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
        "BOD" => {
            if v <= 3.0 {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
        "pH" => {
            if (6.5..=8.5).contains(&v) {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
        // No Class B limit set for these parameters.
        _ => Verdict::Pass,
    }
}

/// Dissolved oxygen carries the most weight: it is the acute signal for fish life.
/// BOD is very widespread (near-universal breach), so it is a light modifier.
fn weight(parameter: &str, value: Option<f64>) -> f64 {
    match parameter {
        "Dissolved oxygen" => {
            if value.unwrap_or(99.0) <= 1.0 {
                6.0
            } else {
                2.0
            }
        }
        "BOD" => 0.3,
        "pH" => 0.3,
        _ => 0.1,
    }
}

fn severity(e: &Evidence) -> f64 {
    let v: f64 = e.value.parse().unwrap_or(0.0);
    match (e.parameter.as_str(), e.verdict) {
        ("Dissolved oxygen", Verdict::Fail) => 1000.0 - v,
        ("BOD", Verdict::Fail) => 500.0 + v,
        ("pH", Verdict::Fail) => 300.0,
        ("Dissolved oxygen", Verdict::Low) => 200.0 - v,
        _ => 100.0,
    }
}

fn reason_for(site: &str, evidence: &[Evidence], score: f64) -> String {
    if score <= 0.0 {
        return format!("{site}: no Class B breach in the readings on file.");
    }
    let do_rows: Vec<&Evidence> = evidence
        .iter()
        .filter(|e| e.parameter == "Dissolved oxygen")
        .collect();
    if let Some(worst) = do_rows
        .iter()
        .min_by(|a, b| a.value.parse::<f64>().unwrap_or(99.0).partial_cmp(&b.value.parse::<f64>().unwrap_or(99.0)).unwrap())
    {
        let v: f64 = worst.value.parse().unwrap_or(0.0);
        let n = do_rows.len();
        let acute = if v <= 1.0 { " (near anoxic — no fish life)" } else { "" };
        return format!(
            "Dissolved oxygen as low as {value} mg/l (limit ≥5){acute}. {n} reading(s) below the limit across the festivals on file. Worst: {phase} phase, {cite}.",
            value = worst.value,
            phase = worst.phase,
            cite = worst.cite
        );
    }
    let e = &evidence[0];
    format!(
        "{} {} against the Class B limit at {} phase, {}.",
        e.parameter, e.value, e.phase, e.cite
    )
}

fn fmt_n(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}
