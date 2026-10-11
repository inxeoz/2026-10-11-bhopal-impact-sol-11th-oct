use serde::{Deserialize, Serialize};

const URL: &str = "https://api.sarvam.ai/speech-to-text";
const MODEL: &str = "saaras:v4";

/// Thin Sarvam speech-to-text client for the Ask box's mic button. The key never
/// reaches the browser: it posts its recording to /api/transcribe and gets text back.
pub struct Stt {
    client: reqwest::Client,
    key: Option<String>,
}

/// What the browser gets back: the words, and the language Sarvam heard ("en-IN", "hi-IN" …).
#[derive(Debug, Serialize, Deserialize)]
pub struct Transcript {
    pub transcript: String,
    #[serde(default)]
    pub language_code: Option<String>,
}

impl Stt {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let key = std::env::var("SARVAMAPI").ok().filter(|k| !k.trim().is_empty());
        Stt { client: reqwest::Client::new(), key }
    }

    pub fn is_available(&self) -> bool {
        self.key.is_some()
    }

    /// Transcribe one short recording (Sarvam's REST call takes up to 30 s of audio).
    /// `mime` is the browser's recording type, e.g. "audio/webm;codecs=opus".
    /// The language is auto-detected, so a Hindi question comes back in Devanagari.
    pub async fn transcribe(&self, audio: Vec<u8>, mime: &str) -> Result<Transcript, String> {
        let key = self.key.as_ref().ok_or("voice input is off: no SARVAMAPI key on the server")?;
        let base = mime.split(';').next().unwrap_or("").trim();
        let ext = match base {
            "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
            "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
            "audio/mpeg" => "mp3",
            "audio/ogg" => "ogg",
            _ => "webm",
        };
        let file = reqwest::multipart::Part::bytes(audio)
            .file_name(format!("question.{ext}"))
            .mime_str(if base.is_empty() { "audio/webm" } else { base })
            .map_err(|e| e.to_string())?;
        let form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model", MODEL)
            .text("language_code", "unknown")
            .text("mode", "transcribe");
        let resp = self
            .client
            .post(URL)
            .header("api-subscription-key", key)
            .multipart(form)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("could not reach Sarvam: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            // Sarvam errors look like {"error":{"message":"…","code":"…"}}.
            let msg = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or(text);
            return Err(format!("Sarvam {status}: {msg}"));
        }
        let mut t: Transcript = serde_json::from_str(&text).map_err(|e| format!("bad Sarvam reply: {e}"))?;
        t.transcript = hindi_numerals(&t.transcript);
        Ok(t)
    }
}

/* ---------- spoken Hindi numbers -> digits ----------
 * Sarvam writes a spoken "9" in Hindi as "नौ", but the rule engine reads quantities from digits
 * ("नौ मीटर सड़क" fell back to a generic rule; "9 मीटर सड़क" got Rule 56's 3 m). So number words
 * become digits — only where they are clearly quantities, since "दो" is also "give" (बता दो) and
 * "एक" is also "a". */

