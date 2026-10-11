use bhopal_sahayak::{llm, models, stt, track1, track2};

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use models::{Answer, AskRequest, GhatsResponse, Lang};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

const RULES: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl";
const GHATS: &str = "track-2-visarjan-ghat-watch/data/bhopal_ghats_2023_2025.csv";
/// The rulebook text, page by page (the same pdftotext output the rules JSONL was split from).
const RULES_TEXT: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules_fulltext_by_page.txt";
const RULES_PDF: &str = "track-1-bhumi-vikas-sahayak/resources/MP_Bhumi_Vikas_Rules_2012_upto_2025-03-06_EN.pdf";

struct App {
    track1: track1::Track1,
    ghats: GhatsResponse,
    llm: llm::Llm,
    stt: stt::Stt,
    /// GET /api/doc, built once.
    doc: serde_json::Value,
}

#[tokio::main]
async fn main() {
    let llm = llm::Llm::from_env();
    println!(
        "Cerebras: {}",
        if llm.is_available() {
            "key loaded — language steps live"
        } else {
            "no CEREBRASAPI key — deterministic mode only"
        }
    );

    let stt = stt::Stt::from_env();
    println!(
        "Sarvam: {}",
        if stt.is_available() { "key loaded — voice input on" } else { "no SARVAMAPI key — voice input off" }
    );

    let track1 = track1::Track1::load(RULES).expect("load rules jsonl");
    let track2 = track2::Track2::load(GHATS).expect("load ghats csv");
    let ghats = track2.rank(&llm).await;
    println!("Loaded {} rules, {} readings across {} ghats.", track1.corpus.rules.len(), 1524, ghats.ghats.len());

    let doc = load_doc(&track1);
    let app = Arc::new(App { track1, ghats, llm, stt, doc });

    let router = Router::new()
        .route("/api/ask", post(ask))
        .route("/api/transcribe", post(transcribe))
        .route("/api/voice", get(voice_handler))
        .route("/api/ghats", get(ghats_handler))
        .route("/api/scene", get(scene_handler))
        .route("/api/catalog", get(catalog_handler))
        .route("/api/check", post(check_handler))
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/doc", get(doc_handler))
        .route_service("/doc/rules-en.pdf", ServeFile::new(RULES_PDF))
        .fallback_service(ServeDir::new("static"))
        .with_state(app);

    let port = port_from_args();
    let addr = format!("0.0.0.0:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not bind {addr}: {e}. Try another port: cargo run -- --port 8081");
            std::process::exit(1);
        }
    };
    println!("Listening on {addr}. Open http://127.0.0.1:{port}");
    axum::serve(listener, router).await.unwrap();
}

/// `--port <n>` / `--port=<n>` / `-p <n>`, else the `PORT` env var (set by hosts), default 8080.
fn port_from_args() -> u16 {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let val = match a.as_str() {
            "--port" | "-p" => args.next(),
            _ => a.strip_prefix("--port=").map(str::to_string),
        };
        if let Some(v) = val {
            match v.parse() {
                Ok(p) => return p,
                Err(_) => {
                    eprintln!("Invalid port: {v}");
                    std::process::exit(1);
                }
            }
        }
    }
    std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080)
}

async fn ghats_handler(State(app): State<Arc<App>>) -> Json<GhatsResponse> {
    Json(app.ghats.clone())
}

/// The Document viewer: every page of the rulebook text, and an index of rules by page.
fn load_doc(t: &track1::Track1) -> serde_json::Value {
    let text = std::fs::read_to_string(RULES_TEXT).unwrap_or_default();
    let mut pages = vec![];
    let mut cur: Option<(u32, String)> = None;
    for line in text.lines() {
        let marker = line
            .strip_prefix("===== PDF page ")
            .and_then(|r| r.strip_suffix(" ====="))
            .and_then(|n| n.trim().parse::<u32>().ok());
        match marker {
            Some(n) => {
                if let Some((p, body)) = cur.take() {
                    pages.push(serde_json::json!({ "page": p, "text": body.trim_end() }));
                }
                cur = Some((n, String::new()));
            }
            None => {
                if let Some((_, body)) = cur.as_mut() {
                    body.push_str(line);
                    body.push('\n');
                }
            }
        }
    }
    if let Some((p, body)) = cur {
        pages.push(serde_json::json!({ "page": p, "text": body.trim_end() }));
    }
    let rules: Vec<_> = t
        .corpus
        .rules
        .iter()
        .map(|r| serde_json::json!({ "rule": r.rule, "title": r.title, "page": r.page_start }))
        .collect();
    println!("Document viewer: {} pages.", pages.len());
    serde_json::json!({
        "title": "M.P. Bhumi Vikas Rules, 2012 (consolidated to 6 March 2025)",
        "pdf": "/doc/rules-en.pdf",
        "pages": pages,
        "rules": rules,
    })
}

