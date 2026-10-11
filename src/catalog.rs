//! Catalog of 3D components found by reading all 106 rules.
//! `docs/3d-catalog.md` is the human version, with every rule-derived value.
//!
//! A component is either drawn (it has a builder in `scene.rs` and `static/scene3d.js`)
//! or catalogued only, with the reason it is not drawn. Questions are matched to
//! components by trigger words (EN + HI); the model may also pick ids from this list,
//! but never supplies a dimension.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub rules: &'static str,
    /// Lower-case substrings matched against the question.
    #[serde(skip)]
    pub triggers: &'static [&'static str],
    /// None when drawn; otherwise why it is not.
    pub not_drawn: Option<&'static str>,
}

const fn drawn(
    id: &'static str,
    name: &'static str,
    group: &'static str,
    rules: &'static str,
    triggers: &'static [&'static str],
) -> Entry {
    Entry { id, name, group, rules, triggers, not_drawn: None }
}

const fn listed(
    id: &'static str,
    name: &'static str,
    group: &'static str,
    rules: &'static str,
    triggers: &'static [&'static str],
    why: &'static str,
) -> Entry {
    Entry { id, name, group, rules, triggers, not_drawn: Some(why) }
}

const INTERIOR: &str = "Room-level interiors are not modelled; the rule sizes are in the catalog.";
const SPECIAL: &str = "Special building type with its own plot, road and parking norms; catalogued, drawn as a generic building.";
const LAYOUT: &str = "Layout-scale (many plots / roads); the 3D view models one plot.";

