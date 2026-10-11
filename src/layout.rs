//! "Draw plot": checks a layout the user drew (any plot outline, blocks placed by hand)
//! against the rules.
//!
//! The open spaces, FAR and envelope come from the same scene logic the 3D answers use
//! (`scene.rs`), fed with the drawn plot's area, road frontage and tallest building. Each
//! plot edge is then classed front (road), rear (facing away from the road) or side, and
//! every block is measured against the edges. All rule numbers come from `Facts`.
//!
//! Plan coordinates are metres; any orientation works (edges are classed by normals).

use crate::facts::Fact;
use crate::models::Source;
use crate::scene::{SceneInput, Setback};
use crate::track1::Track1;
use serde::{Deserialize, Serialize};

type P = [f64; 2];

/// Floor-to-floor height used to turn a floor count into a height (stated in the UI).
const FLOOR_HEIGHT: f64 = 3.0;
/// Slab thickness assumed when checking clear room height from floor-to-floor height (stated in the UI).
const SLAB: f64 = 0.15;
/// Plinth when none is given; Rule 68(1) wants more than 0.45 m (stated in the UI).
const DEFAULT_PLINTH: f64 = 0.5;
/// Corner levels closer than this count as a flat site.
const FLAT_TOLERANCE: f64 = 0.05;

#[derive(Deserialize, Debug, Clone)]
pub struct Layout {
    /// Plot outline, in order (either direction).
    pub plot: Vec<P>,
    /// Edge i runs from plot[i] to plot[i+1]; these edges face the road.
    #[serde(default)]
    pub road_edges: Vec<usize>,
    pub street_width: f64,
    #[serde(default)]
    pub use_type: Option<String>,
    #[serde(default)]
    pub building_type: Option<String>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub wall: Option<Wall>,
    /// Road centre-line level, metres above sea level. Defaults to 0 (a flat, level site).
    #[serde(default)]
    pub road_level: Option<f64>,
    /// Ground level at each plot corner (same order as `plot`). Missing: level with the road.
    #[serde(default)]
    pub plot_levels: Vec<f64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Wall {
    pub front_h: f64,
    pub other_h: f64,
}

/// A drawn block. Rectangles use centre (x, y) and size (w, d); trees and pits use w as
/// diameter; the power line uses a and b.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Item {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub floors: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub stilt: Option<bool>,
    #[serde(default)]
    pub a: Option<P>,
    #[serde(default)]
    pub b: Option<P>,
    #[serde(default)]
    pub voltage: Option<String>,
    #[serde(default)]
    pub water: Option<String>,
    /// Staircase flight width.
    #[serde(default)]
    pub width: Option<f64>,
    /// Building: plinth height above the highest ground under it.
    #[serde(default)]
    pub plinth: Option<f64>,
    #[serde(default)]
    pub basement: Option<bool>,
    /// Own height of a block (garage, gate, tree, custom, hut …), metres.
    #[serde(default)]
    pub h: Option<f64>,
    /// Building: floor-to-floor height.
    #[serde(default)]
    pub floor_height: Option<f64>,
    /// Building or custom block: roof pitch in degrees (0 = flat).
    #[serde(default)]
    pub roof_pitch: Option<f64>,
    /// Building: parapet height on a flat roof (None = not drawn).
    #[serde(default)]
    pub parapet: Option<f64>,
    /// Balcony: floor it projects from (0 = ground).
    #[serde(default)]
    pub on_floor: Option<f64>,
    /// Septic tank depth below ground.
    #[serde(default)]
    pub depth: Option<f64>,
    /// Custom block: check it against the rule its name matches (default on).
    #[serde(default)]
    pub check_rules: Option<bool>,
}

