//! Composes the 3D scene for a question: plot, street, building, open spaces and any
//! catalog components the question (or the user's toggles) asks for.
//!
//! Every rule distance comes from `Facts` (read from the rule text) or the Rule 56
//! tables in `track1.rs`. Where a rule gives no size (a lift shaft, a car bay's
//! position) the value is listed in the component's `illustrative` notes, so the
//! browser can say "drawn, not ruled". The browser only lays out geometry.

use crate::catalog::{self, CATALOG};
use crate::facts::{Fact, Facts, HeightRow};
use crate::models::Source;
use crate::track1::{section, Numbers, Track1};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Words that make a question about the plot itself, even without numbers.
pub const SPATIAL_WORDS: &[&str] = &["open space", "setback", "plot", "खुली जगह", "प्लॉट", "भूखंड"];

/// Inputs used when the question does not say. Each one is reported back as assumed.
const DEFAULT_PLOT_AREA: f64 = 200.0;
const DEFAULT_STREET: f64 = 9.0;
const DEFAULT_HEIGHT: f64 = 9.0;
/// Only used to turn a floor count into a height (and height into floors); reported as assumed.
const FLOOR_HEIGHT: f64 = 3.0;
/// Plot depth : frontage when only the area is known (typical Bhopal plot, e.g. 30x45 ft).
const DEPTH_RATIO: f64 = 1.5;

/// What the user told us. Anything missing gets a stated default.
#[derive(Deserialize, Debug, Default, Clone)]
pub struct SceneInput {
    pub plot_area: Option<f64>,
    pub frontage: Option<f64>,
    pub street_width: Option<f64>,
    pub height: Option<f64>,
    /// Used for height only when `height` is absent.
    pub floors: Option<f64>,
    /// "detached" | "semi-detached" | "row" (residential only)
    pub building_type: Option<String>,
    /// "residential" | "business" | "educational" | "institutional" | "assembly" | "industrial" | "farm_house"
    pub use_type: Option<String>,
    /// Comma-separated catalog ids to draw.
    pub components: Option<String>,
    /// Overhead line class for Rule 52: "lv" | "hv" | "ehv".
    pub voltage: Option<String>,
    /// "lake" | "river" for Rule 50(b).
    pub water: Option<String>,
    /// Plot ground level minus road centre-line level, in metres (negative: plot below road).
    pub plot_rise: Option<f64>,
}

impl SceneInput {
    pub fn from_question(question: &str, nums: &Numbers) -> SceneInput {
        let q = question.to_lowercase();
        let building_type = if crate::track1::mentions(&q, &["semi-detached", "semi detached"]) {
            Some("semi-detached")
        } else if crate::track1::mentions(&q, &["row house", "row-type", "row type", "terrace house"]) {
            Some("row")
        } else if crate::track1::mentions(&q, &["detached", "independent house", "bungalow"]) {
            Some("detached")
        } else {
            None
        };
        let mut components = catalog::match_components(question);
        let voltage = catalog::match_voltage(question);
        let water = catalog::match_water(question);
        if voltage.is_some() && !components.contains(&"electric_line") {
            components.push("electric_line");
        }
        if water.is_some() && !components.contains(&"water_body") {
            components.push("water_body");
        }
        SceneInput {
            plot_area: nums.plot_area,
            frontage: nums.frontage,
            street_width: nums.street_width,
            height: nums.height,
            floors: nums.floors,
            building_type: building_type.map(str::to_string),
            use_type: catalog::match_use(question).map(str::to_string),
            components: Some(components.join(",")),
            voltage: voltage.map(str::to_string),
            water: water.map(str::to_string),
            plot_rise: parse_rise(question),
        }
    }

    /// Use and building type the user picked after asking; they win over the question.
    pub fn apply_details(&mut self, d: &crate::models::Details) {
        if let Some(u) = d.use_type.as_deref().filter(|u| USES.contains(u)) {
            self.use_type = Some(u.to_string());
        }
        if let Some(k) = d.building_type.as_deref().filter(|k| matches!(*k, "detached" | "semi-detached" | "row")) {
            self.building_type = Some(k.to_string());
        }
    }

    /// Add catalog ids (e.g. picked by the model) to the ones already matched.
    pub fn add_components(&mut self, ids: &[String]) {
        let mut cur: Vec<String> = self.ids();
        for id in ids {
            if catalog::entry(id).is_some() && !cur.contains(id) {
                cur.push(id.clone());
            }
        }
        self.components = Some(cur.join(","));
    }