pub const CATALOG: &[Entry] = &[
    // ---- drawn: on the building ----
    drawn("balcony", "Balcony", "Building", "58(1)(c), 2(30)", &["balcony", "balconies", "gallery", "बालकनी", "छज्जा बालकनी"]),
    drawn("chhajja", "Chhajja / sunshade", "Building", "58(1)(a)-(b)", &["chhajja", "chajja", "sunshade", "sun shade", "weather shade", "cornice", "छज्जा", "सनशेड"]),
    drawn("parapet", "Parapet", "Building", "77", &["parapet", "terrace wall", "roof wall", "railing", "मुंडेर", "पैरापेट"]),
    drawn("staircase", "Staircase", "Building", "80", &["stair", "staircase", "steps", "riser", "tread", "सीढ़ी", "सीढ़ियाँ", "जीना"]),
    drawn("lift", "Lift", "Building", "83", &["lift", "elevator", "लिफ्ट"]),
    drawn("ventilation_shaft", "Ventilation shaft", "Building", "56(6)(b)", &["shaft", "duct", "ventilation", "वेंटिलेशन", "डक्ट"]),
    drawn("courtyard", "Inner courtyard", "Building", "56(6)(a)", &["courtyard", "court yard", "chowk", "aangan", "angan", "आंगन", "आँगन", "चौक"]),
    drawn("stilt_parking", "Stilt (covered) parking", "Building", "61 Note (4), 2(54)", &["stilt", "parking", "car park", "पार्किंग", "स्टिल्ट"]),
    drawn("basement", "Basement", "Building", "76", &["basement", "cellar", "underground", "बेसमेंट", "तलघर", "तहखाना"]),
    // ---- drawn: on the plot ----
    drawn("boundary_wall", "Boundary wall + gate", "Plot", "78, 41(c), 42(6)", &["boundary wall", "compound wall", "boundary", "gate", "fence", "चारदीवारी", "बाउंड्री", "दीवार", "गेट"]),
    drawn("garage", "Private garage", "Plot", "75(a), 56(4), 2(20)", &["garage", "गैरेज"]),
    drawn("trees", "Trees (plantation)", "Plot", "67, 17(d)", &["tree", "plantation", "plant", "green", "पेड़", "वृक्ष", "पौधे"]),
    drawn("septic_tank", "Septic tank", "Plot", "79", &["septic", "soak pit", "soakpit", "sewage tank", "सेप्टिक", "सोख्ता"]),
    drawn("rainwater_harvesting", "Rainwater harvesting", "Plot", "81", &["rainwater", "rain water", "harvesting", "recharge pit", "वर्षा जल", "रेन वाटर"]),
    // ---- drawn: around the plot ----
    drawn("electric_line", "Overhead electric line", "Surroundings", "52", &["electric line", "power line", "overhead", "wire", "ht line", "high tension", "high voltage", "kv", "transmission line", "बिजली", "हाई टेंशन", "तार"]),
    drawn("water_body", "Lake / river nearby", "Surroundings", "50(b)", &["lake", "river", "pond", "talab", "tank bund", "nala", "nallah", "water body", "तालाब", "नदी", "झील", "नाला"]),
    // ---- catalogued only ----
    listed("podium", "Podium", "Building", "62, 2(54)", &["podium", "पोडियम"], "Needs a 1000 sq m+ plot and multi-building layout; values catalogued (2.4 m clear, max 3 floors)."),
    listed("parking_floor", "Parking floor", "Building", "63", &["parking floor"], "Needs a 1000 sq m+ plot; values catalogued (2.4 m clear)."),
    listed("service_floor", "Service floor", "Building", "64, 2(63)", &["service floor"], "Values catalogued (2 m clear, 1000 sq m+ plot)."),
    listed("mezzanine", "Mezzanine", "Building", "73, 2(43)", &["mezzanine", "मेज़नाइन", "मेजेनाइन"], INTERIOR),
    listed("loft", "Loft / ledge (tand)", "Building", "72, 2(39), 2(42)", &["loft", "ledge", "tand", "टांड", "दुछत्ती"], INTERIOR),
    listed("habitable_room", "Habitable room", "Rooms", "69", &["bedroom", "living room", "habitable", "room size", "कमरा"], INTERIOR),
    listed("kitchen", "Kitchen", "Rooms", "70", &["kitchen", "रसोई"], INTERIOR),
    listed("bathroom_wc", "Bathroom / WC", "Rooms", "71", &["bathroom", "toilet", "wc", "latrine", "शौचालय", "बाथरूम"], INTERIOR),
    listed("store_room", "Store room", "Rooms", "74", &["store room", "storeroom"], INTERIOR),
    listed("windows", "Windows / lighting", "Rooms", "82", &["window", "light and air", "खिड़की"], INTERIOR),
    listed("projecting_room", "Projecting room", "Building", "58(3)", &["projecting room", "cantilever room"], "Rule gives conditions but no depth limit."),
    listed("skyway", "Skyway", "Surroundings", "66(1)", &["skyway", "sky walk", "skywalk", "bridge between"], "Needs two buildings; values catalogued (6.5 m clear, max 4 m wide)."),
    listed("subway", "Pedestrian subway", "Surroundings", "66(2)", &["subway", "underpass"], "Values catalogued (3.0 m clear height, 4.5 m width)."),
    listed("access_road", "Means of access (road width by length)", "Layout", "38, 39, 41(a)", &["access road", "approach road", "means of access"], LAYOUT),
    listed("cul_de_sac", "Cul-de-sac turning space", "Layout", "43", &["cul-de-sac", "cul de sac", "dead end"], LAYOUT),
    listed("alley", "Alley", "Layout", "44", &["alley", "गली"], LAYOUT),
    listed("community_open_space", "Community open space", "Layout", "47", &["park", "community open space", "playground"], LAYOUT),
    listed("group_housing", "Group housing", "Special", "60", &["group housing", "apartment complex", "society", "township"], SPECIAL),
    listed("cinema", "Cinema (single screen)", "Special", "53(3)(ii), 94", &["cinema", "theatre", "theater", "सिनेमा"], SPECIAL),
    listed("multiplex", "Multiplex", "Special", "95", &["multiplex", "मल्टीप्लेक्स"], SPECIAL),
    listed("shopping_mall", "Shopping / open mall", "Special", "96", &["mall", "मॉल"], SPECIAL),
    listed("marriage_garden", "Marriage garden", "Special", "53(3)(iii)", &["marriage garden", "marriage hall", "मैरिज गार्डन"], SPECIAL),
    listed("fuel_station", "Fuel station", "Special", "53(3)(iv)", &["petrol pump", "fuel station", "filling station", "पेट्रोल पंप"], SPECIAL),
    listed("ev_charging", "EV / CNG station", "Special", "53(3)(C)", &["ev charging", "charging station", "cng"], SPECIAL),
    listed("industrial_plot", "Industrial plot table", "Special", "48", &["industrial plot", "flatted industr"], "Rule 48 plot-area table catalogued; the 3D view uses Rule 57(e) for industrial open spaces."),
    listed("servant_quarter", "Servant quarter / watchman hut", "Plot", "61 Note (3), 58(4), 2(20)", &["servant quarter", "watchman", "chowkidar", "pump room"], "Size caps only (25 / 6 sq m); position is up to the owner."),
    listed("railway", "Railway nearby", "Surroundings", "50(k)", &["railway", "रेलवे"], "Distance is set by the Railway, not by these rules."),
];

