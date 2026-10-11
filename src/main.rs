use bhopal_sahayak::{llm, models, track1, track2};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use models::{Answer, AskRequest, GhatsResponse, Lang};
use std::sync::Arc;
use tower_http::services::ServeDir;

const RULES: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl";
const GHATS: &str = "track-2-visarjan-ghat-watch/data/bhopal_ghats_2023_2025.csv";

struct App {
    track1: track1::Track1,
    ghats: GhatsResponse,
    llm: llm::Llm,
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

    let track1 = track1::Track1::load(RULES).expect("load rules jsonl");
    let track2 = track2::Track2::load(GHATS).expect("load ghats csv");
    let ghats = track2.rank(&llm).await;
    println!("Loaded {} rules, {} readings across {} ghats.", track1.corpus.rules.len(), 1524, ghats.ghats.len());

    let app = Arc::new(App { track1, ghats, llm });

    let router = Router::new()
        .route("/api/ask", post(ask))
        .route("/api/ghats", get(ghats_handler))
        .route("/api/scene", get(scene_handler))
        .route("/api/catalog", get(catalog_handler))
        .route("/api/check", post(check_handler))
        .route("/api/health", get(|| async { "ok" }))
        .fallback_service(ServeDir::new("static"))
        .with_state(app);

    let port = port_from_args();
    let addr = format!("127.0.0.1:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Could not bind {addr}: {e}. Try another port: cargo run -- --port 8081");
            std::process::exit(1);
        }
    };
    println!("Open http://{addr}");
    axum::serve(listener, router).await.unwrap();
}

/// `--port <n>` / `--port=<n>` / `-p <n>`, default 8080.
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
    8080
}

async fn ghats_handler(State(app): State<Arc<App>>) -> Json<GhatsResponse> {
    Json(app.ghats.clone())
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
        "bhumi-vikas" | "track1" | "1" => app.track1.solve(q, lang, &app.llm).await,
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