    fn ids(&self) -> Vec<String> {
        self.components
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// An input the question did not give. `field` matches the `SceneInput` name.
#[derive(Serialize, Debug, Clone)]
pub struct Assumed {
    pub field: &'static str,
    pub text: String,
}

/// One open space on the plot, with the rule text it came from.
#[derive(Serialize, Debug, Clone)]
pub struct Setback {
    pub m: f64,
    pub label: String,
    pub cite: String,
    pub quote: String,
}

impl Setback {
    fn from_fact(m: f64, side: &str, f: &Fact) -> Setback {
        Setback {
            m,
            label: format!("{side} · Rule {}{}", f.rule, f.clause),
            cite: format!("PDF page {}", f.page),
            quote: f.quote.clone(),
        }
    }
    fn from_row(m: f64, side: &str, r: &HeightRow) -> Setback {
        Setback {
            m,
            label: format!("{side} · Rule 42 Table {} row {}", r.table, r.row),
            cite: format!("PDF page {}", r.page),
            quote: format!("Table {} row {}: {}", r.table, r.row, r.quote),
        }
    }
}

/// FAR check: proposed floor area against what Rule 61 / 42 / 17 allows.
#[derive(Serialize, Debug, Clone)]
pub struct FarCheck {
    pub far: f64,
    pub allowed_sqm: f64,
    pub proposed_sqm: f64,
    pub ok: bool,
    pub source: Source,
    pub note: Option<String>,
}

/// One drawn component. `params` are metres unless named otherwise.
#[derive(Serialize, Debug, Clone)]
pub struct Component {
    pub id: &'static str,
    pub name: &'static str,
    pub params: Value,
    pub sources: Vec<Source>,
    /// False when the rules do not allow it here (it is still drawn, flagged red).
    pub ok: bool,
    pub notes: Vec<String>,
    /// Sizes or positions the rules do not fix; drawn so the scene reads, not ruled.
    pub illustrative: Vec<String>,
}

/// A component the question mentioned that the 3D view does not draw.
#[derive(Serialize, Debug, Clone)]
pub struct Mention {
    pub id: &'static str,
    pub name: &'static str,
    pub rules: &'static str,
    pub why: &'static str,
}

/// A component the rules require here that is not switched on.
#[derive(Serialize, Debug, Clone)]
pub struct Suggest {
    pub id: &'static str,
    pub name: &'static str,
    pub why: String,
}

/// Everything the 3D view draws.
#[derive(Serialize, Debug, Clone)]
pub struct Scene {
    pub plot_area_sqm: f64,
    pub frontage_m: f64,
    pub depth_m: f64,
    pub street_width_m: f64,
    pub height_m: f64,
    pub floors: u32,
    pub floor_h_m: f64,
    pub building_type: String,
    pub use_type: String,
    /// Plot ground minus road centre line (m). The 3D view drops or raises the road by this.
    pub plot_rise_m: f64,
    /// Which rule set the open spaces come from, in words.
    pub envelope: String,
    /// Inputs that were not in the question and were defaulted.
    pub assumed: Vec<Assumed>,
    pub front: Option<Setback>,
    pub rear: Option<Setback>,
    pub side_left: Option<Setback>,
    pub side_right: Option<Setback>,
    /// Area inside the open-space lines, before coverage / FAR limits.
    pub open_space_limit_sqm: f64,
    /// Footprint drawn: inside the open spaces, then capped by ground coverage and FAR.
    pub footprint_w_m: f64,
    pub footprint_d_m: f64,
    pub footprint_sqm: f64,
    pub plinth_m: f64,
    pub roof: &'static str,
    pub far: Option<FarCheck>,
    /// Ground-coverage cap in % (Rule 42 Table 5 high rises), if any.
    pub coverage_cap_pct: Option<f64>,
    /// False when the open spaces could not be computed from the rules.
    pub covered: bool,
    /// False when a hard limit fails (road too narrow for the height, FAR exceeded, …).
    pub permitted: bool,
    pub components: Vec<Component>,
    pub not_drawn: Vec<Mention>,
    pub suggested: Vec<Suggest>,
    /// Rule breaches.
    pub notes: Vec<String>,
    /// Advisories: footprint adjusted to fit FAR or coverage, plot norms to check, approvals.
    pub info: Vec<String>,
}

const USES: &[&str] = &["residential", "business", "educational", "institutional", "assembly", "industrial", "farm_house"];

impl Track1 {
    pub fn scene(&self, input: &SceneInput) -> Scene {
        build(self, input)
    }
}

fn build(t: &Track1, input: &SceneInput) -> Scene {
    let fx = &t.facts;
    let mut assumed = Vec::new();
    let mut notes = Vec::new(); // rule breaches
    let mut info = Vec::new(); // advisories: what was adjusted or needs checking
    let mut permitted = true;
    let pos = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);

    // ---- inputs ----
    let use_type = match input.use_type.as_deref() {
        Some(u) if USES.contains(&u) => u.to_string(),
        _ => {
            assumed.push(Assumed { field: "use_type", text: "residential use".into() });
            "residential".into()
        }
    };
    let residential = use_type == "residential";
    let farm = use_type == "farm_house";
    let plot_area = pos(input.plot_area).unwrap_or_else(|| {
        let a = if farm { fx.v("farm.plot_ha").map(|h| h * 10_000.0).unwrap_or(DEFAULT_PLOT_AREA) } else { DEFAULT_PLOT_AREA };
        assumed.push(Assumed { field: "plot_area", text: format!("plot area {a} sq m") });
        a
    });
    let frontage = pos(input.frontage).unwrap_or_else(|| {
        let f = round1((plot_area / DEPTH_RATIO).sqrt());
        assumed.push(Assumed { field: "frontage", text: format!("frontage {f} m (plot taken as 1 : {DEPTH_RATIO} deep)") });
        f
    });
    let depth = round1(plot_area / frontage);
    let street = pos(input.street_width).unwrap_or_else(|| {
        assumed.push(Assumed { field: "street_width", text: format!("street width {DEFAULT_STREET} m") });
        DEFAULT_STREET
    });
    let height = match (pos(input.height), pos(input.floors)) {
        (Some(h), _) => h,
        (None, Some(f)) => {
            let h = f * FLOOR_HEIGHT;
            assumed.push(Assumed { field: "floors", text: format!("height {h} m ({FLOOR_HEIGHT} m per floor)") });
            h
        }
        _ => {
            let h = if farm { fx.v("farm.height").unwrap_or(DEFAULT_HEIGHT) } else { DEFAULT_HEIGHT };
            assumed.push(Assumed { field: "height", text: format!("building height {h} m") });
            h
        }
    };
    let building_type = if !residential {
        "detached".to_string()
    } else {
        match input.building_type.as_deref() {
            Some(k @ ("detached" | "semi-detached" | "row")) => k.to_string(),
            _ => {
                // Rule 53(1): detached is for plots above 225 sq m with frontage above 12 m;
                // smaller plots are taken as row-type, the most common small-plot form.
                let det = fx.v("plot.det.area").zip(fx.v("plot.det.front"));
                let k = match det {
                    Some((a, f)) if plot_area > a && frontage > f => "detached",
                    Some(_) => "row",
                    None => "detached",
                };
                let why = if k == "row" { " (plot too small for detached under Rule 53(1))" } else { "" };
                assumed.push(Assumed { field: "building_type", text: format!("{} building{why}", if k == "row" { "row-type" } else { "detached" }) });
                k.into()
            }
        }
    };
    // Whole storeys of at least FLOOR_HEIGHT: 7.5 m is 2 floors, not 3 of 2.5 m.
    let floors = ((height / FLOOR_HEIGHT + 1e-9).floor() as u32).max(1);
    let rise = input.plot_rise.filter(|r| r.is_finite()).unwrap_or(0.0);
    let floor_h = height / floors as f64;

    if residential {
        if let Some(n) = plot_norm_note(fx, &building_type, plot_area, frontage) {
            info.push(n);
        }
    }

    if rise.abs() > 0.005 {
        let (dir, amt) = if rise > 0.0 { ("above", rise) } else { ("below", -rise) };
        info.push(format!(
            "Plot is {amt} m {dir} the road. On a level plot the height is measured from the ground at the centre of the frontage; on a sloping one from the road centre line or the average plot level, whichever is higher (Rule 2(34))."
        ));
        if rise < 0.0 {
            if let Some(p) = fx.v("plinth.min_cm").map(|c| c / 100.0) {
                let gap = round2(-rise - p);
                if gap > 0.0 {
                    info.push(format!(
                        "A {p} m plinth would still sit {gap} m below the road. Rule 68(1) asks that the plinth be placed so the site drains; raise it or plan drainage."
                    ));
                }
            }
        }
    }

    // ---- open spaces ----
    let r56_max = fx.v("r56.max_h");
    let mut covered = true;
    let mut row: Option<&HeightRow> = None;
    let (mut front, mut rear, mut left, mut right): (Option<Setback>, Option<Setback>, Option<Setback>, Option<Setback>) =
        (None, None, None, None);
    let envelope;

    if farm {
        envelope = "Farm house — Rule 17".to_string();
        if let Some(f) = fx.get("farm.mos") {
            front = Some(Setback::from_fact(f.value, "Front", f));
            rear = Some(Setback::from_fact(f.value, "Rear", f));
            left = Some(Setback::from_fact(f.value, "Side", f));
            right = left.clone();
        }
        if let Some(cap) = fx.get("farm.height") {
            if height > cap.value {
                permitted = false;
                notes.push(format!("Height {height} m is above the {} m farm-house limit (Rule 17(c)).", cap.value));
            }
        }
        if let Some(min) = fx.v("farm.plot_ha") {
            if plot_area < min * 10_000.0 {
                permitted = false;
                notes.push(format!("Plot {plot_area} sq m is under the {min} hectare minimum (Rule 17(a))."));
            }
        }
        if let Some(r) = fx.v("farm.road") {
            if street < r {
                permitted = false;
                notes.push(format!("Road {street} m is under the {r} m minimum (Rule 17(e))."));
            }
        }
    } else if r56_max.map(|m| height > m).unwrap_or(false) {
        row = pick_row(&fx.rule42, height);
        match row {
            Some(r) => {
                envelope = format!(
                    "Height above {} m — Rule 42 Table {} row {} (up to {})",
                    r56_max.unwrap(),
                    r.table,
                    r.row,
                    r.height_cap.map(|c| format!("{c} m")).unwrap_or_else(|| "any height".into())
                );
                front = Some(Setback::from_row(r.front, "Front", r));
                rear = Some(Setback::from_row(r.other, "Rear", r));
                left = Some(Setback::from_row(r.other, "Side", r));
                right = left.clone();
                for (have, need, what) in [
                    (street, r.road_min, "road width"),
                    (plot_area, r.plot_min, "plot area"),
                    (frontage, r.frontage_min, "frontage"),
                ] {
                    if have < need {
                        permitted = false;
                        let unit = if what == "plot area" { "sq m" } else { "m" };
                        notes.push(format!(
                            "Rule 42 Table {} row {} needs {what} of at least {need} {unit}; this one is {have} {unit}.",
                            r.table, r.row
                        ));
                    }
                }
                if r.table == 5 {
                    info.push("High-rise: site clearance committee approval needed first (Rule 42(4)).".into());
                }
                if !permitted {
                    let fits = fx.rule42.iter().filter(|x| street >= x.road_min && plot_area >= x.plot_min && frontage >= x.frontage_min).last();
                    info.push(match fits {
                        Some(x) => format!(
                            "This site does meet Rule 42 Table {} row {}: up to {} with {} m front and {} m other open spaces.",
                            x.table, x.row, x.height_cap.map(|c| format!("{c} m")).unwrap_or_else(|| "any height".into()), x.front, x.other
                        ),
                        None => format!("This site meets no Rule 42 row, so it is limited to {} m.", r56_max.unwrap_or(12.5)),
                    });
                }
            }
            None => {
                envelope = "Height above Rule 56 — Rule 42 table could not be read".into();
                covered = false;
            }
        }
        // Other occupancies also have Rule 57 minimums; the larger one governs each side.
        if !residential {
            let (f57, o57) = occupancy(fx, &use_type, height);
            front = larger(front, f57);
            rear = larger(rear, o57.clone());
            left = larger(left, o57.clone());
            right = larger(right, o57);
        }
    } else if residential {
        envelope = format!("Residential up to {} m — Rule 56", r56_max.unwrap_or(12.5));
        let page = t.rule("56").map(|r| r.page_start).unwrap_or(0);
        front = t.front_band(street).map(|b| Setback {
            m: b.val.parse().unwrap_or(0.0),
            label: "Front · Rule 56(1)(a)".into(),
            cite: format!("PDF page {page}"),
            quote: format!("Front open spaces — {}", squash(&b.quote)),
        });
        rear = t.rear_band(plot_area).map(|b| Setback {
            m: b.val.parse().unwrap_or(0.0), // "Nil" -> 0
            label: "Rear · Rule 56(2)(a)".into(),
            cite: format!("PDF page {}", page + 1),
            quote: format!("Rear Open Space — {}", squash(&b.quote)),
        });
        (left, right) = side_open_spaces(t, &building_type, height, frontage, page + 1);
        if street < 6.0 {
            info.push(
                "Street under 6 m: in existing developed areas the building line is 6 m from the \
                 street centre line (Rule 56(1)(b))."
                    .into(),
            );
        }
        if front.is_none() || rear.is_none() {
            covered = false;
        }
    } else {
        envelope = format!("{} building — Rule 57", title(&use_type));
        let (f57, o57) = occupancy(fx, &use_type, height);
        covered = f57.is_some();
        front = f57;
        rear = o57.clone();
        left = o57.clone();
        right = o57;
    }
    if !covered {
        notes.push("Part of the open-space table could not be read; those open spaces are not drawn.".into());
    }

    // ---- footprint ----
    let m = |s: &Option<Setback>| s.as_ref().map(|s| s.m).unwrap_or(0.0);
    let (fm, rm, lm, rtm) = (m(&front), m(&rear), m(&left), m(&right));
    let mut fw = round2((frontage - lm - rtm).max(0.0));
    let mut fd = round2((depth - fm - rm).max(0.0));
    if fw <= 0.0 || fd <= 0.0 {
        permitted = false;
        notes.push("The open spaces use up the whole plot — nothing is left to build on.".into());
    }
    let open_space_limit = round2(fw * fd);
    if let Some(cap) = row.and_then(|r| r.coverage_pct) {
        let max = plot_area * cap / 100.0;
        if fw * fd > max {
            let k = (max / (fw * fd)).sqrt();
            fw = round2(fw * k);
            fd = round2(fd * k);
            info.push(format!(
                "Footprint reduced to the {cap}% ground-coverage limit (Rule 42 Table 5 row {}).",
                row.unwrap().row
            ));
        }
    }
    // ---- FAR ----
    // The scene shows the largest building the rules allow at this height: if the floor
    // area would exceed FAR, the footprint shrinks to fit (same proportions).
    let mut far = far_check(fx, &use_type, row, plot_area, round2(fw * fd), floors);
    if let Some(f) = far.as_mut() {
        if !f.ok && fw * fd > 0.0 {
            let per_floor = f.allowed_sqm / floors as f64;
            let k = (per_floor / (fw * fd)).sqrt();
            fw = round2(fw * k);
            fd = round2(fd * k);
            info.push(format!(
                "Footprint reduced to {} sq m per floor so {floors} floor{} stay within FAR {} ({} sq m on this plot, {}). Fewer floors would allow a bigger footprint.",
                round2(fw * fd), if floors == 1 { "" } else { "s" }, f.far, f.allowed_sqm, f.source.label
            ));
            f.proposed_sqm = round2(fw * fd * floors as f64);
            f.ok = true;
        }
    }
    let footprint = round2(fw * fd);

    // ---- components ----
    let ids = input.ids();
    let active: Vec<&'static catalog::Entry> = CATALOG.iter().filter(|e| ids.iter().any(|i| i == e.id)).collect();
    let has = |id: &str| active.iter().any(|e| e.id == id);
    let ctx = Ctx {
        fx,
        use_type: &use_type,
        residential: residential || farm,
        height,
        floors,
        floor_h,
        plot_area,
        front: fm,
        rear: rm,
        left: lm,
        right: rtm,
        fw,
        fd,
        r56_max: r56_max.unwrap_or(12.5),
        balcony: has("balcony"),
        chhajja: has("chhajja"),
        voltage: input.voltage.as_deref(),
        water: input.water.as_deref(),
        rise,
    };
    let mut components = Vec::new();
    let mut not_drawn = Vec::new();
    for e in &active {
        if let Some(why) = e.not_drawn {
            not_drawn.push(Mention { id: e.id, name: e.name, rules: e.rules, why });
            continue;
        }
        if let Some(c) = build_component(&ctx, e) {
            components.push(c);
        }
    }

    // A component that breaks its rule (lake buffer, line clearance, …) breaks the scene.
    for c in components.iter().filter(|c| !c.ok) {
        permitted = false;
        notes.push(format!("{}: {}", c.name, c.notes.first().cloned().unwrap_or_default()));
    }

    // ---- plinth (always drawn) ----
    let mut plinth = fx.v("plinth.min_cm").map(|c| c / 100.0).unwrap_or(0.45);
    if has("basement") {
        if let Some(cap) = fx.v("base.plinth") {
            plinth = plinth.min(cap);
            info.push(format!(
                "Plinth drawn at {plinth} m: Rule 68(1) asks for more than {} cm, and with a basement Rule 76(2)(b) caps it at {cap} m.",
                fx.v("plinth.min_cm").unwrap_or(45.0)
            ));
        }
    }

    // ---- what the rules require that is not on ----
    let mut suggested = Vec::new();
    let mut suggest = |id: &'static str, why: String| {
        if !has(id) {
            if let Some(e) = catalog::entry(id) {
                suggested.push(Suggest { id, name: e.name, why });
            }
        }
    };
    if let Some(l) = fx.v("lift.above") {
        if height > l {
            suggest("lift", format!("Required above {l} m (Rule 83)"));
        }
    }
    if let Some(p) = fx.v("tree.per") {
        if plot_area >= p || farm {
            suggest("trees", if farm { "Rule 17(d) trees per hectare".into() } else { format!("One per {p} sq m (Rule 67)") });
        }
    }
    if let Some(p) = fx.v("rwh.plot") {
        if plot_area > p {
            suggest("rainwater_harvesting", format!("Required on plots over {p} sq m (Rule 81)"));
        }
    }
    suggest("parapet", "Required on roof edges (Rule 77)".into());

