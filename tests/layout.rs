//! Draw plot: hand-drawn layouts checked against the rules.

use bhopal_sahayak::layout::{Check, Layout};
use bhopal_sahayak::track1::Track1;
use serde_json::json;

const RULES: &str = "track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl";

fn check(t: &Track1, v: serde_json::Value) -> Check {
    let l: Layout = serde_json::from_value(v).unwrap();
    t.check_layout(&l)
}

fn errors(c: &Check) -> Vec<String> {
    c.issues.iter().filter(|i| i.level == "error").map(|i| i.text.clone()).collect()
}

/// 12 m wide, 20 m deep (240 sq m), road along the bottom edge (y = 20; edge 2 runs
/// from (12,20) to (0,20)). Plan y grows downwards, like the editor.
fn base(items: serde_json::Value) -> serde_json::Value {
    json!({
        "plot": [[0, 0], [12, 0], [12, 20], [0, 20]],
        "road_edges": [2],
        "street_width": 9,
        "building_type": "detached",
        "items": items
    })
}

#[test]
fn compliant_house_passes() {
    let t = Track1::load(RULES).unwrap();
    // 240 sq m detached, 9 m street: front 3.0, rear 3.0 (>225), sides 3 + 3 (h 6 m, frontage 12 is not < 12).
    let c = check(&t, base(json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 14, "floors": 2 },
        { "id": "S", "kind": "staircase", "x": 4, "y": 5, "w": 2, "d": 3 },
        { "id": "T1", "kind": "tree", "x": 1, "y": 1, "w": 1 },
        { "id": "T2", "kind": "tree", "x": 11, "y": 1, "w": 1 },
        { "id": "R", "kind": "rwh_pit", "x": 1.2, "y": 18.8, "w": 1.2, "d": 1.2 }
    ])));
    assert!(c.ok, "{:#?}", errors(&c));
    let class = |i: usize| c.edges[i].class;
    assert_eq!((class(0), class(1), class(2), class(3)), ("rear", "side", "front", "side"));
    assert_eq!(c.edges[2].required, 3.0);
    assert_eq!(c.edges[0].required, 3.0);
    // Buildable area = (12 - 6) x (20 - 6) = 84 sq m, drawn as a clipped polygon.
    assert!(!c.buildable.is_empty());
}

#[test]
fn building_too_close_to_the_road_and_missing_requirements() {
    let t = Track1::load(RULES).unwrap();
    let c = check(&t, base(json!([
        { "id": "A", "kind": "building", "x": 6, "y": 15, "w": 6, "d": 8, "floors": 2 }
    ])));
    let e = errors(&c);
    assert!(e.iter().any(|x| x.contains("from the front edge") && x.contains("3 m")), "{e:#?}");
    assert!(e.iter().any(|x| x.contains("Rule 81")), "rainwater harvesting required over 140 sq m");
    assert!(e.iter().any(|x| x.contains("trees")), "Rule 67 trees");
    assert!(c.bad.contains(&"A".to_string()));
}

#[test]
fn road_on_any_edge_reclassifies_the_plot() {
    let t = Track1::load(RULES).unwrap();
    let mut v = base(json!([]));
    v["road_edges"] = json!([3]); // left edge
    let c = check(&t, v);
    let class: Vec<_> = c.edges.iter().map(|e| e.class).collect();
    assert_eq!(class, vec!["side", "rear", "side", "front"]);
    assert_eq!(c.frontage, 20.0);
}

#[test]
fn balcony_garage_septic_line_and_lift() {
    let t = Track1::load(RULES).unwrap();
    let c = check(&t, base(json!([
        // 5 floors = 15 m: Rule 42 applies, and a lift is required.
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 4, "d": 6, "floors": 5 },
        // 1.6 m deep balcony on the front face: over 1.2 m.
        { "id": "B", "kind": "balcony", "x": 6, "y": 13.8, "w": 3, "d": 1.6 },
        // Garage in the front open space and smaller than 2.5 x 5.
        { "id": "G", "kind": "garage", "x": 2, "y": 18.5, "w": 2.4, "d": 2.5 },
        // Septic tank 1 m from the building.
        { "id": "P", "kind": "septic_tank", "x": 6, "y": 6.5, "w": 1.5, "d": 0.75 },
        // HT line 1 m from the building's side.
        { "id": "L", "kind": "electric_line", "a": [9, 0], "b": [9, 20], "voltage": "hv" }
    ])));
    let e = errors(&c).join("\n");
    for needle in ["Rule 58(1)(c) allows up to 1.2 m", "Rule 75(a)", "front open space", "Rule 79(1)", "Rule 52 needs 1.2 m", "Rule 83"] {
        assert!(e.contains(needle), "missing {needle:?} in\n{e}");
    }
    assert!(c.envelope.contains("Rule 42"));
}

