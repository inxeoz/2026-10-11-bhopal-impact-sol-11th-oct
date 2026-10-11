//! Runs the tool's deterministic core over the provided sample questions.
//! No network: this is exactly the part the jury audits (numbers + abstention).

use bhopal_sahayak::llm::Llm;
use bhopal_sahayak::models::Lang;
use bhopal_sahayak::scene::SceneInput;
use bhopal_sahayak::track1::Track1;
use bhopal_sahayak::track2::{classify, Track2};
use bhopal_sahayak::models::Verdict;

const RULES: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl";
const GHATS: &str = "track-2-visarjan-ghat-watch/data/bhopal_ghats_2023_2025.csv";

#[tokio::test]
async fn front_open_space_q1() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    let a = t
        .solve(
            "My house faces a 10 m wide street and will be 9 m tall. How much open space do I leave at the front?",
            Lang::En,
            &llm,
        )
        .await;
    assert!(!a.abstained, "Q1 should be answered");
    assert_eq!(a.facts["front_open_space_m"], "3.6", "10 m street -> 3.6 m");
    assert!(a.sources[0].label.contains("56(1)(a)"));
}

#[tokio::test]
async fn rear_open_space_q10() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    let a = t
        .solve(
            "What is the minimum rear open space for a 150 sq m residential plot?",
            Lang::En,
            &llm,
        )
        .await;
    assert!(!a.abstained);
    assert_eq!(a.facts["rear_open_space_m"], "1.50", "150 sq m -> 1.50 m");
}

#[tokio::test]
async fn duration_rule23() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    let a = t
        .solve(
            "My development permission was granted in 2023. How long does it stay valid, and how many times can I renew it?",
            Lang::En,
            &llm,
        )
        .await;
    assert!(!a.abstained);
    assert_eq!(a.facts["rule"], "23");
    assert!(a.sources[0].quote.to_lowercase().contains("three years"));
}

#[tokio::test]
async fn deemed_sanction_q4() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    let a = t
        .solve(
            "I applied for building permission and the authority has not replied. What happens if they miss the deadline?",
            Lang::En,
            &llm,
        )
        .await;
    assert!(!a.abstained);
    assert_eq!(a.facts["rule"], "28");
    assert!(a.sources[0].quote.to_lowercase().contains("deemed"));
}

#[tokio::test]
async fn out_of_scope_abstains_q16_q17() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    for q in [
        "How do I pay my property tax online?",
        "What is the current market rate for land in Kolar Road, Bhopal?",
    ] {
        let a = t.solve(q, Lang::En, &llm).await;
        assert!(a.abstained, "should abstain: {q}");
    }
}

#[tokio::test]
async fn scene_built_from_question() {
    let t = Track1::load(RULES).unwrap();
    let a = t
        .solve(
            "My house faces a 10 m wide street and will be 9 m tall. How much open space do I leave at the front?",
            Lang::En,
            &Llm::disabled(),
        )
        .await;
    let s = &a.facts["scene"];
    assert_eq!(s["street_width_m"], 10.0);
    assert_eq!(s["height_m"], 9.0);
    assert_eq!(s["front"]["m"], 3.6, "same number as the answer");
    assert!(s["assumed"].to_string().contains("plot area"), "plot area was not given");
}

#[test]
fn scene_side_open_spaces() {
    let t = Track1::load(RULES).unwrap();
    let scene = |kind: &str, height: f64, frontage: f64| {
        t.scene(&SceneInput {
            plot_area: Some(200.0),
            frontage: Some(frontage),
            street_width: Some(9.0),
            height: Some(height),
            building_type: Some(kind.into()),
            ..Default::default()
        })
    };
    // Detached, both sides 3 m; proviso cuts one to 1.5 m (<= 7 m tall, frontage < 12 m).
    let s = scene("detached", 9.0, 11.0);
    assert_eq!((s.side_left.unwrap().m, s.side_right.unwrap().m), (3.0, 3.0));
    let s = scene("detached", 6.0, 11.0);
    assert_eq!((s.side_left.unwrap().m, s.side_right.unwrap().m), (3.0, 1.5));
    // Semi-detached: one side 3 m, or 2.5 m up to 10 m tall on frontage up to 10 m.
    let s = scene("semi-detached", 9.0, 10.0);
    assert_eq!(s.side_left.unwrap().m, 2.5);
    assert!(s.side_right.is_none());
    // Row-type: no side open space.
    assert!(scene("row", 9.0, 10.0).side_left.is_none());
    // Rear for 200 sq m and front for 9 m street, from the same tables as the answers.
    let s = scene("row", 9.0, 10.0);
    assert_eq!((s.front.unwrap().m, s.rear.unwrap().m), (3.0, 2.5));
    assert_eq!(s.open_space_limit_sqm, 10.0 * (20.0 - 3.0 - 2.5));
    // 3 floors x 145 sq m would break FAR 1.25 (250 sq m): the footprint drawn is 250 / 3.
    assert!((s.footprint_sqm - 250.0 / 3.0).abs() < 0.1, "{}", s.footprint_sqm);
}

