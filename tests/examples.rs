//! Every example in `static/examples.json` (the gallery in the UI) is run through the
//! deterministic core and checked against the outcome recorded in its `expect` block.
//! The expectations were checked by hand against the rule tables in docs/3d-catalog.md.

use bhopal_sahayak::llm::Llm;
use bhopal_sahayak::models::Lang;
use bhopal_sahayak::track1::Track1;
use serde_json::Value;

const RULES: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl";

fn examples() -> Vec<Value> {
    serde_json::from_str(&std::fs::read_to_string("static/examples.json").unwrap()).unwrap()
}

async fn run(t: &Track1, ex: &Value) -> Value {
    let lang = if ex["lang"] == "hi" { Lang::Hi } else { Lang::En };
    let a = t.solve(ex["q"].as_str().unwrap(), lang, &Llm::disabled()).await;
    serde_json::to_value(a).unwrap()
}

/// `cargo test --test examples dump -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn dump() {
    let t = Track1::load(RULES).unwrap();
    for ex in examples() {
        let a = run(&t, &ex).await;
        let s = &a["facts"]["scene"];
        let m = |k: &str| s[k]["m"].as_f64().map(|v| v.to_string()).unwrap_or("-".into());
        println!("\n## {} — {}", ex["group"], ex["title"]);
        println!("answer: {}", a["answer"]);
        if s.is_null() {
            println!("NO SCENE");
            continue;
        }
        println!(
            "plot {} fr {} street {} h {} floors {} use {} type {}",
            s["plot_area_sqm"], s["frontage_m"], s["street_width_m"], s["height_m"], s["floors"], s["use_type"], s["building_type"]
        );
        println!("envelope: {} | F {} R {} L {} Rt {} | permitted {}", s["envelope"], m("front"), m("rear"), m("side_left"), m("side_right"), s["permitted"]);
        let comps: Vec<String> = s["components"].as_array().unwrap().iter()
            .map(|c| format!("{}{}", c["id"].as_str().unwrap(), if c["ok"] == true { "" } else { "✗" })).collect();
        println!("components: {}", comps.join(", "));
        for n in s["notes"].as_array().unwrap() {
            println!("  note: {}", n.as_str().unwrap());
        }
        for n in s["info"].as_array().unwrap() {
            println!("  info: {}", n.as_str().unwrap());
        }
        for c in s["components"].as_array().unwrap() {
            for n in c["notes"].as_array().unwrap() {
                println!("  [{}] {}", c["id"].as_str().unwrap(), n.as_str().unwrap());
            }
        }
    }
}

#[tokio::test]
async fn every_example_matches_its_expected_outcome() {
    let t = Track1::load(RULES).unwrap();
    let mut failures = vec![];
    for ex in examples() {
        let title = ex["title"].as_str().unwrap();
        let x = &ex["expect"];
        let a = run(&t, &ex).await;
        let s = &a["facts"]["scene"];
        let mut fail = |what: String| failures.push(format!("{title}: {what}"));
        if a["abstained"] == true {
            fail("abstained".into());
            continue;
        }
        if s.is_null() {
            fail("no 3D scene".into());
            continue;
        }
        if let Some(h) = x["answer_has"].as_str() {
            if !a["answer"].as_str().unwrap().contains(h) {
                fail(format!("answer {:?} lacks {h:?}", a["answer"]));
            }
        }
        if let Some(h) = x["envelope_has"].as_str() {
            if !s["envelope"].as_str().unwrap().contains(h) {
                fail(format!("envelope {:?} lacks {h:?}", s["envelope"]));
            }
        }
        for (k, key) in [("front", "front"), ("rear", "rear")] {
            if let Some(v) = x[k].as_f64() {
                if s[key]["m"].as_f64() != Some(v) {
                    fail(format!("{k} {} != {v}", s[key]["m"]));
                }
            }
        }
        if let Some(sides) = x["sides"].as_array() {
            let got = [s["side_left"]["m"].as_f64(), s["side_right"]["m"].as_f64()];
            let want = [sides[0].as_f64(), sides[1].as_f64()];
            if got != want {
                fail(format!("sides {got:?} != {want:?}"));
            }
        }
        if let Some(p) = x["permitted"].as_bool() {
            if s["permitted"].as_bool() != Some(p) {
                fail(format!("permitted {} != {p}; notes {}", s["permitted"], s["notes"]));
            }
        }
        let comps = s["components"].as_array().unwrap();
        let state = |id: &str| comps.iter().find(|c| c["id"] == id).map(|c| c["ok"] == true);
        for id in x["components"].as_array().into_iter().flatten() {
            if state(id.as_str().unwrap()).is_none() {
                fail(format!("component {id} not drawn"));
            }
        }
        for id in x["failing"].as_array().into_iter().flatten() {
            if state(id.as_str().unwrap()) != Some(false) {
                fail(format!("component {id} should break its rule"));
            }
        }
        if let Some(h) = x["note_has"].as_str() {
            let all = s["notes"].as_array().unwrap().iter().chain(s["info"].as_array().unwrap());
            if !all.clone().any(|n| n.as_str().unwrap().contains(h)) {
                fail(format!("no note containing {h:?}: {} {}", s["notes"], s["info"]));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