    Scene {
        plot_area_sqm: plot_area,
        frontage_m: frontage,
        depth_m: depth,
        street_width_m: street,
        height_m: height,
        floors,
        floor_h_m: round2(floor_h),
        building_type,
        use_type,
        plot_rise_m: rise,
        envelope,
        assumed,
        front,
        rear,
        side_left: left,
        side_right: right,
        open_space_limit_sqm: open_space_limit,
        footprint_w_m: fw,
        footprint_d_m: fd,
        footprint_sqm: footprint,
        plinth_m: plinth,
        roof: if farm { "sloping" } else { "flat" },
        far,
        coverage_cap_pct: row.and_then(|r| r.coverage_pct),
        covered,
        permitted,
        components,
        not_drawn,
        suggested,
        notes,
        info,
    }
}

/// Plot level relative to the road from the question, in metres (+ above, − below):
/// "plot is 1.5 m above the road", "road is 2 m higher than the plot", "सड़क से 1 मीटर नीचे",
/// or two sea levels ("road at 500 m above sea level, plot at 501.5 m above sea level").
pub fn parse_rise(q: &str) -> Option<f64> {
    let q = q.to_lowercase();
    let num = r"([0-9]+(?:\.[0-9]+)?)\s*(?:m|meters?|metres?|मीटर)";
    let up = |w: &str| ["above", "higher than", "ऊपर", "ऊँचा", "ऊंचा", "ऊँची", "ऊंची"].iter().any(|k| w.starts_with(k));
    // Two sea levels first, so "500 m above sea level" is never read as a plot-road difference.
    let re = Regex::new(&format!(r"{num}\s*(?:above (?:mean )?sea level|amsl|msl)")).ok()?;
    let (mut road, mut plot) = (None, None);
    for c in re.captures_iter(&q) {
        let start = c.get(0)?.start();
        let mut lo = start.saturating_sub(40);
        while !q.is_char_boundary(lo) {
            lo -= 1;
        }
        let before = &q[lo..start];
        let r = before.rfind("road").max(before.rfind("street"));
        let p = ["plot", "land", "site", "ground"].iter().filter_map(|k| before.rfind(k)).max();
        let v: f64 = c[1].parse().ok()?;
        match (r, p) {
            (Some(a), Some(b)) if a > b => road = Some(v),
            (Some(_), None) => road = Some(v),
            (_, Some(_)) => plot = Some(v),
            _ => {}
        }
    }
    if let (Some(r), Some(p)) = (road, plot) {
        return Some(round2(p - r));
    }
    let q = re_sea_free(&q);

    // plot ... N m above/below the road
    let re = Regex::new(&format!(
        r"(?:plot|land|site|ground|house|building)[^.?!।]*?{num}\s*(above|higher than|below|lower than)\s+[^.?!।,;]{{0,20}}?(?:road|street)"
    ))
    .ok()?;
    if let Some(c) = re.captures(&q) {
        let v: f64 = c[1].parse().ok()?;
        return Some(if up(&c[2]) { v } else { -v });
    }
    // road ... N m above/below the plot
    let re = Regex::new(&format!(
        r"(?:road|street)[^.?!।]*?{num}\s*(above|higher than|below|lower than)\s+[^.?!।,;]{{0,20}}?(?:plot|land|site|ground)"
    ))
    .ok()?;
    if let Some(c) = re.captures(&q) {
        let v: f64 = c[1].parse().ok()?;
        return Some(if up(&c[2]) { -v } else { v });
    }
    // Hindi: (प्लॉट) सड़क से N मीटर ऊँचा / नीचे
    let re = Regex::new(&format!(r"(?:सड़क|रोड)\s*से\s*{num}\s*(ऊपर|ऊँचा|ऊंचा|ऊँची|ऊंची|नीचे|नीचा|नीची)")).ok()?;
    if let Some(c) = re.captures(&q) {
        let v: f64 = c[1].parse().ok()?;
        return Some(if up(&c[2]) { v } else { -v });
    }
    None
}