#[test]
fn l_shaped_plot_and_crossed_outline() {
    let t = Track1::load(RULES).unwrap();
    let mut v = base(json!([]));
    v["plot"] = json!([[0, 0], [20, 0], [20, 10], [10, 10], [10, 25], [0, 25]]);
    v["road_edges"] = json!([4]);
    let c = check(&t, v);
    assert!(c.buildable.is_empty(), "no single buildable polygon for a non-convex plot");
    assert!(c.issues.iter().any(|i| i.text.contains("not convex")));

    let mut v = base(json!([]));
    v["plot"] = json!([[0, 0], [10, 10], [10, 0], [0, 10]]);
    assert!(errors(&check(&t, v)).iter().any(|x| x.contains("crosses itself")));
}

/// Same 12 x 20 plot; the road (bottom edge) is at 500 m above sea level.
fn levelled(levels: [f64; 4], items: serde_json::Value) -> serde_json::Value {
    let mut v = base(items);
    v["road_level"] = json!(500.0);
    v["plot_levels"] = json!(levels);
    v
}

#[test]
fn flat_plot_above_the_road_lowers_the_wall() {
    let t = Track1::load(RULES).unwrap();
    // Plot level at 501 m, 1 m above the road; flat, so heights start from the plot.
    let mut v = levelled([501.0; 4], json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 14, "floors": 4, "plinth": 0.5 }
    ]));
    v["wall"] = json!({ "front_h": 1.5, "other_h": 2.0 });
    let c = check(&t, v);
    assert!(!c.sloping);
    assert_eq!(c.reference_level, 501.0);
    // 0.5 plinth + 4 x 3 m = 12.5 m: still Rule 56.
    assert_eq!(c.levels[0].rule_height, 12.5);
    assert!(c.envelope.contains("Rule 56"), "{}", c.envelope);
    let e = errors(&c).join("\n");
    // Walls are measured above the street centre line (Rule 78): 1 m of that is ground.
    assert!(e.contains("Front wall on edge 3 rises 2.5 m") && e.contains("at most 0.5 m"), "{e}");
    assert!(e.contains("Side / rear wall") && e.contains("at most 1 m"), "{e}");
}

#[test]
fn sloping_plot_measures_from_the_higher_reference() {
    let t = Track1::load(RULES).unwrap();
    // Rises 3 m from the road (y = 20) to the rear (y = 0): 1:6.7, steeper than 1:20.
    let c = check(&t, levelled([503.0, 503.0, 500.0, 500.0], json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 12, "floors": 4, "plinth": 0.6 }
    ])));
    assert!(c.sloping);
    // Higher of road 500 and the average plot level 501.5.
    assert_eq!(c.reference_level, 501.5);
    // Highest ground under the building is at y = 4: 500 + 3 x 16/20 = 502.4.
    assert_eq!(c.levels[0].ground, 502.4);
    // Roof 502.4 + 0.6 + 12 = 515.0; 515.0 - 501.5 = 13.5 m > 12.5: Rule 42 and a lift.
    assert_eq!(c.levels[0].rule_height, 13.5);
    assert!(c.envelope.contains("Rule 42"), "{}", c.envelope);
    assert!(errors(&c).iter().any(|x| x.contains("Rule 83")));
    assert!(c.issues.iter().any(|i| i.text.contains("Rule 16(4)(j)")), "contours on a 1:7 slope");
}

#[test]
fn plinth_and_sunken_plot() {
    let t = Track1::load(RULES).unwrap();
    // Plot 1.2 m below the road.
    let c = check(&t, levelled([498.8; 4], json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 14, "floors": 2, "plinth": 0.3 },
        { "id": "B", "kind": "building", "x": 6, "y": 10, "w": 1, "d": 1, "floors": 1, "plinth": 0.6, "basement": true }
    ])));
    let e = errors(&c).join("\n");
    assert!(e.contains("Rule 68(1) needs more than 0.45 m"), "{e}");
    assert!(e.contains("Rule 76(2)(b)"), "{e}");
    assert!(c.issues.iter().any(|i| i.text.contains("below the road centre line")));
}