impl Item {
    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("{} {}", kind_name(&self.kind), self.id))
    }
    fn rect(&self) -> Vec<P> {
        let (hw, hd) = (self.w / 2.0, self.d / 2.0);
        vec![
            [self.x - hw, self.y - hd],
            [self.x + hw, self.y - hd],
            [self.x + hw, self.y + hd],
            [self.x - hw, self.y + hd],
        ]
    }
    fn floors(&self) -> f64 {
        self.floors.unwrap_or(1.0).max(1.0)
    }
    fn floor_h(&self) -> f64 {
        self.floor_height.filter(|v| *v > 0.0).unwrap_or(FLOOR_HEIGHT)
    }
    /// Physical height of the storeys (no plinth, ridge or parapet).
    fn height(&self) -> f64 {
        self.height.unwrap_or(self.floors() * self.floor_h())
    }
    /// Ridge rise of a pitched roof over the shorter span.
    fn ridge(&self) -> f64 {
        let p = self.roof_pitch.unwrap_or(0.0).clamp(0.0, 75.0);
        self.w.min(self.d) / 2.0 * p.to_radians().tan()
    }
    fn area(&self) -> f64 {
        self.w * self.d
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct EdgeReq {
    pub index: usize,
    /// "front" | "rear" | "side"
    pub class: &'static str,
    pub required: f64,
    pub label: String,
    pub source: Option<Source>,
    /// The required open-space line, inset from the edge.
    pub line: [P; 2],
}

#[derive(Serialize, Debug, Clone)]
pub struct Issue {
    /// "error" (breaks a rule) | "info" (advice, or a value the rules leave open)
    pub level: &'static str,
    pub item: Option<String>,
    pub text: String,
    pub source: Option<Source>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Meter {
    pub used: f64,
    pub limit: Option<f64>,
    pub label: String,
    pub ok: bool,
    pub source: Option<Source>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Check {
    pub ok: bool,
    pub errors: usize,
    pub plot_area: f64,
    pub frontage: f64,
    pub max_height: f64,
    pub envelope: String,
    pub edges: Vec<EdgeReq>,
    /// Area inside all open-space lines (convex plots only; empty otherwise).
    pub buildable: Vec<P>,
    pub far: Option<Meter>,
    pub coverage: Meter,
    pub issues: Vec<Issue>,
    /// Item ids that break a rule (drawn red).
    pub bad: Vec<String>,
    pub assumed: Vec<String>,
    /// Levels (metres above sea level) and the reference level heights are measured from.
    pub road_level: f64,
    pub reference_level: f64,
    pub sloping: bool,
    /// Ground plane z = a·x + b·y + c fitted through the corner levels.
    pub plane: [f64; 3],
    /// Steepest slope as a fraction (rise / run).
    pub slope: f64,
    pub levels: Vec<BuildingLevel>,
    pub custom: Vec<CustomAck>,
}

/// Where a building sits vertically.
#[derive(Serialize, Debug, Clone)]
pub struct BuildingLevel {
    pub id: String,
    /// Highest ground under the footprint.
    pub ground: f64,
    /// Ground-floor level (top of plinth, or of stilt deck).
    pub floor: f64,
    pub roof: f64,
    /// Highest point counted for height: roof plus ridge, plus a parapet taller than Rule 2(9) excludes.
    pub top: f64,
    /// Height for the rules: roof minus reference level, stilt excluded (Rule 2(9)).
    pub rule_height: f64,
}

pub fn kind_name(kind: &str) -> &'static str {
    match kind {
        "building" => "Building",
        "balcony" => "Balcony",
        "garage" => "Garage",
        "septic_tank" => "Septic tank",
        "tree" => "Tree",
        "rwh_pit" => "Rainwater pit",
        "parking_bay" => "Parking bay",
        "staircase" => "Staircase",
        "lift" => "Lift",
        "shaft" => "Ventilation shaft",
        "courtyard" => "Courtyard",
        "water_body" => "Lake / river",
        "electric_line" => "Power line",
        "gate" => "Gate",
        "custom" => "Custom",
        "sunshade" => "Chhajja / sunshade",
        "hut" => "Watchman hut / pump room",
        "servant_quarter" => "Servant quarter",
        "porch" => "Porch",
        "roof_structure" => "Rooftop structure",
        "ramp" => "Ramp",
        "parapet_wall" => "Parapet",
        _ => "Item",
    }
}

impl Track1 {
    pub fn check_layout(&self, l: &Layout) -> Check {
        check(self, l)
    }
}

struct Out {
    issues: Vec<Issue>,
    bad: Vec<String>,
}

impl Out {
    fn err(&mut self, item: Option<&Item>, text: String, src: Option<Source>) {
        if let Some(i) = item {
            if !self.bad.contains(&i.id) {
                self.bad.push(i.id.clone());
            }
        }
        self.issues.push(Issue { level: "error", item: item.map(|i| i.id.clone()), text, source: src });
    }
    fn info(&mut self, item: Option<&Item>, text: String, src: Option<Source>) {
        self.issues.push(Issue { level: "info", item: item.map(|i| i.id.clone()), text, source: src });
    }
}

fn src(f: Option<&Fact>) -> Option<Source> {
    f.map(|f| f.source())
}

fn sb_src(s: &Setback) -> Source {
    Source { label: s.label.clone(), cite: s.cite.clone(), quote: s.quote.clone() }
}

/// Names a user might give a custom block, mapped to what the rules call it. More specific
/// phrases come first ("guard room" before "room"). Matching is whole-word.
const ALIASES: &[(&str, &[&str])] = &[
    ("hut", &["watchman", "guard room", "guard cabin", "security cabin", "chowkidar", "pump room", "pump house", "चौकीदार"]),
    ("servant_quarter", &["servant quarter", "servant room", "maid room", "outhouse", "out house"]),
    ("roof_structure", &["water tank", "overhead tank", "mumty", "mumptee", "machine room", "chimney", "टंकी"]),
    ("garage", &["garage", "car shed", "carport", "car port", "vehicle shed", "गैरेज"]),
    ("parking_bay", &["parking", "car park", "car space", "पार्किंग"]),
    ("porch", &["porch", "portico", "पोर्च"]),
    ("balcony", &["balcony", "gallery", "sit out", "sit-out", "बालकनी"]),
    ("sunshade", &["chhajja", "chajja", "sunshade", "sun shade", "weather shade", "cornice", "छज्जा"]),
    ("staircase", &["stair", "stairs", "staircase", "steps", "सीढ़ी", "जीना"]),
    ("lift", &["lift", "elevator", "लिफ्ट"]),
    ("shaft", &["shaft", "duct", "ventilation"]),
    ("courtyard", &["courtyard", "court yard", "aangan", "angan", "chowk", "आंगन", "आँगन"]),
    ("septic_tank", &["septic", "soak pit", "soakpit", "sewage tank", "सेप्टिक", "सोख्ता"]),
    ("rwh_pit", &["rainwater", "rain water", "recharge pit", "harvesting", "rwh"]),
    ("tree", &["tree", "plant", "पेड़", "पौधा"]),
    ("gate", &["gate", "entrance", "गेट", "फाटक"]),
    ("water_body", &["pond", "lake", "nala", "nallah", "talab", "river", "तालाब", "नदी"]),
    ("ramp", &["ramp", "driveway ramp"]),
    ("parapet_wall", &["parapet", "मुंडेर"]),
    ("building", &["room", "house", "annex", "annexe", "extension", "block", "wing", "shop", "office", "कमरा", "मकान", "दुकान"]),
];

/// The rule kind a custom name stands for, if any.
pub fn resolve_custom(name: &str) -> Option<&'static str> {
    let q = name.to_lowercase();
    ALIASES
        .iter()
        .find(|(_, ws)| ws.iter().any(|w| crate::catalog::contains_word(&q, w)))
        .map(|(k, _)| *k)
}

fn kind_rules(kind: &str) -> &'static str {
    match kind {
        "garage" => "Rules 75, 56(4), 2(20)",
        "parking_bay" => "Rule 84 (norms in Appendix 1)",
        "balcony" => "Rule 58(1)(c)",
        "sunshade" => "Rule 58(1)(a)-(b)",
        "staircase" => "Rule 80",
        "lift" => "Rule 83",
        "shaft" => "Rule 56(6)(b)",
        "courtyard" => "Rule 56(6)(a)",
        "septic_tank" => "Rule 79",
        "rwh_pit" => "Rule 81",
        "tree" => "Rule 67",
        "gate" => "Rules 41(c), 42(6)",
        "water_body" => "Rule 50(b)",
        "hut" => "Rules 58(4)(c), 2(20)",
        "servant_quarter" => "Rules 2(30), 61 Note (3)",
        "porch" => "Rules 2(55), 2(20)",
        "roof_structure" => "Rule 2(9)",
        "ramp" => "Rule 76(4)(g)",
        "parapet_wall" => "Rules 77, 2(9)",
        "building" => "open spaces, FAR and height (Rules 56/57/42, 61)",
        _ => "",
    }
}

/// What a custom block was taken to be.
#[derive(Serialize, Debug, Clone)]
pub struct CustomAck {
    pub id: String,
    pub name: String,
    /// Rule kind it was read as (None: no rule names it).
    pub as_kind: Option<String>,
    pub as_name: Option<String>,
    pub rules: Option<String>,
    pub checked: bool,
}

