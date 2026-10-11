//! Every number the 3D scene uses, read out of the rule text.
//!
//! Each fact is a pattern over the rule's text (whitespace collapsed). The value is a
//! capture group, the quote is the whole match, and the page comes from the running
//! page header before the match. If the rulebook text changes and a pattern stops
//! matching, the fact is missing and whatever depends on it is not drawn — it is never
//! replaced with a typed-in number. `tests/sample_questions.rs` checks they all resolve.

use crate::corpus::Corpus;
use crate::models::Source;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct Fact {
    pub value: f64,
    pub rule: &'static str,
    pub clause: &'static str,
    pub page: u32,
    pub quote: String,
}

impl Fact {
    pub fn source(&self) -> Source {
        Source {
            label: format!("Rule {}{}", self.rule, self.clause),
            cite: format!("PDF page {}", self.page),
            quote: self.quote.clone(),
        }
    }
}

/// (id, rule, clause, pattern, capture group)
type Def = (&'static str, &'static str, &'static str, &'static str, usize);

// Rule 42 tables are flattened column-by-column in the source text; these patterns
// follow the flattened order. Groups: road, plot, frontage, FAR, height cap, front, other.
const T4: &str = r"{n}\. ([\d.]+) m\.(?: &)? (\d+) sq\. (\d+) m\. 1:([\d.]+) -\]? Up to (\d+) ([\d.]+) ([\d.]+)";
// Groups: road, plot, frontage, FAR, coverage %, height cap (absent on the last row), front, other.
const T5: &str = r"{n}\. ([\d.]+) m\. (\d+) (\d+) m\. 1:([\d.]+) (\d+) (?:Up to (\d+)|Above) ([\d.]+)(?: m\.)? ([\d.]+)";

// Rule 56(6)(b) ventilation shaft table: (height, area, min side) x 5, then "above 30" (area, side).
const SHAFT: &str = r"Up to (10) ([\d.]+) ([\d.]+) (12\.5) ([\d.]+) ([\d.]+) (18) ([\d.]+) ([\d.]+) (24) ([\d.]+) ([\d.]+) (30) ([\d.]+) ([\d.]+) above 30 ([\d.]+) ([\d.]+)";