fn infos(c: &Check) -> Vec<String> {
    c.issues.iter().filter(|i| i.level == "info").map(|i| i.text.clone()).collect()
}

#[test]
fn custom_blocks_are_read_by_name() {
    let t = Track1::load(RULES).unwrap();
    let c = check(&t, base(json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 12, "floors": 2, "parapet": 1.05 },
        // "Car shed" -> garage, smaller than 2.5 x 5.
        { "id": "C1", "kind": "custom", "name": "Car shed", "x": 1.5, "y": 6, "w": 2.4, "d": 4.5 },
        // Same block with checking off: drawn, not checked.
        { "id": "C2", "kind": "custom", "name": "Car shed", "x": 10.5, "y": 6, "w": 2.4, "d": 4.5, "check_rules": false },
        // "Watchman cabin" -> watchman hut; 9 sq m in the front open space is over 6 sq m.
        { "id": "C3", "kind": "custom", "name": "Watchman cabin", "x": 9, "y": 18.5, "w": 3, "d": 3 },
        // No rule names a swimming pool.
        { "id": "C4", "kind": "custom", "name": "Swimming pool", "x": 6, "y": 1.5, "w": 4, "d": 2 }
    ])));
    let e = errors(&c).join("\n");
    let i = infos(&c).join("\n");
    assert!(e.contains("Car shed is 2.4 × 4.5 m; Rule 75(a)"), "{e}");
    assert_eq!(e.matches("Rule 75(a)").count(), 1, "the unchecked copy adds no error\n{e}");
    assert!(i.contains("‘Car shed’ is read as Garage"), "{i}");
    assert!(i.contains("rule checking is off"), "{i}");
    assert!(e.contains("Rule 58(4)(c) allows up to 6 sq m"), "{e}");
    assert!(i.contains("No rule names ‘Swimming pool’"), "{i}");
    let ack = |id: &str| c.custom.iter().find(|a| a.id == id).unwrap().clone();
    assert_eq!(ack("C1").as_kind.as_deref(), Some("garage"));
    assert!(ack("C1").checked && !ack("C2").checked);
    assert_eq!(ack("C4").as_kind, None);
}

#[test]
fn heights_and_slopes_of_components() {
    let t = Track1::load(RULES).unwrap();
    let c = check(&t, base(json!([
        // 2.7 m floor to floor leaves 2.55 m under a 0.15 m slab: under Rule 69's 2.6 m.
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 12, "floors": 2, "floor_height": 2.7, "parapet": 0.9 },
        // 30° roof on a 6 m span: ridge 3 x tan 30° = 1.73 m.
        { "id": "R", "kind": "building", "x": 10.5, "y": 4.5, "w": 0.0, "d": 0.0, "floors": 1 },
        { "id": "B", "kind": "balcony", "x": 6, "y": 16.6, "w": 2, "d": 1.2, "on_floor": 0 },
        { "id": "G", "kind": "garage", "x": 1.5, "y": 6, "w": 2.5, "d": 5, "h": 2.0 },
        { "id": "P", "kind": "septic_tank", "x": 6, "y": 1.5, "w": 1.5, "d": 0.75, "depth": 0.8 }
    ])));
    let e = errors(&c).join("\n");
    for needle in ["Rule 69 needs 2.6 m", "Rule 77 needs at least 1.05 m", "first or higher floors", "Rule 75(1) needs at least 2.2 m", "Rule 79(2)(a) needs at least 1 m"] {
        assert!(e.contains(needle), "missing {needle:?}\n{e}");
    }

    let c = check(&t, base(json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 12, "floors": 2, "roof_pitch": 30 }
    ])));
    // 0.5 plinth + 6 m storeys + 1.73 m ridge.
    assert_eq!(c.levels[0].rule_height, 8.23);
    let c = check(&t, base(json!([
        { "id": "A", "kind": "building", "x": 6, "y": 10, "w": 6, "d": 12, "floors": 2, "parapet": 1.8 }
    ])));
    // A 1.8 m parapet is over the 1.5 m that Rule 2(9) leaves out, so it counts.
    assert_eq!(c.levels[0].rule_height, 8.3);
    assert!(infos(&c).iter().any(|x| x.contains("counts in building height")));
}