fn check(t: &Track1, l: &Layout) -> Check {
    // Custom blocks with rule checking on take the kind their name matches.
    let mut acks = vec![];
    let items: Vec<Item> = l
        .items
        .iter()
        .map(|it| {
            if it.kind != "custom" {
                return it.clone();
            }
            let name = it.name.clone().unwrap_or_default();
            let kind = resolve_custom(&name);
            let on = it.check_rules != Some(false);
            acks.push(CustomAck {
                id: it.id.clone(),
                name: name.clone(),
                as_kind: kind.map(str::to_string),
                as_name: kind.map(|k| kind_name(k).to_string()),
                rules: kind.map(|k| kind_rules(k).to_string()),
                checked: on && kind.is_some(),
            });
            match kind {
                Some(k) if on => Item { kind: k.to_string(), ..it.clone() },
                _ => it.clone(),
            }
        })
        .collect();
    let items = &items;
    let fx = &t.facts;
    let mut o = Out { issues: vec![], bad: vec![] };
    let plot = &l.plot;
    let n = plot.len();
    let area = signed_area(plot).abs();
    let mut assumed = vec![];

    let buildings: Vec<&Item> = items.iter().filter(|i| i.kind == "building").collect();
    let of = |k: &'static str| items.iter().filter(move |i| i.kind == k);

    // ---- plot sanity ----
    if n >= 4 && self_intersects(plot) {
        o.err(None, "The plot outline crosses itself. Drag the corners so the edges do not cross.".into(), None);
        if area < 1.0 {
            return empty(o, area);
        }
    }
    if n < 3 || area < 1.0 {
        o.err(None, "Draw the plot outline first: at least three corners.".into(), None);
        return empty(o, area);
    }
    let mut roads: Vec<usize> = l.road_edges.iter().copied().filter(|&i| i < n).collect();
    if roads.is_empty() {
        roads.push(0);
        assumed.push("edge 1 faces the road (click an edge with the Road tool to change)".to_string());
    }
    let frontage: f64 = roads.iter().map(|&i| dist(plot[i], plot[(i + 1) % n])).sum();

    // ---- levels: Rule 2(9) / 2(34) ----
    let road = l.road_level.unwrap_or(0.0);
    let levels: Vec<f64> = if l.plot_levels.len() == n { l.plot_levels.clone() } else { vec![road; n] };
    let plane = fit_plane(plot, &levels);
    let ground = |p: P| plane[0] * p[0] + plane[1] * p[1] + plane[2];
    let (lo, hi) = levels.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    let sloping = hi - lo > FLAT_TOLERANCE;
    let avg_level = levels.iter().sum::<f64>() / n as f64;
    // Flat: ground at the centre of the frontage. Sloping: road centre line or the average
    // plot level, whichever is higher.
    let frontage_centre = {
        let ms: Vec<P> = roads.iter().map(|&i| mid(plot[i], plot[(i + 1) % n])).collect();
        [ms.iter().map(|m| m[0]).sum::<f64>() / ms.len() as f64, ms.iter().map(|m| m[1]).sum::<f64>() / ms.len() as f64]
    };
    let reference = if sloping { road.max(avg_level) } else { ground(frontage_centre) };
    let slope = (plane[0] * plane[0] + plane[1] * plane[1]).sqrt();
    let stilt_h = fx.v("stilt.h").unwrap_or(2.4) + 0.2;
    let blevels: Vec<BuildingLevel> = buildings
        .iter()
        .map(|b| {
            let g = b.rect().iter().map(|&p| ground(p)).fold(f64::MIN, f64::max);
            let floor = g + b.plinth.unwrap_or(DEFAULT_PLINTH);
            let st = if b.stilt == Some(true) { stilt_h } else { 0.0 };
            let roof = floor + st + b.floors() * b.floor_h();
            // Rule 2(9): parapets up to the stated height are left out of building height.
            let excl = fx.v("parapet.excl").unwrap_or(1.5);
            let para = b.parapet.filter(|p| b.ridge() == 0.0 && *p > excl).unwrap_or(0.0);
            let top = roof + b.ridge() + para;
            BuildingLevel {
                id: b.id.clone(),
                ground: round2(g),
                floor: round2(floor),
                roof: round2(roof),
                top: round2(top),
                rule_height: round2(top - st - reference),
            }
        })
        .collect();
    let rh = |b: &Item| blevels.iter().find(|x| x.id == b.id).map(|x| x.rule_height).unwrap_or(b.height());
    let max_h = blevels.iter().map(|x| x.rule_height).fold(0.0, f64::max);
    if buildings.is_empty() {
        o.info(None, "Add a building block to check open spaces, FAR and coverage.".into(), None);
    }

    // ---- the governing rules, from the same scene logic as the 3D answers ----
    let input = SceneInput {
        plot_area: Some(round2(area)),
        frontage: Some(round2(frontage)),
        street_width: Some(l.street_width),
        height: Some(if max_h > 0.0 { max_h } else { FLOOR_HEIGHT }),
        use_type: l.use_type.clone(),
        building_type: l.building_type.clone(),
        ..Default::default()
    };
    let scene = t.scene(&input);
    for a in &scene.assumed {
        if matches!(a.field, "use_type" | "building_type") {
            assumed.push(a.text.clone());
        }
    }
    if !buildings.is_empty() {
        // The plot geometry is measured directly below, so skip the scene's
        // rectangular-plot estimate of whether anything is left to build on.
        for note in scene.notes.iter().filter(|n| !n.starts_with("The open spaces use up")) {
            o.err(None, note.clone(), None);
        }
    }
    for note in scene.info.iter().filter(|n| !n.starts_with("Footprint reduced") && !n.starts_with("Plinth drawn")) {
        o.info(None, note.clone(), None);
    }

    // ---- classify edges ----
    let sign = if signed_area(plot) > 0.0 { 1.0 } else { -1.0 };
    let normal = |i: usize| {
        let (p, q) = (plot[i], plot[(i + 1) % n]);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        [sign * dy / len, -sign * dx / len] // outward
    };
    let rn = {
        let s = roads.iter().fold([0.0, 0.0], |acc, &i| {
            let v = normal(i);
            [acc[0] + v[0], acc[1] + v[1]]
        });
        let len = (s[0] * s[0] + s[1] * s[1]).sqrt().max(1e-9);
        [s[0] / len, s[1] / len]
    };
    let centroid = centroid(plot);
    // Which side of the road axis an edge sits on: left / right of someone facing the road.
    let side_of = |i: usize| {
        let m = mid(plot[i], plot[(i + 1) % n]);
        let v = [m[0] - centroid[0], m[1] - centroid[1]];
        if v[0] * rn[1] - v[1] * rn[0] >= 0.0 { 0 } else { 1 }
    };
    let classes: Vec<&'static str> = (0..n)
        .map(|i| {
            if roads.contains(&i) {
                "front"
            } else {
                let v = normal(i);
                if v[0] * rn[0] + v[1] * rn[1] < -0.5 { "rear" } else { "side" }
            }
        })
        .collect();

    // Side requirements: Rule 56 gives two values (left/right may differ by proviso, one
    // may be absent for semi-detached). Give each side group the assignment that suits the
    // drawn buildings best — the rule lets either side take the reduced value.
    let sides = [scene.side_left.clone(), scene.side_right.clone()];
    let side_edges: [Vec<usize>; 2] = [
        (0..n).filter(|&i| classes[i] == "side" && side_of(i) == 0).collect(),
        (0..n).filter(|&i| classes[i] == "side" && side_of(i) == 1).collect(),
    ];
    let shortfall = |group: &[usize], req: f64| -> f64 {
        buildings
            .iter()
            .map(|b| group.iter().map(|&e| (req - poly_seg_dist(&b.rect(), plot[e], plot[(e + 1) % n])).max(0.0)).sum::<f64>())
            .sum()
    };
    let m = |s: &Option<Setback>| s.as_ref().map(|x| x.m).unwrap_or(0.0);
    let straight = shortfall(&side_edges[0], m(&sides[0])) + shortfall(&side_edges[1], m(&sides[1]));
    let swapped = shortfall(&side_edges[0], m(&sides[1])) + shortfall(&side_edges[1], m(&sides[0]));
    let side_for = |g: usize| -> &Option<Setback> {
        let idx = if swapped < straight { 1 - g } else { g };
        &sides[idx]
    };

    let mut edges = vec![];
    for i in 0..n {
        let (class, sb) = match classes[i] {
            "front" => ("front", &scene.front),
            "rear" => ("rear", &scene.rear),
            _ => ("side", side_for(side_of(i))),
        };
        let req = m(sb);
        let nv = normal(i);
        let (p, q) = (plot[i], plot[(i + 1) % n]);
        let inset = |a: P| [a[0] - nv[0] * req, a[1] - nv[1] * req];
        edges.push(EdgeReq {
            index: i,
            class,
            required: req,
            label: match sb {
                Some(s) => format!("{class} {req} m · {}", s.label.split("· ").nth(1).unwrap_or(&s.label)),
                None => format!("{class} · none required"),
            },
            source: sb.as_ref().map(sb_src),
            line: [inset(p), inset(q)],
        });
    }

    // Buildable area: intersection of the inset half-planes (exact for convex plots).
    let buildable = if is_convex(plot) {
        let mut poly = plot.clone();
        for e in &edges {
            let nv = normal(e.index);
            let p = plot[e.index];
            poly = clip(&poly, |x| (x[0] - p[0]) * nv[0] + (x[1] - p[1]) * nv[1] + e.required);
        }
        poly
    } else {
        o.info(None, "Plot is not convex: open-space lines are shown per edge; the shaded buildable area is omitted.".into(), None);
        vec![]
    };

    // ---- buildings ----
    for b in &buildings {
        let r = b.rect();
        if !rect_inside(&r, plot) {
            o.err(Some(b), format!("{} is not fully inside the plot.", b.label()), None);
            continue;
        }
        for e in &edges {
            if e.required <= 0.0 {
                continue;
            }
            let dd = poly_seg_dist(&r, plot[e.index], plot[(e.index + 1) % n]);
            if dd + 0.005 < e.required {
                o.err(
                    Some(b),
                    format!(
                        "{} is {} m from the {} edge; {} needs {} m.",
                        b.label(),
                        round2(dd),
                        e.class,
                        e.source.as_ref().map(|s| s.label.split("· ").nth(1).unwrap_or(&s.label).to_string()).unwrap_or_default(),
                        e.required
                    ),
                    e.source.clone(),
                );
            }
        }
        if b.stilt == Some(true) {
            if let Some(min) = fx.v("stilt.plot") {
                if area < min {
                    o.err(Some(b), format!("{}: stilt parking is for plots of {min} sq m or more; this plot is {} sq m.", b.label(), round1(area)), src(fx.get("stilt.plot")));
                }
            }
        }
    }

    // ---- FAR and coverage ----
    let floor_area: f64 = buildings.iter().map(|b| b.area() * b.floors()).sum();
    let far = scene.far.as_ref().map(|f| {
        let limit = round2(f.far * area);
        Meter {
            used: round2(floor_area),
            limit: Some(limit),
            label: format!("Floor area vs FAR {}", f.far),
            ok: floor_area <= limit + 0.01,
            source: Some(f.source.clone()),
        }
    });
    if let Some(f) = &far {
        if !f.ok {
            o.err(None, format!("Floor area {} sq m is over the FAR limit of {} sq m.", f.used, f.limit.unwrap()), f.source.clone());
        }
    }
    let covered: f64 = buildings.iter().map(|b| b.area()).sum::<f64>()
        + of("garage").map(|g| g.area()).filter(|a| *a > fx.v("garage.free_sqm").unwrap_or(25.0)).sum::<f64>();
    let cap = scene.coverage_cap_pct;
    let pct = if area > 0.0 { covered / area * 100.0 } else { 0.0 };
    let coverage = Meter {
        used: round1(pct),
        limit: cap,
        label: "Ground coverage %".into(),
        ok: cap.map(|c| pct <= c + 0.01).unwrap_or(true),
        source: None,
    };
    if !coverage.ok {
        o.err(None, format!("Ground coverage {}% is over the {}% limit (Rule 42 Table 5).", round1(pct), cap.unwrap()), None);
    }

    // ---- components ----
    let inside_building = |i: &Item| buildings.iter().find(|b| rect_inside(&i.rect(), &b.rect()));
    let res = matches!(scene.use_type.as_str(), "residential" | "farm_house");

    // Is a block inside a required open space (closer to an edge than that edge allows)?
    let in_mos = |r: &[P]| edges.iter().any(|e| e.required > 0.0 && poly_seg_dist(r, plot[e.index], plot[(e.index + 1) % n]) + 0.005 < e.required);
    let touches_building = |r: &[P]| buildings.iter().any(|b| poly_poly_dist(r, &b.rect()) < 0.05);

    for it in items {
        match it.kind.as_str() {
            "balcony" => {
                // Rule 58(1)(c): "at first or higher floors".
                let fl = it.on_floor.unwrap_or(1.0);
                if fl < 1.0 {
                    o.err(Some(it), format!("{} is on the ground floor; Rule 58(1)(c) allows projected balconies at first or higher floors.", it.label()), src(fx.get("proj.balcony")));
                }
                if let Some(host) = buildings.iter().find(|b| poly_poly_dist(&it.rect(), &b.rect()) < 0.05) {
                    if fl > host.floors() - 1.0 {
                        o.err(Some(it), format!("{} is on floor {fl}, but {} has only {} floors.", it.label(), host.label(), host.floors()), None);
                    }
                }
                let (max, clear) = (fx.v("proj.balcony"), fx.v("proj.balcony_clear"));
                let s = src(fx.get("proj.balcony"));
                if let Some(max) = max {
                    let depth = it.w.min(it.d);
                    if depth > max + 0.005 {
                        o.err(Some(it), format!("{} projects {} m; Rule 58(1)(c) allows up to {max} m.", it.label(), round2(depth)), s.clone());
                    }
                }
                if let Some(clear) = clear {
                    let dd = (0..n).map(|e| poly_seg_dist(&it.rect(), plot[e], plot[(e + 1) % n])).fold(f64::MAX, f64::min);
                    if dd + 0.005 < clear {
                        o.err(Some(it), format!("{} is {} m from the plot boundary; Rule 58(1)(c) needs {clear} m.", it.label(), round2(dd)), s.clone());
                    }
                }
                if !buildings.iter().any(|b| poly_poly_dist(&it.rect(), &b.rect()) < 0.05) {
                    o.err(Some(it), format!("{} is not attached to a building.", it.label()), None);
                }
            }
            "garage" => {
                if let (Some(gw), Some(gl)) = (fx.v("garage.w"), fx.v("garage.l")) {
                    let (a, b) = (it.w.min(it.d), it.w.max(it.d));
                    if a + 0.005 < gw || b + 0.005 < gl {
                        o.err(Some(it), format!("{} is {} × {} m; Rule 75(a) needs at least {gw} × {gl} m.", it.label(), round2(a), round2(b)), src(fx.get("garage.w")));
                    }
                }
                if !rect_inside(&it.rect(), plot) {
                    o.err(Some(it), format!("{} is not fully inside the plot.", it.label()), None);
                }
                let in_front = edges
                    .iter()
                    .filter(|e| e.class == "front" && e.required > 0.0)
                    .any(|e| poly_seg_dist(&it.rect(), plot[e.index], plot[(e.index + 1) % n]) + 0.005 < e.required);
                if in_front {
                    o.err(Some(it), format!("{} sits in the front open space; Rule 56(4) allows a garage only at the rear end of a side open space.", it.label()), None);
                }
                if let Some(min_h) = fx.v("garage.h") {
                    let h = it.h.unwrap_or(2.6);
                    if h + 0.005 < min_h {
                        o.err(Some(it), format!("{} is {h} m high; Rule 75(1) needs at least {min_h} m.", it.label()), src(fx.get("garage.h")));
                    }
                }
                if let Some(free) = fx.v("garage.free_sqm") {
                    if it.area() > free {
                        o.info(Some(it), format!("{} is over {free} sq m, so it counts as ground coverage (Rule 2(20)).", it.label()), src(fx.get("garage.free_sqm")));
                    }
                }
            }
            "septic_tank" => {
                if let Some(min_d) = fx.v("septic.depth") {
                    let d = it.depth.unwrap_or(1.0);
                    if d + 0.005 < min_d {
                        o.err(Some(it), format!("{} is {d} m deep; Rule 79(2)(a) needs at least {min_d} m below the surface.", it.label()), src(fx.get("septic.w_cm")));
                    }
                }
                let s = src(fx.get("septic.gap"));
                if let Some(gap) = fx.v("septic.gap") {
                    for b in &buildings {
                        let dd = poly_poly_dist(&it.rect(), &b.rect());
                        if dd + 0.005 < gap {
                            o.err(Some(it), format!("{} is {} m from {}; Rule 79(1) needs {gap} m.", it.label(), round2(dd), b.label()), s.clone());
                        }
                    }
                }
                if let (Some(wc), Some(lm)) = (fx.v("septic.w_cm"), fx.v("septic.len_min")) {
                    let (a, b) = (it.w.min(it.d), it.w.max(it.d));
                    if a + 0.005 < wc / 100.0 {
                        o.err(Some(it), format!("{} is {} m wide; Rule 79(2)(a) needs at least {} m.", it.label(), round2(a), wc / 100.0), src(fx.get("septic.w_cm")));
                    } else if b < lm * a - 0.005 || b > 4.0 * a + 0.005 {
                        o.err(Some(it), format!("{} is {} m long; Rule 79(2)(a) wants 2 to 4 times its width ({}–{} m).", it.label(), round2(b), round2(lm * a), round2(4.0 * a)), src(fx.get("septic.len_min")));
                    }
                }
                if !rect_inside(&it.rect(), plot) {
                    o.err(Some(it), format!("{} is not inside the plot.", it.label()), None);
                }
            }
            "staircase" => {
                let Some(b) = inside_building(it) else {
                    o.err(Some(it), format!("{} must be inside a building.", it.label()), None);
                    continue;
                };
                let key = match scene.use_type.as_str() {
                    _ if res => "stair.w.res",
                    "assembly" => "stair.w.asm",
                    "educational" if rh(b) > 24.0 => "stair.w.edu_high",
                    "educational" => "stair.w.edu_low",
                    "institutional" => "stair.w.inst",
                    _ => "stair.w.other",
                };
                if let Some(min) = fx.v(key) {
                    let w = it.width.unwrap_or(it.w.min(it.d) / 2.0);
                    if w + 0.005 < min {
                        o.err(Some(it), format!("{} flight is {} m wide; Rule 80(2) needs {min} m here.", it.label(), round2(w)), src(fx.get(key)));
                    }
                }
            }
            "lift" => {
                if inside_building(it).is_none() {
                    o.err(Some(it), format!("{} must be inside a building.", it.label()), None);
                }
            }
            "shaft" => {
                let Some(b) = inside_building(it) else {
                    o.err(Some(it), format!("{} must be inside a building.", it.label()), None);
                    continue;
                };
                if let Some((need_a, need_s, f)) = fx.shaft(rh(b)) {
                    let (a, side) = (it.area(), it.w.min(it.d));
                    if a + 0.005 < need_a || side + 0.005 < need_s {
                        o.err(Some(it), format!("{} is {} sq m with a {} m side; for a {} m building Rule 56(6)(b) needs {need_a} sq m and {need_s} m.", it.label(), round2(a), round2(side), rh(b)), Some(f.source()));
                    }
                }
            }
            "courtyard" => {
                let Some(b) = inside_building(it) else {
                    o.err(Some(it), format!("{} must be inside a building.", it.label()), None);
                    continue;
                };
                if let (Some(ms), Some(fr)) = (fx.v("court.side"), fx.v("court.frac")) {
                    let need_area = (b.height() * fr).powi(2);
                    let side = it.w.min(it.d);
                    if side + 0.005 < ms || it.area() + 0.005 < need_area {
                        o.err(Some(it), format!("{} is {} × {} m; Rule 56(6)(a) needs sides of {ms} m and area (height ÷ 5)² = {} sq m.", it.label(), round2(it.w), round2(it.d), round2(need_area)), src(fx.get("court.side")));
                    }
                }
            }
            "water_body" => {
                let kind = it.water.as_deref().unwrap_or("lake");
                let id = if kind == "river" { "water.river" } else { "water.buffer" };
                if let Some(buf) = fx.v(id) {
                    for b in &buildings {
                        let dd = poly_poly_dist(&it.rect(), &b.rect());
                        if dd + 0.005 < buf {
                            o.err(Some(b), format!("{} is {} m from the {kind}; Rule 50(b) needs {buf} m.", b.label(), round2(dd)), src(fx.get(id)));
                        }
                    }
                }
            }
            "electric_line" => {
                let (Some(a), Some(bp)) = (it.a, it.b) else { continue };
                let class = it.voltage.as_deref().unwrap_or("lv");
                let (key, label) = match class {
                    "hv" => ("line.hv.h", "high voltage up to 33 kV"),
                    "ehv" => ("line.ehv.half", "extra high voltage above 33 kV"),
                    _ => ("line.lv.h", "low / medium voltage"),
                };
                if let Some(h) = fx.v(key) {
                    for b in items.iter().filter(|x| matches!(x.kind.as_str(), "building" | "balcony")) {
                        let dd = poly_seg_dist(&b.rect(), a, bp);
                        if dd + 0.005 < h {
                            o.err(Some(b), format!("{} is {} m from the {label} line; Rule 52 needs {h} m horizontally.", b.label(), round2(dd)), src(fx.get(key)));
                        }
                    }
                }
            }
            "gate" => {
                let on_road = roads.iter().any(|&e| poly_seg_dist(&it.rect(), plot[e], plot[(e + 1) % n]) < 0.4);
                if !on_road {
                    o.err(Some(it), format!("{} should be on the road edge.", it.label()), None);
                }
                let tall = scene.height_m > fx.v("r56.max_h").unwrap_or(12.5);
                if (!res || tall) && buildings.len() > 0 {
                    if let Some(g) = fx.v("gate.width") {
                        let w = it.w.max(it.d);
                        if w + 0.005 < g {
                            o.err(Some(it), format!("{} is {} m wide; Rule 41(c) / 42(6) needs {g} m for fire engines.", it.label(), round2(w)), src(fx.get("gate.width")));
                        }
                    }
                }
            }
            "sunshade" => {
                if let Some(max) = fx.v("proj.sunshade") {
                    let dpt = it.w.min(it.d);
                    if dpt > max + 0.005 {
                        o.err(Some(it), format!("{} projects {} m; Rule 58(1)(b) allows up to {max} m.", it.label(), round2(dpt)), src(fx.get("proj.sunshade")));
                    }
                }
                if !touches_building(&it.rect()) {
                    o.err(Some(it), format!("{} is not attached to a building.", it.label()), None);
                }
            }
            "hut" => {
                if let Some(max) = fx.v("hut.max") {
                    if it.area() > max + 0.005 {
                        if in_mos(&it.rect()) {
                            o.err(Some(it), format!("{} is {} sq m in an open space; Rule 58(4)(c) allows up to {max} sq m there.", it.label(), round2(it.area())), src(fx.get("hut.max")));
                        } else {
                            o.info(Some(it), format!("{} is over {max} sq m, so it counts as covered area (Rule 2(20)).", it.label()), src(fx.get("hut.max")));
                        }
                    }
                }
            }
            "servant_quarter" => {
                if let Some(max) = fx.v("sq.max") {
                    if it.area() > max + 0.005 {
                        o.info(Some(it), format!("{} is {} sq m; over {max} sq m it counts in FAR (Rule 2(30)).", it.label(), round2(it.area())), src(fx.get("sq.max")));
                    }
                }
                if in_mos(&it.rect()) {
                    o.err(Some(it), format!("{} sits in a required open space.", it.label()), None);
                }
            }
            "porch" => {
                o.info(Some(it), format!("{}: a porch is not counted in ground coverage or FAR (Rule 2(20), 2(30)).", it.label()), src(fx.get("porch.def")));
                if !touches_building(&it.rect()) {
                    o.err(Some(it), format!("{} is not attached to a building.", it.label()), None);
                }
            }
            "roof_structure" => match inside_building(it) {
                None => o.err(Some(it), format!("{} should sit on a building's roof (inside its outline).", it.label()), None),
                Some(b) => {
                    let total: f64 = items.iter().filter(|x| x.kind == "roof_structure" && rect_inside(&x.rect(), &b.rect())).map(|x| x.area()).sum();
                    let third = b.area() / 3.0;
                    if total > third + 0.005 {
                        o.info(Some(it), format!("Rooftop structures on {} cover {} sq m, over a third of the roof ({} sq m): they count in its height (Rule 2(9)).", b.label(), round2(total), round2(third)), src(fx.get("height.excl")));
                    } else {
                        o.info(Some(it), format!("{} is left out of building height: rooftop structures cover no more than a third of the roof (Rule 2(9)).", it.label()), src(fx.get("height.excl")));
                    }
                }
            },
            "ramp" => {
                if in_mos(&it.rect()) {
                    o.err(Some(it), format!("{} sits in a required open space; Rule 76(4)(g) allows open ramps only where they do not obstruct it.", it.label()), src(fx.get("ramp.mos")));
                }
            }
            "parapet_wall" => {
                if let Some(min) = fx.v("parapet.h") {
                    let h = it.h.unwrap_or(1.0);
                    if h + 0.005 < min {
                        o.err(Some(it), format!("{} is {h} m; Rule 77 needs at least {min} m.", it.label()), src(fx.get("parapet.h")));
                    }
                }
            }
            _ => {}
        }
    }

    // Custom blocks: say what each was taken to be.
    for a in &acks {
        let it = l.items.iter().find(|x| x.id == a.id);
        let label = if a.name.is_empty() { "This custom block".to_string() } else { format!("‘{}’", a.name) };
        let text = match (&a.as_name, a.checked) {
            (Some(k), true) => format!("{label} is read as {k} and checked against {}.", a.rules.clone().unwrap_or_default()),
            (Some(k), false) => format!("{label} looks like {k} ({}), but rule checking is off for it.", a.rules.clone().unwrap_or_default()),
            (None, _) => {
                let hit = t.corpus.search(&a.name, 1).into_iter().find(|(_, sc)| *sc > 2.0).map(|(i, _)| &t.corpus.rules[i]);
                match hit {
                    Some(r) => format!("No rule names {label}; it is drawn but not checked. Closest rule text: Rule {} — {}.", r.rule, r.title),
                    None => format!("No rule names {label}; it is drawn but not checked."),
                }
            }
        };
        o.issues.push(Issue { level: "info", item: it.map(|x| x.id.clone()), text, source: None });
    }

    // Building heights: storeys (Rule 69), roof parapet (Rules 77, 2(9)).
    for b in &buildings {
        if let Some(min) = fx.v("room.h") {
            let clear = round2(b.floor_h() - SLAB);
            if clear + 0.005 < min {
                o.err(Some(b), format!("{} has {} m floor to floor, about {clear} m clear under a {SLAB} m slab; Rule 69 needs {min} m for habitable rooms.", b.label(), b.floor_h()), src(fx.get("room.h")));
            }
        }
        if b.ridge() == 0.0 {
            match (b.parapet, fx.v("parapet.h"), fx.v("parapet.excl")) {
                (Some(p), Some(min), Some(excl)) if p > 0.0 => {
                    if p + 0.005 < min {
                        o.err(Some(b), format!("{} parapet is {p} m; Rule 77 needs at least {min} m.", b.label()), src(fx.get("parapet.h")));
                    } else if p > excl + 0.005 {
                        o.info(Some(b), format!("{} parapet {p} m is over {excl} m, so it counts in building height (Rule 2(9)).", b.label()), src(fx.get("parapet.excl")));
                    }
                }
                (_, Some(min), _) => o.info(Some(b), format!("{} has no parapet; Rule 77 asks for {min} m on the edges of a usable roof.", b.label()), src(fx.get("parapet.h"))),
                _ => {}
            }
        }
    }

    // Required elements that are missing.
    if let Some(above) = fx.v("lift.above") {
        for b in buildings.iter().filter(|b| rh(b) > above) {
            let has = of("lift").any(|lf| rect_inside(&lf.rect(), &b.rect()));
            if !has {
                o.err(Some(b), format!("{} is {} m tall from the reference level; Rule 83 requires a lift above {above} m.", b.label(), rh(b)), src(fx.get("lift.above")));
            }
        }
    }
    if let Some(p) = fx.v("rwh.plot") {
        if area > p && of("rwh_pit").next().is_none() && !buildings.is_empty() {
            o.err(None, format!("Plot is {} sq m: Rule 81 requires rainwater harvesting above {p} sq m. Add a rainwater pit.", round1(area)), src(fx.get("rwh.plot")));
        }
    }
    let (need_trees, tree_src) = if scene.use_type == "farm_house" {
        (fx.v("farm.trees_ha").map(|h| (h * area / 10_000.0).ceil()), src(fx.get("farm.trees_ha")))
    } else {
        (fx.v("tree.per").filter(|p| area >= *p).map(|p| (area / p).floor()), src(fx.get("tree.per")))
    };
    if let Some(need) = need_trees {
        let have = of("tree").count() as f64;
        if have < need && !buildings.is_empty() {
            o.err(None, format!("{have} of {need} trees: Rule 67 asks one per 100 sq m of plot."), tree_src);
        }
    }
    // Rule 78: wall heights are measured above the centre line of the front street, so a
    // plot above the road gets a lower wall, one below it a taller one.
    if let Some(w) = &l.wall {
        if let (Some(fh), Some(oh)) = (fx.v("wall.front"), fx.v("wall.other")) {
            for e in &edges {
                let front = e.class == "front";
                let (h, cap) = if front { (w.front_h, fh) } else { (w.other_h, oh) };
                let g = levels[e.index].max(levels[(e.index + 1) % n]);
                let above = round2(g + h - road);
                if above > cap + 0.005 {
                    let id = if front { "wall.front" } else { "wall.other" };
                    let room = round2((road + cap - g).max(0.0));
                    o.err(
                        None,
                        format!(
                            "{} wall on edge {} rises {above} m above the street centre line; Rule 78 allows {cap} m, so at most {room} m on this ground.",
                            if front { "Front" } else { "Side / rear" },
                            e.index + 1
                        ),
                        src(fx.get(id)),
                    );
                }
            }
        }
    }

    // ---- levels ----
    if !buildings.is_empty() {
        let how = if sloping {
            format!("the higher of the road centre line ({}) and the average plot level ({})", round2(road), round2(avg_level))
        } else {
            "the ground at the centre of the frontage".to_string()
        };
        o.info(None, format!("Heights are measured from the reference level {} m, {how} (Rule 2(34)).", round2(reference)), src(fx.get("ref.level")));
    }
    if let (Some(ratio), Some(interval)) = (fx.v("slope.ratio"), fx.v("slope.contour")) {
        if slope > 1.0 / ratio {
            o.info(None, format!("The site slopes about 1:{}, steeper than 1:{ratio}: the site plan must show contours every {interval} m (Rule 16(4)(j)).", (1.0 / slope).round()), src(fx.get("slope.ratio")));
        }
    }
    let min_p = fx.v("plinth.min_cm").map(|c| c / 100.0);
    for (b, lv) in buildings.iter().zip(&blevels) {
        let p = b.plinth.unwrap_or(DEFAULT_PLINTH);
        if b.basement == Some(true) {
            if let Some(cap) = fx.v("base.plinth") {
                if p > cap + 0.005 {
                    o.err(Some(b), format!("{} has a basement, so its plinth may be at most {cap} m (Rule 76(2)(b)); it is {p} m.", b.label()), src(fx.get("base.plinth")));
                }
            }
        } else if let Some(min) = min_p {
            if p <= min {
                o.err(Some(b), format!("{} plinth is {p} m; Rule 68(1) needs more than {min} m above the surrounding ground.", b.label()), src(fx.get("plinth.min_cm")));
            }
        }
        if lv.floor < road - 0.005 {
            o.info(
                Some(b),
                format!("{} ground floor ({} m) is {} m below the road centre line ({} m). Rule 68(1) asks that the plinth be placed so the site drains; plan for drainage.", b.label(), lv.floor, round2(road - lv.floor), round2(road)),
                src(fx.get("plinth.drainage")),
            );
        }
        if lv.floor < reference - 0.005 {
            o.info(
                Some(b),
                format!("{} ground floor is {} m below the reference level: if habitable, that space counts in FAR (Rule 76(6)).", b.label(), round2(reference - lv.floor)),
                src(fx.get("below.ref.far")),
            );
        }
    }

    for b in buildings.iter().filter(|b| b.floors() >= 2.0) {
        if !of("staircase").any(|s| rect_inside(&s.rect(), &b.rect())) {
            o.info(Some(b), format!("{} has {} floors but no staircase drawn.", b.label(), b.floors()), None);
        }
    }

    let errors = o.issues.iter().filter(|i| i.level == "error").count();
    Check {
        ok: errors == 0,
        errors,
        plot_area: round2(area),
        frontage: round2(frontage),
        max_height: max_h,
        envelope: scene.envelope.clone(),
        edges,
        buildable,
        far,
        coverage,
        issues: o.issues,
        bad: o.bad,
        assumed,
        road_level: round2(road),
        reference_level: round2(reference),
        sloping,
        plane,
        slope: round2(slope * 1000.0) / 1000.0,
        levels: blevels,
        custom: acks,
    }
}