#[test]
fn every_fact_reads_from_the_rule_text() {
    let t = Track1::load(RULES).unwrap();
    assert!(t.facts.missing.is_empty(), "facts not found in rule text: {:?}", t.facts.missing);
    let v = |id: &str| t.facts.v(id).unwrap();
    // Spot-check values against the PDF.
    assert_eq!(v("proj.balcony"), 1.2);
    assert_eq!(v("proj.balcony_clear"), 1.5);
    assert_eq!(v("parapet.h"), 1.05);
    assert_eq!(v("wall.front"), 1.5);
    assert_eq!(v("stair.w.res"), 0.85);
    assert_eq!(v("stair.riser.res_cm"), 17.0);
    assert_eq!(v("lift.above"), 12.5);
    assert_eq!(v("line.hv.v"), 3.7);
    assert_eq!(v("occ.ind.step"), 0.25);
    assert_eq!(v("far.res"), 1.25);
    assert_eq!(t.facts.shaft(12.0).map(|(a, s, _)| (a, s)), Some((2.8, 1.2)));
}

#[test]
fn rule42_tables_read_row_by_row() {
    let t = Track1::load(RULES).unwrap();
    let rows: Vec<_> = t
        .facts
        .rule42
        .iter()
        .map(|r| (r.table, r.road_min, r.plot_min, r.far, r.height_cap, r.front, r.other))
        .collect();
    assert_eq!(
        rows,
        vec![
            (4, 12.0, 1000.0, 1.5, Some(18.0), 7.5, 6.0),
            (4, 18.0, 1500.0, 1.75, Some(24.0), 9.0, 6.0),
            (4, 24.0, 2000.0, 2.0, Some(30.0), 12.0, 7.5),
            (5, 30.0, 2500.0, 2.25, Some(45.0), 15.0, 7.5),
            (5, 36.0, 3000.0, 2.5, Some(60.0), 18.0, 9.0),
            (5, 45.0, 3500.0, 2.75, Some(75.0), 21.0, 9.0),
            (5, 60.0, 4000.0, 3.0, Some(90.0), 24.0, 10.0),
            (5, 75.0, 4500.0, 3.0, None, 30.0, 12.0),
        ]
    );
}

#[test]
fn scene_above_12_5_m_uses_rule_42() {
    let t = Track1::load(RULES).unwrap();
    // 15 m on a 9 m road: Table 4 row 1 open spaces, but the road is too narrow.
    let s = t.scene(&SceneInput { height: Some(15.0), ..Default::default() });
    assert!(s.covered);
    assert_eq!((s.front.as_ref().unwrap().m, s.rear.as_ref().unwrap().m), (7.5, 6.0));
    assert!(!s.permitted);
    assert!(s.notes.iter().any(|n| n.contains("road width of at least 12")));
    assert!(s.suggested.iter().any(|x| x.id == "lift"), "lift is required above 12.5 m");
}