/// 0–99, in the spellings Sarvam uses plus common variants.
const HI_NUMBERS: &[(&str, f64)] = &[
    ("शून्य", 0.0), ("एक", 1.0), ("दो", 2.0), ("तीन", 3.0), ("चार", 4.0), ("पाँच", 5.0), ("पांच", 5.0),
    ("छह", 6.0), ("छः", 6.0), ("छे", 6.0), ("सात", 7.0), ("आठ", 8.0), ("नौ", 9.0), ("दस", 10.0),
    ("ग्यारह", 11.0), ("बारह", 12.0), ("तेरह", 13.0), ("चौदह", 14.0), ("पंद्रह", 15.0), ("पन्द्रह", 15.0),
    ("सोलह", 16.0), ("सत्रह", 17.0), ("अठारह", 18.0), ("उन्नीस", 19.0), ("बीस", 20.0),
    ("इक्कीस", 21.0), ("बाईस", 22.0), ("तेईस", 23.0), ("चौबीस", 24.0), ("पच्चीस", 25.0), ("छब्बीस", 26.0),
    ("सत्ताईस", 27.0), ("अट्ठाईस", 28.0), ("अठ्ठाईस", 28.0), ("उनतीस", 29.0), ("तीस", 30.0),
    ("इकतीस", 31.0), ("बत्तीस", 32.0), ("तैंतीस", 33.0), ("चौंतीस", 34.0), ("पैंतीस", 35.0), ("छत्तीस", 36.0),
    ("सैंतीस", 37.0), ("अड़तीस", 38.0), ("उनतालीस", 39.0), ("चालीस", 40.0),
    ("इकतालीस", 41.0), ("बयालीस", 42.0), ("तैंतालीस", 43.0), ("चवालीस", 44.0), ("पैंतालीस", 45.0),
    ("छियालीस", 46.0), ("सैंतालीस", 47.0), ("अड़तालीस", 48.0), ("उनचास", 49.0), ("पचास", 50.0),
    ("इक्यावन", 51.0), ("बावन", 52.0), ("तिरपन", 53.0), ("चौवन", 54.0), ("पचपन", 55.0), ("छप्पन", 56.0),
    ("सत्तावन", 57.0), ("अट्ठावन", 58.0), ("उनसठ", 59.0), ("साठ", 60.0),
    ("इकसठ", 61.0), ("बासठ", 62.0), ("तिरसठ", 63.0), ("चौंसठ", 64.0), ("पैंसठ", 65.0), ("छियासठ", 66.0),
    ("सड़सठ", 67.0), ("अड़सठ", 68.0), ("उनहत्तर", 69.0), ("सत्तर", 70.0),
    ("इकहत्तर", 71.0), ("बहत्तर", 72.0), ("तिहत्तर", 73.0), ("चौहत्तर", 74.0), ("पचहत्तर", 75.0),
    ("छिहत्तर", 76.0), ("सतहत्तर", 77.0), ("अठहत्तर", 78.0), ("उन्यासी", 79.0), ("अस्सी", 80.0),
    ("इक्यासी", 81.0), ("बयासी", 82.0), ("तिरासी", 83.0), ("चौरासी", 84.0), ("पचासी", 85.0),
    ("छियासी", 86.0), ("सत्तासी", 87.0), ("अट्ठासी", 88.0), ("नवासी", 89.0), ("नब्बे", 90.0),
    ("इक्यानवे", 91.0), ("बानवे", 92.0), ("तिरानवे", 93.0), ("चौरानवे", 94.0), ("पंचानवे", 95.0),
    ("छियानवे", 96.0), ("सत्तानवे", 97.0), ("अट्ठानवे", 98.0), ("निन्यानवे", 99.0),
    ("डेढ़", 1.5), ("डेढ", 1.5), ("ढाई", 2.5),
];
const HI_SCALES: &[(&str, f64)] = &[("सौ", 100.0), ("हज़ार", 1000.0), ("हजार", 1000.0), ("लाख", 100000.0)];
/// साढ़े 7 = 7.5, सवा 7 = 7.25, पौने 7 = 6.75.
const HI_FRACTIONS: &[(&str, f64)] = &[("साढ़े", 0.5), ("साढे", 0.5), ("सवा", 0.25), ("पौने", -0.25)];
const HI_POINT: &[&str] = &["दशमलव", "पॉइंट", "प्वाइंट"];
/// Words that mark a number as a quantity.
const HI_UNITS: &[&str] = &[
    "मीटर", "मीटरों", "मी", "मिमी", "सेमी", "किमी", "किलोमीटर", "फीट", "फ़ीट", "फुट", "वर्ग", "स्क्वायर",
    "स्क्वेयर", "मंजिल", "मंज़िल", "मंजिला", "मंज़िला", "मंजिलें", "मंज़िलें", "माला", "माले", "फ्लोर",
    "प्रतिशत", "परसेंट", "साल", "वर्ष", "महीने", "महीना", "दिन", "हेक्टेयर", "एकड़", "केवी", "किलोवोल्ट",
    "गुना", "बार", "कमरे", "पेड़", "वाहन", "गाड़ियाँ",
];

fn lookup(table: &[(&str, f64)], w: &str) -> Option<f64> {
    table.iter().find(|(k, _)| *k == w).map(|(_, v)| *v)
}

/// The word without surrounding punctuation, plus that punctuation, so "होगी?" -> ("", "होगी", "?").
fn split_punct(tok: &str) -> (&str, &str, &str) {
    let p = |c: char| matches!(c, '।' | '?' | ',' | '.' | '!' | ';' | ':' | '"' | '\'' | '(' | ')');
    let core = tok.trim_matches(p);
    let start = tok.find(core).unwrap_or(0);
    (&tok[..start], core, &tok[start + core.len()..])
}