/// Drop "N m above sea level" phrases so the relative patterns below cannot match them.
fn re_sea_free(q: &str) -> String {
    Regex::new(r"[0-9]+(?:\.[0-9]+)?\s*(?:m|meters?|metres?)\s*(?:above (?:mean )?sea level|amsl|msl)")
        .unwrap()
        .replace_all(q, "")
        .into_owned()
}

/// Rule 53(1) plot norms for the building type; advisory, as they govern new plots.
fn plot_norm_note(fx: &Facts, kind: &str, area: f64, frontage: f64) -> Option<String> {
    let (ok, need) = match kind {
        "detached" => {
            let (a, f) = (fx.v("plot.det.area")?, fx.v("plot.det.front")?);
            (area > a && frontage > f, format!("above {a} sq m with frontage above {f} m"))
        }
        "semi-detached" => {
            let (lo, hi, flo, fhi) = (fx.v("plot.semi.lo")?, fx.v("plot.semi.hi")?, fx.v("plot.semi.flo")?, fx.v("plot.semi.fhi")?);
            ((lo..=hi).contains(&area) && (flo..=fhi).contains(&frontage), format!("{lo}–{hi} sq m with frontage {flo}–{fhi} m"))
        }
        _ => {
            let (lo, hi, flo, fhi) = (fx.v("plot.row.lo")?, fx.v("plot.row.hi")?, fx.v("plot.row.flo")?, fx.v("plot.row.fhi")?);
            ((lo..=hi).contains(&area) && (flo..=fhi).contains(&frontage), format!("{lo}–{hi} sq m with frontage {flo}–{fhi} m"))
        }
    };
    (!ok).then(|| format!(
        "Rule 53(1) plot norms: a {kind} building is for plots {need}; this one is {area} sq m with {frontage} m frontage. These norms govern new plots, so check with the Authority."
    ))
}