fn empty(o: Out, area: f64) -> Check {
    Check {
        ok: false,
        errors: o.issues.len(),
        plot_area: area,
        frontage: 0.0,
        max_height: 0.0,
        envelope: String::new(),
        edges: vec![],
        buildable: vec![],
        far: None,
        coverage: Meter { used: 0.0, limit: None, label: "Ground coverage %".into(), ok: true, source: None },
        issues: o.issues,
        bad: o.bad,
        assumed: vec![],
        road_level: 0.0,
        reference_level: 0.0,
        sloping: false,
        plane: [0.0, 0.0, 0.0],
        slope: 0.0,
        levels: vec![],
        custom: vec![],
    }
}

/* ---------- geometry ---------- */

/// Least-squares plane z = a·x + b·y + c through the corner levels (exact for 3 corners).
fn fit_plane(p: &[P], z: &[f64]) -> [f64; 3] {
    let n = p.len() as f64;
    let (mut sx, mut sy, mut sz, mut sxx, mut syy, mut sxy, mut sxz, mut syz) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (q, &h) in p.iter().zip(z) {
        sx += q[0];
        sy += q[1];
        sz += h;
        sxx += q[0] * q[0];
        syy += q[1] * q[1];
        sxy += q[0] * q[1];
        sxz += q[0] * h;
        syz += q[1] * h;
    }
    // Solve [[sxx sxy sx][sxy syy sy][sx sy n]] [a b c] = [sxz syz sz] by Cramer's rule.
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let r = [sxz, syz, sz];
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det3(m);
    if d.abs() < 1e-9 {
        return [0.0, 0.0, sz / n];
    }
    let col = |i: usize| {
        let mut k = m;
        for row in 0..3 {
            k[row][i] = r[row];
        }
        det3(k) / d
    };
    [col(0), col(1), col(2)]
}