const DEFS: &[Def] = &[
    // Rule 2 — definitions
    ("far.balcony_free", "2", "(30)", r"balcony \(upto a width of ([\d.]+) mtrs\.\)", 1),
    ("garage.free_sqm", "2", "(20)", r"private garage not exceeding (\d+) sq\. meters", 1),
    // Rule 16(4)(j) — contours on sloping sites
    ("slope.contour", "16", "(4)(j)", r"contours at an interval of ([\d.]+) meters in case of the site which has a slope more than 1:(\d+)", 1),
    ("slope.ratio", "16", "(4)(j)", r"contours at an interval of ([\d.]+) meters in case of the site which has a slope more than 1:(\d+)", 2),
    // Rule 53(1) — plot size and frontage by building type
    ("plot.det.area", "53", "(1)(i)", r"Detached building above (\d+) above (\d+)", 1),
    ("plot.det.front", "53", "(1)(i)", r"Detached building above (\d+) above (\d+)", 2),
    ("plot.semi.lo", "53", "(1)(i)", r"Semi-detached building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 1),
    ("plot.semi.hi", "53", "(1)(i)", r"Semi-detached building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 2),
    ("plot.semi.flo", "53", "(1)(i)", r"Semi-detached building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 3),
    ("plot.semi.fhi", "53", "(1)(i)", r"Semi-detached building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 4),
    ("plot.row.lo", "53", "(1)(i)", r"Row type building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 1),
    ("plot.row.hi", "53", "(1)(i)", r"Row type building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 2),
    ("plot.row.flo", "53", "(1)(i)", r"Row type building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 3),
    ("plot.row.fhi", "53", "(1)(i)", r"Row type building (\d+)-(\d+) ([\d.]+) to ([\d.]+)", 4),
    // Rule 56(5) — where Rule 56 stops and Rule 42 takes over
    ("r56.max_h", "56", "(5)", r"For building of height above ([\d.]+) meters the open spaces around building shall be as given in Table 4 and 5 of Rule 42", 1),
    // Rule 17 — farm house
    ("farm.plot_ha", "17", "(a)", r"Minimum size of the plot shall be ([\d.]+) hectare", 1),
    ("farm.far", "17", "(b)", r"Maximum F\.A\.R\. shall be ([\d.]+)", 1),
    ("farm.height", "17", "(c)", r"maximum height of structure shall be ([\d.]+) meters with sloping roof", 1),
    ("farm.trees_ha", "17", "(d)", r"minimum of (\d+) live trees per hectare", 1),
    ("farm.road", "17", "(e)", r"Farm House shall be accessible by road having width not less than ([\d.]+) meters", 1),
    ("farm.mos", "17", "(f)", r"minimum ([\d.]+) meters open space \(MOS\) from all sides", 1),
    // Rule 41 — gate for non-residential buildings (42(6) repeats it above 12.5 m)
    ("gate.width", "41", "(c)", r"in no case it shall measure less than ([\d.]+) meters", 1),
    // Rule 50 — distance from water
    ("water.buffer", "50", "(b)", r"within a distance of (\d+) meters of the highest water level of a water body/water course and (\d+) mtrs\. in case of river", 1),
    ("water.river", "50", "(b)", r"within a distance of (\d+) meters of the highest water level of a water body/water course and (\d+) mtrs\. in case of river", 2),
    // Rule 52 — overhead electric lines
    ("line.lv.v", "52", "(1)(a)", r"Low and Medium voltage ([\d.]+) ([\d.]+) lines and service lines", 1),
    ("line.lv.h", "52", "(1)(a)", r"Low and Medium voltage ([\d.]+) ([\d.]+) lines and service lines", 2),
    ("line.hv.v", "52", "(1)(b)", r"High voltage lines up to and ([\d.]+) ([\d.]+) including 33000 volts", 1),
    ("line.hv.h", "52", "(1)(b)", r"High voltage lines up to and ([\d.]+) ([\d.]+) including 33000 volts", 2),
    ("line.ehv.half", "52", "(1) Note", r"clear horizontal corridor of (\d+) meters \(([\d.]+) meter open both side of centre line\)", 2),
    // Rule 56(6) — courtyards and ventilation shafts
    ("court.side", "56", "(6)(a)(i)", r"inner courtyard, having minimum side of (\d+) meters", 1),
    ("court.frac", "56", "(6)(a)(ii)", r"square of (one-fifth) the height of the highest wall", 1),
    // Rule 57 — open spaces for other occupancies
    ("occ.edu", "57", "(a)", r"Except for nursery school, the open spaces around the building shall be not less than ([\d.]+) meters", 1),
    ("occ.inst", "57", "(b)", r"Institutional Building - The open spaces around the building shall not be less than ([\d.]+) meters", 1),
    ("occ.asm.front", "57", "(c)", r"Assembly Building- The open space at front shall not be less than ([\d.]+) meters and other spaces around the building shall not be less than ([\d.]+) meters", 1),
    ("occ.asm.other", "57", "(c)", r"Assembly Building- The open space at front shall not be less than ([\d.]+) meters and other spaces around the building shall not be less than ([\d.]+) meters", 2),
    ("occ.biz.front", "57", "(d)", r"Storage Buildings - The open spaces shall not be less than ([\d.]+) meters in the front and ([\d.]+) meters on other three sides", 1),
    ("occ.biz.other", "57", "(d)", r"Storage Buildings - The open spaces shall not be less than ([\d.]+) meters in the front and ([\d.]+) meters on other three sides", 2),
    ("occ.ind.base", "57", "(e)", r"Industrial Buildings - The open spaces around the building shall not be less than ([\d.]+) meters for heights up to ([\d.]+) meters with an increase of the open spaces of ([\d.]+) meters", 1),
    ("occ.ind.upto", "57", "(e)", r"Industrial Buildings - The open spaces around the building shall not be less than ([\d.]+) meters for heights up to ([\d.]+) meters with an increase of the open spaces of ([\d.]+) meters", 2),
    ("occ.ind.step", "57", "(e)", r"Industrial Buildings - The open spaces around the building shall not be less than ([\d.]+) meters for heights up to ([\d.]+) meters with an increase of the open spaces of ([\d.]+) meters", 3),
    // Rule 58 — projections
    ("proj.cornice", "58", "(1)(a)", r"Cornice, roof or weather shade not more than ([\d.]+) meter wide", 1),
    ("proj.sunshade", "58", "(1)(b)", r"Sunshade over window/ventilators or other openings not more than ([\d.]+) meters wide", 1),
    ("proj.balcony", "58", "(1)(c)", r"Projected balcony at first or higher floors extending upto ([\d.]+) meters with clearance of ([\d.]+) meters from plot boundary", 1),
    ("proj.balcony_clear", "58", "(1)(c)", r"Projected balcony at first or higher floors extending upto ([\d.]+) meters with clearance of ([\d.]+) meters from plot boundary", 2),
    // Rule 61 — FAR (Table 8) and covered (stilt) parking
    ("far.res", "61", " Table 8", r"1\. Resdential ([\d.]+)", 1),
    ("far.com", "61", " Table 8", r"([\d.]+) Convenience Shopping Centre", 1),
    ("far.public", "61", " Table 8", r"3\. Public ([\d.]+) Administra", 1),
    ("stilt.h", "61", " Note (4)", r"Covered parking with a clear height of ([\d.]+) meters may be provided within the permissible ground coverage on plots measuring \d?\[?(\d+) sq", 1),
    ("stilt.plot", "61", " Note (4)", r"Covered parking with a clear height of ([\d.]+) meters may be provided within the permissible ground coverage on plots measuring \d?\[?(\d+) sq", 2),
    // Rules 67, 68 — plantation, plinth
    ("tree.per", "67", "", r"one tree for every (\d+) square meters of individual plot", 1),
    ("plinth.min_cm", "68", "(1)", r"height of the plinth shall be more than (\d+) centimeters", 1),
    // Rule 75 — private garage
    ("garage.w", "75", "(a)", r"Private Garage-([\d.]+) meters X ([\d.]+) meters", 1),
    ("garage.l", "75", "(a)", r"Private Garage-([\d.]+) meters X ([\d.]+) meters", 2),
    // Rule 76 — basement
    ("base.h", "76", "(2)(a)", r"clear height of at least ([\d.]+) meters as measured from the floor to the underside of the beam", 1),
    ("base.h_com", "76", "(2)(a)", r"minimum clear height as calculated above shall be ([\d.]+) meters", 1),
    ("base.plinth", "76", "(2)(b)", r"basement is built shall not be more than ([\d.]+) meters above the surrounding ground", 1),
    // Rule 77 — parapet
    ("parapet.h", "77", "", r"shall not be less than ([\d.]+) meters in height from the finished floor level", 1),
    // Rule 78 — boundary wall
    ("wall.front", "78", "(a)", r"maximum height of the compound wall shall be ([\d.]+) meters above the center line of the front street", 1),
    ("wall.service", "78", "(b)", r"shall not have a height more than ([\d.]+) meters above the central line of the service road in case it exists otherwise it shall be not more than ([\d.]+) meters", 1),
    ("wall.other", "78", "(b)", r"shall not have a height more than ([\d.]+) meters above the central line of the service road in case it exists otherwise it shall be not more than ([\d.]+) meters", 2),
    // Rule 79 — septic tank
    ("septic.gap", "79", "(1)", r"but not within (\d+) meters thereof", 1),
    ("septic.w_cm", "79", "(2)(a)", r"minimum width of (\d+) centimeters and minimum depth of (one) meter", 1),
    ("septic.depth", "79", "(2)(a)", r"minimum width of (\d+) centimeters and minimum depth of (one) meter", 2),
    ("septic.cap", "79", "(2)(a)", r"minimum liquid capacity of (one) cubic meter", 1),
    ("septic.len_min", "79", "(2)(a)", r"Length of tanks shall be (\d) to (\d) times the width", 1),
    // Rule 80 — staircase
    ("stair.w.res", "80", "(2)(i)", r"Residential buildings \(Dwellings\) - ([\d.]+) meters", 1),
    ("stair.w.hotel", "80", "(2)(ii)", r"hotel buildings - ([\d.]+) meters", 1),
    ("stair.w.asm", "80", "(2)(iii)", r"theatres and cinemas - ([\d.]+) meters", 1),
    ("stair.w.edu_low", "80", "(2)(iv)", r"up to 24 m\. in height - ([\d.]+) meters \(b\) more than 24 m\. in height - ([\d.]+) meters", 1),
    ("stair.w.edu_high", "80", "(2)(iv)", r"up to 24 m\. in height - ([\d.]+) meters \(b\) more than 24 m\. in height - ([\d.]+) meters", 2),
    ("stair.w.inst", "80", "(2)(v)", r"up to 10 beds - ([\d.]+) meters \(b\) more than 10 beds - ([\d.]+) meters", 1),
    ("stair.w.other", "80", "(2)(vi)", r"All other buildings - ([\d.]+) meters", 1),
    ("stair.landing_cm", "80", "(3)", r"twice the tread plus (\d+) centimeters", 1),
    ("stair.tread.res_cm", "80", "(4)", r"without nosing shall be (\d+) centimeters for residential", 1),
    ("stair.tread.other_cm", "80", "(4)", r"tread for other buildings shall be (\d+) centimeter", 1),
    ("stair.riser.res_cm", "80", "(5)", r"risers shall be (\d+) centimeters for residential buildings and (\d+) centimeters for other buildings", 1),
    ("stair.riser.other_cm", "80", "(5)", r"risers shall be (\d+) centimeters for residential buildings and (\d+) centimeters for other buildings", 2),
    ("stair.per_flight", "80", "(5)", r"limited to (\d+) numbers per flight", 1),
    ("stair.headroom", "80", "(6)", r"minimum clear head-room in any staircase shall be ([\d.]+) meters", 1),
    // Rule 81 — rainwater harvesting
    ("rwh.plot", "81", "", r"plot size more than (\d+) Square meter", 1),
    // Rule 83 — lifts
    ("lift.above", "83", "", r"lifts shall be made for building more than ([\d.]+) meters in height", 1),
];

/// Rule text with no number to read, quoted as the source of a check (value is 0).
const QUOTES: &[(&str, &str, &str, &str)] = &[
    ("ref.level", "2", "(34)", r#""ground level" means ground or reference level which would be at the centre point of frontage of the plot or land from which the access is being taken\. If the site is sloping, the reference level will be taken at the centre line of the existing or proposed approach road or street in front of the plot or land from which the access is being taken or the average level of the plot, whichever is higher"#),
    ("height.def", "2", "(9)", r#""building height or height of a building" means the vertical length of the building which shall be measured from a ground or reference level"#),
    ("below.ref.far", "76", "(6)", r"If the built form below the ground or reference level is used as habitable accommodation because of the existing topography such area may be permitted as habitable area and shall be counted in the Floor Area Ratio"),
    ("plinth.drainage", "68", "(1)", r"The plinth or any part of a building or out house shall be so located with respect to surrounding ground level that adequate drainage of the site is assured"),
    ("wall.measure", "78", "(b)", r"It shall be measured above the central line of the front street"),
];

/// Rule 42 Table 4 / Table 5 row, read from the text.
#[derive(Clone, Debug, Serialize)]
pub struct HeightRow {
    pub table: u8,
    pub row: u8,
    pub road_min: f64,
    pub plot_min: f64,
    pub frontage_min: f64,
    pub far: f64,
    pub coverage_pct: Option<f64>,
    /// None on the last Table 5 row ("Above 90 meters").
    pub height_cap: Option<f64>,
    pub front: f64,
    pub other: f64,
    pub page: u32,
    pub quote: String,
}

pub struct Facts {
    map: HashMap<&'static str, Fact>,
    pub missing: Vec<&'static str>,
    pub rule42: Vec<HeightRow>,
    /// Rule 56(6)(b) rows: (height up to — None for "above 30", area, min side).
    shaft_rows: Vec<(Option<f64>, f64, f64)>,
    shaft_fact: Option<Fact>,
}

impl Facts {
    pub fn load(corpus: &Corpus) -> Facts {
        let texts: HashMap<&str, String> = corpus
            .rules
            .iter()
            .map(|r| (r.rule.as_str(), squash(&r.text)))
            .collect();
        let starts: HashMap<&str, u32> =
            corpus.rules.iter().map(|r| (r.rule.as_str(), r.page_start)).collect();

        let mut map = HashMap::new();
        let mut missing = Vec::new();
        let mut compiled: HashMap<&str, Regex> = HashMap::new();
        for &(id, rule, clause, pat, group) in DEFS {
            let re = compiled
                .entry(pat)
                .or_insert_with(|| Regex::new(&format!("(?i){pat}")).expect("fact pattern"));
            let hit = texts.get(rule).and_then(|t| {
                let c = re.captures(t)?;
                let value = word_num(c.get(group)?.as_str())?;
                let m = c.get(0)?;
                Some(Fact {
                    value,
                    rule,
                    clause,
                    page: page_at(t, m.start(), starts[rule]),
                    quote: m.as_str().to_string(),
                })
            });
            match hit {
                Some(f) => {
                    map.insert(id, f);
                }
                None => missing.push(id),
            }
        }

        for &(id, rule, clause, pat) in QUOTES {
            let hit = texts.get(rule).and_then(|t| {
                let m = Regex::new(&format!("(?i){pat}")).ok()?.find(t)?;
                Some(Fact { value: 0.0, rule, clause, page: page_at(t, m.start(), starts[rule]), quote: m.as_str().to_string() })
            });
            match hit {
                Some(f) => {
                    map.insert(id, f);
                }
                None => missing.push(id),
            }
        }

        let mut rule42 = Vec::new();
        if let Some(t) = texts.get("42") {
            for n in 1..=3 {
                if let Some(r) = table_row(t, T4, 4, n, starts["42"]) {
                    rule42.push(r);
                } else {
                    missing.push("rule42.table4");
                }
            }
            for n in 1..=5 {
                if let Some(r) = table_row(t, T5, 5, n, starts["42"]) {
                    rule42.push(r);
                } else {
                    missing.push("rule42.table5");
                }
            }
        }
        let (mut shaft_rows, mut shaft_fact) = (Vec::new(), None);
        if let Some(c) = texts.get("56").and_then(|t| Regex::new(SHAFT).unwrap().captures(t).map(|c| (t, c))) {
            let (t, c) = c;
            let n: Vec<f64> = c.iter().skip(1).filter_map(|g| g?.as_str().parse().ok()).collect();
            for row in n[..15].chunks(3) {
                shaft_rows.push((Some(row[0]), row[1], row[2]));
            }
            shaft_rows.push((None, n[15], n[16]));
            let m = c.get(0).unwrap();
            shaft_fact = Some(Fact {
                value: 0.0,
                rule: "56",
                clause: "(6)(b)",
                page: page_at(t, m.start(), starts["56"]),
                quote: format!("Height of building / area of shaft / minimum side: {}", m.as_str()),
            });
        } else {
            missing.push("shaft.table");
        }
        Facts { map, missing, rule42, shaft_rows, shaft_fact }
    }

    pub fn get(&self, id: &str) -> Option<&Fact> {
        self.map.get(id)
    }

    pub fn v(&self, id: &str) -> Option<f64> {
        self.map.get(id).map(|f| f.value)
    }

    /// Rule 56(6)(b): ventilation shaft (area, min side) for a building height — the
    /// first row at or above it. Between rows we round up to the next row (safe side).
    pub fn shaft(&self, height: f64) -> Option<(f64, f64, &Fact)> {
        let f = self.shaft_fact.as_ref()?;
        self.shaft_rows
            .iter()
            .find(|(h, _, _)| h.map(|h| height <= h).unwrap_or(true))
            .map(|&(_, a, s)| (a, s, f))
    }
}

fn table_row(t: &str, pat: &str, table: u8, n: u8, start: u32) -> Option<HeightRow> {
    let re = Regex::new(&format!("(?i){}", pat.replace("{n}", &n.to_string()))).ok()?;
    let c = re.captures(t)?;
    let f = |i: usize| c.get(i).and_then(|g| g.as_str().parse::<f64>().ok());
    let m = c.get(0)?;
    let (coverage_pct, cap, front, other) = if table == 4 {
        (None, f(5), f(6)?, f(7)?)
    } else {
        (f(5), f(6), f(7)?, f(8)?)
    };
    Some(HeightRow {
        table,
        row: n,
        road_min: f(1)?,
        plot_min: f(2)?,
        frontage_min: f(3)?,
        far: f(4)?,
        coverage_pct,
        height_cap: cap,
        front,
        other,
        page: page_at(t, m.start(), start),
        quote: m.as_str().to_string(),
    })
}

/// Page of a position: the last running header ("… Bhumi Vikas Rules, 2012 78") before it.
fn page_at(text: &str, pos: usize, start: u32) -> u32 {
    let re = Regex::new(r"Bhumi Vikas Rules, 2012 (\d+)").unwrap();
    re.captures_iter(&text[..pos])
        .filter_map(|c| c[1].parse().ok())
        .last()
        .unwrap_or(start)
}

fn word_num(s: &str) -> Option<f64> {
    match s.to_lowercase().as_str() {
        "one" => Some(1.0),
        "one-fifth" => Some(0.2),
        n => n.parse().ok(),
    }
}

pub fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