async fn doc_handler(State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    Json(app.doc.clone())
}

/// Draw plot: check a hand-drawn layout against the rules.
async fn check_handler(
    State(app): State<Arc<App>>,
    Json(layout): Json<bhopal_sahayak::layout::Layout>,
) -> Json<bhopal_sahayak::layout::Check> {
    Json(app.track1.check_layout(&layout))
}

/// The component palette: every catalog entry, drawn or not, plus any fact that failed to read.
async fn catalog_handler(State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "components": bhopal_sahayak::catalog::CATALOG,
        "missing_facts": app.track1.facts.missing,
    }))
}

/// The 3D view's sliders call this; open spaces are recomputed from Rule 56 each time.
async fn scene_handler(
    State(app): State<Arc<App>>,
    Query(input): Query<bhopal_sahayak::scene::SceneInput>,
) -> Json<bhopal_sahayak::scene::Scene> {
    Json(app.track1.scene(&input))
}

/// Whether the mic button should show: only when the server holds a Sarvam key.
async fn voice_handler(State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "available": app.stt.is_available() }))
}

/// The mic button posts its raw recording here (Content-Type = the recording's type).
async fn transcribe(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    audio: Bytes,
) -> Result<Json<stt::Transcript>, (StatusCode, String)> {
    if !app.stt.is_available() {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "voice input is off: no SARVAMAPI key on the server".into()));
    }
    if audio.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "no audio received".into()));
    }
    let mime = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("audio/webm");
    app.stt
        .transcribe(audio.to_vec(), mime)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))
}

async fn ask(
    State(app): State<Arc<App>>,
    Json(req): Json<AskRequest>,
) -> Result<Json<Answer>, (StatusCode, String)> {
    let lang = req.lang.unwrap_or(Lang::En);
    let q = req.question.trim();
    if q.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty question".into()));
    }
    let ans = match req.track.as_str() {
        "bhumi-vikas" | "track1" | "1" => app.track1.solve_with(q, lang, &app.llm, &req.details).await,
        "visarjan" | "track2" | "2" => track2_answer(&app, q, lang),
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown track: {other}"))),
    };
    Ok(Json(ans))
}

/// Track 2 is a lookup over a fixed ranking, not free generation.
fn track2_answer(app: &App, question: &str, lang: Lang) -> Answer {
    let q = question.to_lowercase();
    let hit = app
        .ghats
        .ghats
        .iter()
        .find(|g| q.contains(&g.ghat.to_lowercase()) || g.ghat.to_lowercase().split_whitespace().any(|w| w.len() > 3 && q.contains(w)));

    let g = match hit {
        Some(g) => g,
        None => &app.ghats.ghats[0],
    };

    let answer = match lang {
        Lang::En => format!("Rank {} — {} — risk: {}.", g.rank, g.ghat, g.tier),
        Lang::Hi => format!("क्रम {} — {} — जोखिम: {}।", g.rank, g.ghat, g.tier),
    };
    let detail = g.reason.clone();
    let sources = g
        .evidence
        .iter()
        .map(|e| models::Source {
            label: format!("{} · {}", e.parameter, e.value),
            cite: e.cite.clone(),
            quote: format!("{} — {} ({:?})", e.parameter, e.value, e.verdict).to_lowercase(),
        })
        .collect();

    Answer {
        track: "visarjan".into(),
        question: question.into(),
        lang: lang.code().into(),
        answer,
        detail,
        abstained: false,
        sources,
        note: app.ghats.note.clone(),
        facts: serde_json::json!({ "score": g.score, "tier": g.tier, "water_body": g.water_body }),
    }
}