fn signed_area(p: &[P]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
}

fn centroid(p: &[P]) -> P {
    let n = p.len() as f64;
    [p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n]
}

fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn mid(a: P, b: P) -> P {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

fn point_seg(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

fn cross(o: P, a: P, b: P) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    if segs_cross(a, b, c, d) {
        return 0.0;
    }
    point_seg(a, c, d).min(point_seg(b, c, d)).min(point_seg(c, a, b)).min(point_seg(d, a, b))
}

fn inside(p: P, poly: &[P]) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
    }
    c
}

/// Distance from a polygon to a segment (0 if they touch or the segment is inside).
fn poly_seg_dist(poly: &[P], a: P, b: P) -> f64 {
    if inside(a, poly) || inside(b, poly) {
        return 0.0;
    }
    let n = poly.len();
    (0..n).map(|i| seg_seg(poly[i], poly[(i + 1) % n], a, b)).fold(f64::MAX, f64::min)
}

fn poly_poly_dist(p: &[P], q: &[P]) -> f64 {
    if p.iter().any(|x| inside(*x, q)) || q.iter().any(|x| inside(*x, p)) {
        return 0.0;
    }
    let n = q.len();
    (0..n).map(|i| poly_seg_dist(p, q[i], q[(i + 1) % n])).fold(f64::MAX, f64::min)
}