/// Rule 42: the first Table 4/5 row whose height cap covers the height; the last row otherwise.
fn pick_row(rows: &[HeightRow], height: f64) -> Option<&HeightRow> {
    rows.iter().find(|r| r.height_cap.map(|c| height <= c).unwrap_or(true))
}

/// Rule 57 open spaces for non-residential uses: (front, other sides).
fn occupancy(fx: &Facts, use_type: &str, height: f64) -> (Option<Setback>, Option<Setback>) {
    let one = |id: &str| fx.get(id).map(|f| (Setback::from_fact(f.value, "Front", f), Setback::from_fact(f.value, "Side", f)));
    match use_type {
        "educational" => one("occ.edu").map(|(a, b)| (Some(a), Some(b))).unwrap_or_default(),
        "institutional" => one("occ.inst").map(|(a, b)| (Some(a), Some(b))).unwrap_or_default(),
        "assembly" => (
            fx.get("occ.asm.front").map(|f| Setback::from_fact(f.value, "Front", f)),
            fx.get("occ.asm.other").map(|f| Setback::from_fact(f.value, "Side", f)),
        ),
        "business" => (
            fx.get("occ.biz.front").map(|f| Setback::from_fact(f.value, "Front", f)),
            fx.get("occ.biz.other").map(|f| Setback::from_fact(f.value, "Side", f)),
        ),
        "industrial" => {
            let (Some(base), Some(upto), Some(step)) = (fx.get("occ.ind.base"), fx.v("occ.ind.upto"), fx.v("occ.ind.step"))
            else {
                return (None, None);
            };
            // "0.25 m for every increase of 1 meter or fraction thereof in height above 16 m"
            let extra = ((height - upto).max(0.0)).ceil() * step;
            let mos = round2(base.value + extra);
            (Some(Setback::from_fact(mos, "Front", base)), Some(Setback::from_fact(mos, "Side", base)))
        }
        _ => (None, None),
    }
}

fn larger(a: Option<Setback>, b: Option<Setback>) -> Option<Setback> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b.m > a.m { b } else { a }),
        (a, b) => a.or(b),
    }
}

fn far_check(fx: &Facts, use_type: &str, row: Option<&HeightRow>, plot: f64, footprint: f64, floors: u32) -> Option<FarCheck> {
    let mut note = None;
    let (far, source) = if let Some(r) = row {
        (r.far, Source {
            label: format!("Rule 42 Table {} row {}", r.table, r.row),
            cite: format!("PDF page {}", r.page),
            quote: r.quote.clone(),
        })
    } else {
        let id = match use_type {
            "residential" => "far.res",
            "farm_house" => "farm.far",
            "business" => {
                note = Some("Commercial FAR depends on the centre category (Rule 61 Table 8, 1.25 to 2.50); the lowest, Convenience Shopping Centre, is used.".into());
                "far.com"
            }
            "educational" | "institutional" => "far.public",
            _ => return None, // assembly / industrial: Rule 53 / 48, not modelled
        };
        let f = fx.get(id)?;
        (f.value, f.source())
    };
    let allowed = round2(far * plot);
    let proposed = round2(footprint * floors as f64);
    Some(FarCheck { far, allowed_sqm: allowed, proposed_sqm: proposed, ok: proposed <= allowed + 0.01, source, note })
}