#[test]
fn scene_other_occupancies_rule_57() {
    let t = Track1::load(RULES).unwrap();
    let scene = |u: &str, h: f64| {
        t.scene(&SceneInput { use_type: Some(u.into()), height: Some(h), plot_area: Some(2000.0), ..Default::default() })
    };
    let s = scene("assembly", 9.0);
    assert_eq!((s.front.unwrap().m, s.rear.unwrap().m), (12.0, 6.0));
    let s = scene("business", 9.0);
    assert_eq!((s.front.unwrap().m, s.side_left.unwrap().m), (6.0, 4.5));
    // Industrial above 12.5 m: Rule 42 row 1 (7.5 / 6.0) vs Rule 57(e) 4.5 + 0.25 x ceil(18-16) = 5.0.
    let s = scene("industrial", 18.0);
    assert_eq!((s.front.unwrap().m, s.side_left.unwrap().m), (7.5, 6.0));
    let s = scene("farm_house", 7.0);
    assert_eq!(s.front.unwrap().m, 7.5);
    // FAR 0.10 on 2000 sq m = 200 sq m over 2 floors: the drawn footprint shrinks to fit.
    let far = s.far.unwrap();
    assert!(far.ok && far.proposed_sqm <= far.allowed_sqm + 0.1, "{far:?}");
    assert_eq!(s.roof, "sloping");
}

#[tokio::test]
async fn tall_building_answer_matches_the_3d_scene() {
    let t = Track1::load(RULES).unwrap();
    let a = t
        .solve(
            "Commercial office building 24 m tall on a 1600 sq m plot, 40 m frontage, on an 18 m road. What open space at the front?",
            Lang::En,
            &Llm::disabled(),
        )
        .await;
    // Not Rule 56 (residential up to 12.5 m): Rule 42 Table 4 row 2 says 9 m front, 6 m others.
    assert_eq!(a.facts["front_open_space_m"], 9.0);
    assert_eq!(a.facts["scene"]["front"]["m"], 9.0);
    assert!(a.sources[0].label.contains("Rule 42"));
}

#[test]
fn numbers_take_the_nearest_context_word() {
    let n = bhopal_sahayak::track1::Numbers::parse("24 m tall office on a 1600 sq m plot, 40 m frontage, on an 18 m road");
    assert_eq!((n.height, n.frontage, n.street_width, n.plot_area), (Some(24.0), Some(40.0), Some(18.0), Some(1600.0)));
    assert_eq!(bhopal_sahayak::track1::Numbers::parse("45 m residential tower on 4000 sq m").height, Some(45.0));
    // Hindi: no panic on byte windows, and the road width is found.
    let n = bhopal_sahayak::track1::Numbers::parse("मेरा प्लॉट 200 वर्ग मीटर का है और सामने 9 मीटर चौड़ी सड़क है");
    assert_eq!((n.plot_area, n.street_width), (Some(200.0), Some(9.0)));
}