/// Use types the 3D view knows. Rule 54 classes, grouped by which open-space rule applies.
pub const USES: &[(&str, &[&str])] = &[
    ("farm_house", &["farm house", "farmhouse", "farm-house", "फार्म हाउस", "फार्महाउस"]),
    ("industrial", &["factory", "industrial", "industry", "workshop", "warehouse", "फैक्ट्री", "कारखाना", "उद्योग"]),
    ("assembly", &["auditorium", "assembly", "community hall", "cinema", "theatre", "theater", "सभागार"]),
    ("educational", &["school", "college", "coaching", "educational", "स्कूल", "विद्यालय", "कॉलेज"]),
    ("institutional", &["hospital", "clinic", "nursing home", "institutional", "अस्पताल"]),
    ("business", &["shop", "office", "commercial", "mercantile", "showroom", "store building", "storage", "godown", "दुकान", "ऑफिस", "व्यावसायिक", "गोदाम"]),
];

pub fn entry(id: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.id == id)
}

/// Catalog ids whose trigger words appear in the question.
pub fn match_components(q: &str) -> Vec<&'static str> {
    let q = q.to_lowercase();
    CATALOG
        .iter()
        .filter(|e| e.triggers.iter().any(|t| contains_word(&q, t)))
        .map(|e| e.id)
        .collect()
}

pub fn match_use(q: &str) -> Option<&'static str> {
    let q = q.to_lowercase();
    USES.iter()
        .find(|(_, ts)| ts.iter().any(|t| contains_word(&q, t)))
        .map(|(u, _)| *u)
}

/// Substring match that does not fire inside a longer ASCII word
/// ("wc" must not match "wcs" is fine, but "kv" must not match "kvass"; "lift" not "uplift").
pub fn contains_word(q: &str, t: &str) -> bool {
    let ascii = t.chars().all(|c| c.is_ascii());
    if !ascii {
        return q.contains(t);
    }
    let mut from = 0;
    while let Some(i) = q[from..].find(t) {
        let s = from + i;
        let e = s + t.len();
        let before = q[..s].chars().next_back().map_or(true, |c| !c.is_ascii_alphanumeric());
        // Allow plural / suffix forms after the trigger ("trees", "balconies" via own trigger).
        let after = q[e..].chars().next().map_or(true, |c| !c.is_ascii_alphanumeric() || c == 's');
        if before && after {
            return true;
        }
        from = e;
    }
    false
}

/// Overhead line class from the question (Rule 52 rows).
pub fn match_voltage(q: &str) -> Option<&'static str> {
    let q = q.to_lowercase();
    let kv = regex::Regex::new(r"(\d+)\s*kv").unwrap();
    if let Some(c) = kv.captures(&q) {
        let n: f64 = c[1].parse().unwrap_or(0.0);
        return Some(if n > 33.0 { "ehv" } else { "hv" });
    }
    if ["extra high", "ehv", "transmission", "tower line"].iter().any(|k| q.contains(k)) {
        Some("ehv")
    } else if ["high tension", "high voltage", "ht line", "हाई टेंशन"].iter().any(|k| q.contains(k)) {
        Some("hv")
    } else if ["low tension", "lt line", "service line", "low voltage"].iter().any(|k| q.contains(k)) {
        Some("lv")
    } else {
        None
    }
}

pub fn match_water(q: &str) -> Option<&'static str> {
    let q = q.to_lowercase();
    if ["river", "नदी"].iter().any(|k| q.contains(k)) {
        Some("river")
    } else if ["lake", "pond", "talab", "nala", "nallah", "water body", "तालाब", "झील", "नाला"].iter().any(|k| q.contains(k)) {
        Some("lake")
    } else {
        None
    }
}