fn is_number_word(w: &str) -> bool {
    lookup(HI_NUMBERS, w).is_some() || lookup(HI_SCALES, w).is_some() || lookup(HI_FRACTIONS, w).is_some() || HI_POINT.contains(&w)
}

/// Value of a run of number words: "दो सौ पचास" = 250, "साढ़े सात" = 7.5, "नौ दशमलव पाँच" = 9.5.
fn run_value(words: &[&str]) -> Option<f64> {
    let (mut total, mut cur, mut frac, mut decimals) = (0.0, 0.0, 0.0, None::<String>);
    for w in words {
        if let Some(d) = decimals.as_mut() {
            let n = lookup(HI_NUMBERS, w)?;
            if n.fract() != 0.0 || n > 9.0 {
                return None;
            }
            d.push_str(&(n as u8).to_string());
        } else if HI_POINT.contains(w) {
            decimals = Some(String::new());
        } else if let Some(f) = lookup(HI_FRACTIONS, w) {
            frac = f;
        } else if let Some(n) = lookup(HI_NUMBERS, w) {
            cur += n + frac;
            frac = 0.0;
        } else if let Some(s) = lookup(HI_SCALES, w) {
            let base = if cur == 0.0 { 1.0 + frac } else { cur };
            frac = 0.0;
            if s >= 1000.0 {
                total += base * s;
                cur = 0.0;
            } else {
                cur = base * s;
            }
        }
    }
    let mut v = total + cur;
    if let Some(d) = decimals.filter(|d| !d.is_empty()) {
        v += format!("0.{d}").parse::<f64>().ok()?;
    }
    (v > 0.0 || words.contains(&"शून्य")).then_some(v)
}

fn fmt_number(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Rewrite Hindi number words as digits where they are quantities.
pub fn hindi_numerals(text: &str) -> String {
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let (lead, core, _) = split_punct(toks[i]);
        if !is_number_word(core) {
            out.push(toks[i].to_string());
            i += 1;
            continue;
        }
        // Longest run of number words; punctuation after a word ends the run.
        let mut j = i;
        while j < toks.len() {
            let (l, c, t) = split_punct(toks[j]);
            if !is_number_word(c) || (j > i && !l.is_empty()) {
                break;
            }
            j += 1;
            if !t.is_empty() {
                break;
            }
        }
        let words: Vec<&str> = toks[i..j].iter().map(|t| split_punct(t).1).collect();
        let tail = split_punct(toks[j - 1]).2;
        let next = toks.get(j).map(|t| split_punct(t).1).unwrap_or("");
        let ambiguous = words.len() == 1 && matches!(words[0], "एक" | "दो");
        let quantity = !ambiguous || (tail.is_empty() && HI_UNITS.contains(&next));
        match run_value(&words).filter(|_| quantity) {
            Some(v) => out.push(format!("{lead}{}{tail}", fmt_number(v))),
            None => out.extend(toks[i..j].iter().map(|t| t.to_string())),
        }
        i = j;
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::hindi_numerals as n;

    #[test]
    fn quantities_become_digits() {
        assert_eq!(n("मेरा प्लॉट नौ मीटर सड़क पर है।"), "मेरा प्लॉट 9 मीटर सड़क पर है।");
        assert_eq!(n("दो सौ वर्ग मीटर का प्लॉट"), "200 वर्ग मीटर का प्लॉट");
        assert_eq!(n("दो सौ पचास वर्ग मीटर"), "250 वर्ग मीटर");
        assert_eq!(n("साढ़े सात मीटर ऊँचा"), "7.5 मीटर ऊँचा");
        assert_eq!(n("डेढ़ हज़ार वर्ग मीटर"), "1500 वर्ग मीटर");
        assert_eq!(n("नौ दशमलव पाँच मीटर"), "9.5 मीटर");
        assert_eq!(n("तीन मंजिल का मकान, बारह मीटर सड़क"), "3 मंजिल का मकान, 12 मीटर सड़क");
        assert_eq!(n("दो मंजिला मकान"), "2 मंजिला मकान");
        assert_eq!(n("सड़क बीस?"), "सड़क 20?");
    }

    #[test]
    fn non_quantities_stay_words() {
        assert_eq!(n("मुझे बता दो कितनी जगह चाहिए"), "मुझे बता दो कितनी जगह चाहिए");
        assert_eq!(n("मेरा एक सवाल है"), "मेरा एक सवाल है");
        assert_eq!(n("बता दो।"), "बता दो।");
        assert_eq!(n("My plot is 200 sq m"), "My plot is 200 sq m");
    }
}