#[tokio::test]
async fn components_from_a_dynamic_question() {
    let t = Track1::load(RULES).unwrap();
    let a = t
        .solve(
            "G+2 house on 180 sq m plot facing a 12 m road with a balcony, staircase, garage and an 11 kV line in front",
            Lang::En,
            &Llm::disabled(),
        )
        .await;
    let s = &a.facts["scene"];
    let ids: Vec<&str> = s["components"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    for id in ["balcony", "staircase", "garage", "electric_line"] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }
    let c = |id: &str| s["components"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap().clone();
    // Front 3.6 m on a 12 m road; balcony = min(1.2, 3.6 - 1.5).
    assert_eq!(c("balcony")["params"]["depth"], 1.2);
    assert_eq!(c("electric_line")["params"]["class"], "hv");
    assert_eq!(c("staircase")["params"]["width"], 0.85);
    assert_eq!(s["height_m"], 9.0);
}

#[test]
fn track2_classification() {
    assert_eq!(classify("Dissolved oxygen", Some(0.0), "", "0"), Verdict::Fail);
    assert_eq!(classify("Dissolved oxygen", Some(6.0), "", "6.0"), Verdict::Pass);
    assert_eq!(classify("BOD", Some(10.0), "", "10"), Verdict::Fail);
    assert_eq!(classify("BOD", Some(2.0), "", "2.0"), Verdict::Pass);
    assert_eq!(classify("pH", Some(7.0), "", "7.0"), Verdict::Pass);
    assert_eq!(classify("pH", Some(9.5), "", "9.5"), Verdict::Fail);
    // BDL is low, not missing.
    assert_eq!(classify("Dissolved oxygen", None, "BDL", "BDL"), Verdict::Low);
    // Blank / not sampled is missing, never clean.
    assert_eq!(
        classify("Dissolved oxygen", None, "not sampled", ""),
        Verdict::Missing
    );
}

#[tokio::test]
async fn track2_bairagarh_ranks_top() {
    let t = Track2::load(GHATS).unwrap();
    let llm = Llm::disabled();
    let r = t.rank(&llm).await;
    assert_eq!(r.ghats.len(), 7, "seven Bhopal ghats");
    assert_eq!(
        r.ghats[0].ghat, "Bairagarh (Sant Hirdaram) kund",
        "zero-oxygen kund should rank first"
    );
    // The headline evidence must be a real traceable page.
    assert!(
        r.ghats[0].evidence.iter().any(|e| e.verdict == Verdict::Fail),
        "top ghat should have a failed reading on file"
    );
}

/// Run every sample question and print the outcome. `cargo test report -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn report_all_samples() {
    let t = Track1::load(RULES).unwrap();
    let llm = Llm::disabled();
    let qs = [
        "My house faces a 10 m wide street and will be 9 m tall. How much open space do I leave at the front?",
        "I want a farm house on agricultural land. What is the minimum plot size and the maximum FAR?",
        "My development permission was granted in 2023. How long does it stay valid, and how many times can I renew it?",
        "I applied for building permission and the authority has not replied. What happens if they miss the deadline?",
        "How far must my balcony stay from a low-voltage overhead electric line, vertically and horizontally?",
        "Who is allowed to sign and submit my building plans?",
        "What documents go with an application for building permission?",
        "What is the fee for building permission for a residential house?",
        "Can I change my plan after it is sanctioned?",
        "What is the minimum rear open space for a 150 sq m residential plot?",
        "Which rule decides what happens when the Development Plan and these rules disagree?",
        "What are the rules for lifts in a building taller than 15 m?",
        "मेरा प्लॉट 200 वर्ग मीटर का है और सामने 9 मीटर चौड़ी सड़क है। आगे कितनी जगह खाली छोड़नी होगी?",
        "बिल्डिंग परमिशन की फीस कितनी है और किस नियम में लिखी है?",
        "How do I pay my property tax online?",
        "What is the current market rate for land in Kolar Road, Bhopal?",
    ];
    let mut ok = 0;
    for (i, q) in qs.iter().enumerate() {
        let a = t.solve(q, Lang::En, &llm).await;
        if !a.abstained { ok += 1; }
        println!("{:2}. [{}] {}", i + 1, if a.abstained { "ABSTAIN" } else { "ANSWER " }, q);
        println!("      -> {}", a.answer);
        if let Some(s) = a.sources.first() {
            println!("      src: {} | {}", s.label, s.cite);
        }
    }
    println!("\nanswered {ok}/{} (last two are meant to abstain)", qs.len());
}

#[test]
fn plot_level_against_the_road() {
    use bhopal_sahayak::scene::parse_rise;
    assert_eq!(parse_rise("my plot is 1.5 m above the road"), Some(1.5));
    assert_eq!(parse_rise("The land sits 2 metres below the street"), Some(-2.0));
    assert_eq!(parse_rise("the road is 0.8 m higher than my plot"), Some(-0.8));
    assert_eq!(parse_rise("road at 500 m above sea level and plot at 501.2 m above sea level"), Some(1.2));
    assert_eq!(parse_rise("प्लॉट सड़क से 1 मीटर नीचे है"), Some(-1.0));
    assert_eq!(parse_rise("9 m tall house on a 12 m road"), None);
}

#[tokio::test]
async fn raised_plot_lowers_the_boundary_wall() {
    let t = Track1::load(RULES).unwrap();
    let a = t
        .solve(
            "My 200 sq m plot is 1.2 m above a 9 m road. 9 m tall house with a boundary wall. What front open space?",
            Lang::En,
            &Llm::disabled(),
        )
        .await;
    let s = &a.facts["scene"];
    // 1.2 is the level, not the road width.
    assert_eq!(s["street_width_m"], 9.0);
    assert_eq!(s["plot_rise_m"], 1.2);
    let wall = s["components"].as_array().unwrap().iter().find(|c| c["id"] == "boundary_wall").unwrap();
    // Rule 78 heights are above the street centre line: 1.5 - 1.2 and 2.0 - 1.2.
    assert_eq!(wall["params"]["front_h"], 0.3);
    assert_eq!(wall["params"]["other_h"], 0.8);
}