/* ---------- components ---------- */

struct Ctx<'a> {
    fx: &'a Facts,
    use_type: &'a str,
    /// Residential stair/tread/riser rules apply (homes and farm houses).
    residential: bool,
    height: f64,
    floors: u32,
    floor_h: f64,
    plot_area: f64,
    front: f64,
    rear: f64,
    left: f64,
    right: f64,
    fw: f64,
    fd: f64,
    r56_max: f64,
    balcony: bool,
    chhajja: bool,
    voltage: Option<&'a str>,
    water: Option<&'a str>,
    /// Plot ground above the road centre line (m).
    rise: f64,
}

struct Builder {
    c: Component,
}

impl Builder {
    fn new(e: &catalog::Entry) -> Builder {
        Builder {
            c: Component {
                id: e.id,
                name: e.name,
                params: json!({}),
                sources: vec![],
                ok: true,
                notes: vec![],
                illustrative: vec![],
            },
        }
    }
    fn src(&mut self, f: Option<&Fact>) -> &mut Self {
        if let Some(f) = f {
            if !self.c.sources.iter().any(|s| s.quote == f.quote) {
                self.c.sources.push(f.source());
            }
        }
        self
    }
    fn note(&mut self, s: impl Into<String>) -> &mut Self {
        self.c.notes.push(s.into());
        self
    }
    fn fail(&mut self, s: impl Into<String>) -> &mut Self {
        self.c.ok = false;
        self.note(s)
    }
    fn drawn_as(&mut self, s: impl Into<String>) -> &mut Self {
        self.c.illustrative.push(s.into());
        self
    }
    fn done(&mut self, params: Value) -> Option<Component> {
        self.c.params = params;
        Some(self.c.clone())
    }
}