fn rect_inside(r: &[P], poly: &[P]) -> bool {
    let n = poly.len();
    let eps_in = |p: P| inside(p, poly) || (0..n).any(|i| point_seg(p, poly[i], poly[(i + 1) % n]) < 1e-6);
    r.iter().all(|p| eps_in(*p))
        && (0..r.len()).all(|i| (0..n).all(|j| !segs_cross(r[i], r[(i + 1) % r.len()], poly[j], poly[(j + 1) % n])))
}

fn self_intersects(p: &[P]) -> bool {
    let n = p.len();
    for i in 0..n {
        for j in i + 1..n {
            if (j + 1) % n == i || j == i + 1 {
                continue;
            }
            if segs_cross(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

fn is_convex(p: &[P]) -> bool {
    let n = p.len();
    let mut sign = 0.0;
    for i in 0..n {
        let c = cross(p[i], p[(i + 1) % n], p[(i + 2) % n]);
        if c.abs() < 1e-9 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// Sutherland–Hodgman: keep the part of `poly` where f(x) <= 0.
fn clip(poly: &[P], f: impl Fn(P) -> f64) -> Vec<P> {
    let n = poly.len();
    let mut out = vec![];
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (fa, fb) = (f(a), f(b));
        if fa <= 0.0 {
            out.push(a);
        }
        if (fa <= 0.0) != (fb <= 0.0) {
            let t = fa / (fa - fb);
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
        }
    }
    out
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
