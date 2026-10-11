//! Runs the tool's deterministic core over the provided sample questions.
//! No network: this is exactly the part the jury audits (numbers + abstention).

use bhopal_sahayak::llm::Llm;
use bhopal_sahayak::models::Lang;
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