fn build_component(x: &Ctx, e: &catalog::Entry) -> Option<Component> {
    let fx = x.fx;
    let mut b = Builder::new(e);
    let f = |id: &str| fx.get(id);
    let v = |id: &str| fx.v(id);

    match e.id {
        "balcony" => {
            let (max, clear) = (v("proj.balcony")?, v("proj.balcony_clear")?);
            b.src(f("proj.balcony")).src(f("far.balcony_free"));
            let depth = round2(max.min(x.front - clear));
            if x.floors < 2 {
                b.fail("Single storey: balconies are allowed at first or higher floors only.");
            } else if depth <= 0.0 {
                b.fail(format!("Front open space {} m leaves no room: a balcony must stay {clear} m from the plot boundary.", x.front));
            } else {
                b.note(format!("Up to {max} m deep and at least {clear} m from the plot boundary → {depth} m here, first floor and above."));
                if let Some(free) = v("far.balcony_free") {
                    b.note(format!("Not counted in FAR up to {free} m wide (Rule 2(30))."));
                }
            }
            b.drawn_as("Width along the facade (half the front) — the rule only forbids a continuous projection.");
            b.done(json!({ "depth": depth.max(0.0), "from_floor": 1, "width_frac": 0.5 }))
        }
        "chhajja" => {
            let d = v("proj.sunshade")?;
            b.src(f("proj.sunshade")).src(f("proj.cornice"));
            let depth = round2(d.min(x.front));
            b.note(format!("Sunshade over each window, at most {d} m wide."));
            b.drawn_as("Window positions and widths.");
            b.done(json!({ "depth": depth }))
        }
        "parapet" => {
            let h = v("parapet.h")?;
            b.src(f("parapet.h")).note(format!("At least {h} m above the roof finish."));
            b.done(json!({ "height": h }))
        }
        "boundary_wall" => {
            let (fh, oh) = (v("wall.front")?, v("wall.other")?);
            b.src(f("wall.front")).src(f("wall.other"));
            b.note(format!("Front wall at most {fh} m; side and rear at most {oh} m (no service road)."));
            // Rule 78 measures these above the centre line of the front street.
            let (fh, oh) = (round2((fh - x.rise).max(0.0)), round2((oh - x.rise).max(0.0)));
            if x.rise.abs() > 0.005 {
                b.src(f("wall.measure"));
                b.note(format!(
                    "Measured above the street centre line: with the plot {} m {} the road, the walls rise at most {fh} m (front) and {oh} m (others) above the plot.",
                    round2(x.rise.abs()), if x.rise > 0.0 { "above" } else { "below" }
                ));
                if fh == 0.0 {
                    b.note("The plot is higher than the front-wall limit: no front wall above the plot ground is allowed.");
                }
            }
            let needs_fire_gate = !x.residential || x.height > x.r56_max;
            let gate = if needs_fire_gate {
                let g = v("gate.width")?;
                b.src(f("gate.width")).note(format!("Gate at least {g} m wide for fire engines (Rule 41(c) / 42(6))."));
                g
            } else {
                b.drawn_as("Gate width 3 m — the rules set none for homes.");
                3.0
            };
            b.done(json!({ "front_h": fh, "other_h": oh, "gate_w": gate }))
        }
        "staircase" => {
            let (wid, wkey) = match x.use_type {
                _ if x.residential => ("stair.w.res", "residential"),
                "assembly" => ("stair.w.asm", "assembly"),
                "educational" if x.height > 24.0 => ("stair.w.edu_high", "educational over 24 m"),
                "educational" => ("stair.w.edu_low", "educational up to 24 m"),
                "institutional" => ("stair.w.inst", "institutional up to 10 beds"),
                _ => ("stair.w.other", "other buildings"),
            };
            let width = v(wid)?;
            let (tread, riser_max) = if x.residential {
                (v("stair.tread.res_cm")? / 100.0, v("stair.riser.res_cm")? / 100.0)
            } else {
                (v("stair.tread.other_cm")? / 100.0, v("stair.riser.other_cm")? / 100.0)
            };
            let per_flight = v("stair.per_flight")?;
            let landing = round2(2.0 * tread + v("stair.landing_cm")? / 100.0);
            let headroom = v("stair.headroom")?;
            let n = (x.floor_h / riser_max - 1e-9).ceil().max(2.0);
            let riser = x.floor_h / n;
            let per = (n / 2.0).ceil(); // dog-leg: two flights per floor
            let run = (per - 1.0) * tread;
            let core_w = round2(2.0 * width + 0.1);
            let core_d = round2(run + 2.0 * landing);
            let (tid, rid) = if x.residential {
                ("stair.tread.res_cm", "stair.riser.res_cm")
            } else {
                ("stair.tread.other_cm", "stair.riser.other_cm")
            };
            for id in [wid, tid, rid, "stair.landing_cm", "stair.headroom"] {
                b.src(f(id));
            }
            b.note(format!(
                "{wkey}: width {width} m, tread {} cm, riser {} cm here ({} risers per floor, max {} cm), landing {landing} m, headroom {headroom} m.",
                round1(tread * 100.0), round1(riser * 100.0), n, round1(riser_max * 100.0)
            ));
            if per > per_flight {
                b.fail(format!("{per} risers in a flight exceeds the {per_flight} allowed — needs more flights."));
            }
            if core_w >= x.fw || core_d >= x.fd {
                b.fail(format!("Stair core {core_w} × {core_d} m does not fit the {} × {} m footprint.", x.fw, x.fd));
            }
            b.drawn_as("Dog-leg layout and its position at the rear corner; 0.1 m gap between flights.");
            b.done(json!({
                "width": width, "tread": tread, "riser": round2(riser * 100.0) / 100.0, "risers_per_floor": n,
                "per_flight": per, "landing": landing, "headroom": headroom, "core_w": core_w, "core_d": core_d
            }))
        }
        "lift" => {
            let above = v("lift.above")?;
            b.src(f("lift.above"));
            let required = x.height > above;
            b.note(if required {
                format!("Required: building is above {above} m (Rule 83).")
            } else {
                format!("Not required at {} m (only above {above} m); drawn as asked.", x.height)
            });
            b.drawn_as("Shaft 2 × 2 m and machine room 2.5 m — car sizes are in NBC Part VIII, not these rules.");
            b.done(json!({ "required": required, "shaft_w": 2.0, "shaft_d": 2.0, "machine_h": 2.5 }))
        }
        "ventilation_shaft" => {
            let (area, side, fact) = fx.shaft(x.height)?;
            b.src(Some(fact));
            let other = round2(side.max(area / side));
            b.note(format!("For a {} m building: at least {area} sq m, sides at least {side} m → {side} × {other} m.", x.height));
            if x.height > 30.0 {
                b.note("Above 30 m mechanical ventilation is also required (Rule 56(6)(b) Note 1).");
            }
            b.drawn_as("Position inside the footprint.");
            b.done(json!({ "a": side, "b": other, "area": area }))
        }
        "courtyard" => {
            let (min_side, frac) = (v("court.side")?, v("court.frac")?);
            b.src(f("court.side")).src(f("court.frac"));
            let side = round2(min_side.max(x.height * frac));
            b.note(format!(
                "Side at least {min_side} m and area at least (height ÷ 5)² = {} sq m → {side} × {side} m open to sky.",
                round2((x.height * frac).powi(2))
            ));
            if side + 3.0 > x.fw || side + 3.0 > x.fd {
                b.fail(format!("A {side} m courtyard leaves less than 1.5 m of building around it in a {} × {} m footprint.", x.fw, x.fd));
            }
            b.done(json!({ "side": side }))
        }
        "stilt_parking" => {
            let (h, min_plot) = (v("stilt.h")?, v("stilt.plot")?);
            let (bw, bl) = (v("garage.w")?, v("garage.l")?);
            b.src(f("stilt.h"));
            if x.plot_area < min_plot {
                b.fail(format!("Covered parking is for plots of {min_plot} sq m or more; this one is {} sq m.", x.plot_area));
            }
            b.note(format!("Clear height {h} m, within the ground coverage; not counted in FAR or building height."));
            let cars = ((x.fw / bw).floor() * (x.fd / bl).floor().max(1.0)).max(0.0);
            b.drawn_as(format!("Car bays drawn at the Rule 75(a) private-garage size {bw} × {bl} m; parking norms are in Appendix 1."));
            b.done(json!({ "clear_h": h, "bay_w": bw, "bay_l": bl, "cars": cars }))
        }
        "basement" => {
            let h = v("base.h")?;
            b.src(f("base.h")).src(f("base.plinth"));
            b.note(format!("Clear height at least {h} m; must stay inside the open-space lines (Rule 76(2))."));
            if x.use_type == "business" {
                if let Some(hc) = v("base.h_com") {
                    b.src(f("base.h_com"));
                    b.note(format!("{hc} m if the basement itself is used for shops, which Rule 76(3)(e) allows only on plots of 1 hectare or more."));
                }
            }
            b.note("Not habitable; parking, storage and services only (Rule 76(3)).");
            b.done(json!({ "clear_h": h }))
        }
        "garage" => {
            let (w, l) = (v("garage.w")?, v("garage.l")?);
            b.src(f("garage.w")).src(f("garage.free_sqm"));
            let (side, room) = if x.left >= x.right { ("left", x.left) } else { ("right", x.right) };
            if room < w {
                b.fail(format!("Needs a side open space at least {w} m wide (Rule 56(4) puts it at the rear end of one); widest here is {room} m."));
            } else {
                b.note(format!("{w} × {l} m at the rear end of the {side} open space (Rule 56(4)); up to 25 sq m is not counted as ground coverage."));
            }
            b.done(json!({ "w": w, "l": l, "side": side }))
        }
        "trees" => {
            let count = if x.use_type == "farm_house" {
                let per_ha = v("farm.trees_ha")?;
                b.src(f("farm.trees_ha"));
                (per_ha * x.plot_area / 10_000.0).ceil()
            } else {
                let per = v("tree.per")?;
                b.src(f("tree.per"));
                (x.plot_area / per).floor()
            };
            if count < 1.0 {
                b.fail("Plot is under 100 sq m: Rule 67 asks for none.");
            } else {
                b.note(format!("{count} trees for a {} sq m plot.", x.plot_area));
            }
            b.drawn_as("Tree positions in the open spaces (shown at most 60).");
            b.done(json!({ "count": count }))
        }
        "septic_tank" => {
            let w = v("septic.w_cm")? / 100.0;
            let dep = v("septic.depth")?;
            let len = round2((v("septic.len_min")? * w).max(v("septic.cap")? / (w * dep)));
            let gap = v("septic.gap")?;
            b.src(f("septic.w_cm")).src(f("septic.len_min")).src(f("septic.gap"));
            let need = gap + w;
            let zone = if x.rear >= need {
                "rear"
            } else if x.front >= need {
                "front"
            } else if x.left.max(x.right) >= need {
                if x.left >= x.right { "left" } else { "right" }
            } else {
                b.fail(format!("No open space is {need} m deep: the tank must stay {gap} m from the building."));
                "rear"
            };
            if b.c.ok {
                b.note(format!("Fits in the {zone} open space, {gap} m from the building."));
            }
            b.note(format!("{w} m wide, {dep} m deep, {len} m long (2–4 × width, ≥ 1 cu m), {gap} m from the building."));
            b.done(json!({ "w": w, "len": len, "depth": dep, "gap": gap, "zone": zone }))
        }
        "rainwater_harvesting" => {
            let p = v("rwh.plot")?;
            b.src(f("rwh.plot"));
            let required = x.plot_area > p;
            b.note(if required { format!("Required: plot is over {p} sq m.") } else { format!("Not required at or under {p} sq m.") });
            b.drawn_as("Pit marker only — the designs are in Appendix H-1 to H-3, not in this dataset.");
            b.done(json!({ "required": required }))
        }
        "electric_line" => {
            let class = x.voltage.unwrap_or("lv");
            if x.voltage.is_none() {
                b.drawn_as("Low/medium voltage assumed — say \"11 kV\", \"33 kV\" or \"HT line\" to change.");
            }
            let (vc, hc, label) = match class {
                "hv" => (v("line.hv.v"), v("line.hv.h")?, "high voltage up to 33 kV"),
                "ehv" => (None, v("line.ehv.half")?, "extra high voltage above 33 kV"),
                _ => (v("line.lv.v"), v("line.lv.h")?, "low / medium voltage"),
            };
            b.src(f(match class { "hv" => "line.hv.h", "ehv" => "line.ehv.half", _ => "line.lv.h" }));
            let proj = if x.balcony { v("proj.balcony")?.min((x.front - v("proj.balcony_clear")?).max(0.0)) } else { 0.0 }
                .max(if x.chhajja { v("proj.sunshade").unwrap_or(0.0) } else { 0.0 });
            let gap = round2(x.front - proj);
            let from = match (x.balcony && proj > 0.0, x.chhajja) {
                (true, _) => "the balcony edge",
                (false, true) => "the chhajja edge",
                _ => "the building",
            };
            if gap < hc {
                b.fail(format!("Line along the front boundary is {gap} m from {from}; Rule 52 needs {hc} m horizontally ({label})."));
            } else {
                b.note(format!("Allowed: line along the front boundary is {gap} m from {from}; Rule 52 needs at least {hc} m horizontally ({label})."));
            }
            if let Some(vc) = vc {
                b.note(format!("Vertical clearance at least {vc} m (from maximum sag)."));
            }
            b.drawn_as("Line runs along the front plot boundary; pole and wire heights are not set by these rules.");
            let wire_h = match class { "ehv" => 22.0, "hv" => 9.0, _ => 7.0 };
            b.done(json!({ "class": class, "v": vc, "h": hc, "wire_h": wire_h, "gap": gap }))
        }
        "water_body" => {
            let kind = x.water.unwrap_or("lake");
            let id = if kind == "river" { "water.river" } else { "water.buffer" };
            let buffer = v(id)?;
            b.src(f(id));
            if x.rear < buffer {
                b.fail(format!("Building is {} m from the water's edge; Rule 50(b) needs {buffer} m from the highest water level.", x.rear));
            } else {
                b.note(format!("{} m from the water's edge; at least {buffer} m needed.", x.rear));
            }
            b.drawn_as(format!("The {kind} is placed right behind the rear boundary."));
            b.done(json!({ "kind": kind, "buffer": buffer }))
        }
        _ => None,
    }
}

/// Rule 56(3): side open spaces, read from the rule text including its provisos.
fn side_open_spaces(t: &Track1, kind: &str, height: f64, frontage: f64, page: u32) -> (Option<Setback>, Option<Setback>) {
    let Some(r) = t.rule("56") else { return (None, None) };
    let text = squash(&section(&r.text, "Side open space", "(4) Notwithstanding"));
    let set = |m: f64, clause: &str, quote: &str| {
        Some(Setback {
            m,
            label: format!("Side · Rule 56(3){clause}"),
            cite: format!("PDF page {page}"),
            quote: quote.to_string(),
        })
    };
    let grab = |re: &str| {
        Regex::new(re).ok().and_then(|re| re.captures(&text)).map(|c| {
            let nums: Vec<f64> = c.iter().skip(1).filter_map(|g| g?.as_str().parse().ok()).collect();
            (nums, c[0].to_string())
        })
    };

    match kind {
        "row" => (None, None),
        "semi-detached" => {
            let Some((base, q)) = grab(r"(?i)minimum side open space of ([0-9.]+) meters on one side") else {
                return (None, None);
            };
            let reduced = grab(
                r"(?i)Semi-detached building up to ([0-9.]+) meters in height on plots with a frontage up to ([0-9.]+) meters, the side open space may be reduced to ([0-9.]+)",
            );
            match reduced {
                Some((n, rq)) if height <= n[0] && frontage <= n[1] => (set(n[2], "(ii) proviso", &rq), None),
                _ => (set(base[0], "(ii)", &q), None),
            }
        }
        _ => {
            let Some((base, q)) = grab(r"(?i)minimum side open spaces of ([0-9.]+) meters on both the sides") else {
                return (None, None);
            };
            let reduced = grab(
                r"(?i)up to ([0-9.]+) meters in height on plots with a frontage less than ([0-9.]+) meters, one of the sides open space may be reduced to ([0-9.]+)",
            );
            let left = set(base[0], "(i)", &q);
            match reduced {
                Some((n, rq)) if height <= n[0] && frontage < n[1] => (left, set(n[2], "(i) proviso", &rq)),
                _ => (left.clone(), left),
            }
        }
    }
}

fn title(u: &str) -> String {
    let mut c = u.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
