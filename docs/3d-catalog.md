# 3D component catalog — M.P. Bhumi Vikas Rules, 2012

Built by reading all 106 rules (1–105 and 26-B) in
`track-1-bhumi-vikas-sahayak/data/bhumi_vikas_rules.jsonl`. It lists every physical thing
the rules give a size, distance, height or position for, and whether the 3D view draws it.

- **Code:** `src/catalog.rs` (the list and trigger words), `src/facts.rs` (every number, read
  from the rule text by pattern), `src/scene.rs` (composer), `static/components3d.js` (one
  drawing function per component).
- **Rule:** the browser never invents a rule number. Each value below is a regex capture
  over the rule text, with the PDF page and the exact quote shown in the UI.
  `cargo test every_fact_reads_from_the_rule_text` fails if a pattern stops matching.
- **Drawn, not ruled:** where a rule gives no size (lift shaft, window spacing, gate width
  for homes), the component lists that value under `illustrative`, and the inspector shows it
  as "Drawn, not ruled".

Page numbers are PDF pages.

## How a question becomes a scene

1. **Numbers** come from code (`Numbers::parse`): plot area, frontage, street width, height,
   floors ("3 floors", "G+2"). Each number is classified by the nearest context word.
2. **Components and use type** come from trigger words in `catalog.rs` (EN + HI). When the
   Cerebras key is set, the model may add ids **from the catalog only**. It never supplies a
   number.
3. **Open spaces** come from the rule that governs the case:

   | Case | Rule |
   |---|---|
   | Residential, height ≤ 12.5 m | 56(1)–(3): front by street width, rear by plot area, side by detached / semi-detached / row, with both provisos |
   | Any use, height > 12.5 m | 42 Table 4 (≤ 30 m) or Table 5 (high rise), row picked by height; road, plot and frontage minimums are checked |
   | Educational / institutional / assembly / business / industrial | 57(a)–(e); above 12.5 m the larger of 57 and 42 on each side |
   | Farm house | 17: 7.5 m all round, height 7.5 m sloping roof, plot 0.40 ha, road 7.5 m |

4. **FAR** is checked against Rule 61 Table 8 (or the Rule 42 row, or 17(b)). The footprint is
   capped by Table 5 ground coverage (30%) for high rises.
5. Anything required here but not switched on is suggested: lift above 12.5 m (83), trees
   (67), rainwater harvesting over 140 sq m (81), parapet (77).

Unstated inputs get defaults that are **listed as assumed**: 200 sq m plot, 1 : 1.5 deep plot,
9 m street, 9 m height, detached, residential, 3 m per floor.

## Drawn components

| id | What | Rules (page) | Values read from the text | Inputs |
|---|---|---|---|---|
| plot, road, building | Plot slab, street, floors with slabs | 2(9), 2(68), 2(72) | floors = height ÷ 3 m (assumed) | area, frontage, street, height |
| open spaces | Front / rear / side strips | 56 (77–79), 42 (58–59), 57 (80), 17 (34) | see tables below | street, area, height, use, type |
| plinth | Raised base | 68(1) (87), 76(2)(b) (90) | more than 45 cm; with a basement at most 0.45 m | basement on/off |
| balcony | First floor and up, front face | 58(1)(c) (81), 2(30) | up to 1.20 m, at least 1.5 m from the plot boundary, not continuous; up to 1.20 m is FAR-free | front open space, floors |
| chhajja | Sunshade over windows | 58(1)(a)–(b) (81) | cornice / weather shade 0.75 m; sunshade 0.75 m | — |
| parapet | Roof-edge wall | 77 (91) | at least 1.05 m | — |
| boundary_wall | Compound wall + gate | 78 (92), 41(c) (58), 42(6) (60) | front ≤ 1.5 m; sides / rear ≤ 2 m (3 m with a service road); gate ≥ 4.5 m for non-residential or > 12.5 m | use, height |
| staircase | Dog-leg stair core + mumty | 80 (93–94) | width 0.85 m residential; 1.5 m hotel / assembly / educational ≤ 24 m / institutional ≤ 10 beds / other; 2.0 m educational > 24 m or institutional > 10 beds; tread 25 cm residential, 30 cm other; riser ≤ 17 cm residential, 15 cm other; ≤ 15 risers per flight; landing ≥ 2 × tread + 25 cm; headroom 2.2 m | use, height, floor height |
| lift | Shaft, cab, machine room | 83 (95) | required above 12.5 m; car size in NBC Part VIII, so the shaft is illustrative | height |
| ventilation_shaft | Duct through all floors | 56(6)(b) (79) | height ≤ 10: 1.2 sq m / 0.9 m · 12.5: 2.8 / 1.2 · 18: 4.0 / 1.5 · 24: 5.4 / 1.8 · 30: 8.0 / 2.4 · above 30: 9.0 / 3.0 (+ mechanical) | height |
| courtyard | Inner court open to sky | 56(6)(a) (79) | side ≥ 3 m; area ≥ (height ÷ 5)² | height, footprint |
| stilt_parking | Open ground floor with columns and cars | 61 Note (4) (84), 2(54) | clear height 2.4 m, plot ≥ 200 sq m, not counted in FAR or height | plot area |
| basement | Below-ground box + ramp | 76 (90–91) | clear height ≥ 2.4 m (3.6 m commercial); within the open-space lines; plinth ≤ 0.45 m | use |
| garage | Private garage | 75(a) (90), 56(4) (78), 2(20) | 2.5 m × 5.0 m, at the rear end of a side open space; ≤ 25 sq m excluded from coverage | side open space |
| trees | Plantation | 67 (87), 17(d) (34) | one per 100 sq m of plot; farm house 100 per hectare | plot area, use |
| septic_tank | Tank below ground | 79 (92–93) | width ≥ 75 cm, depth ≥ 1 m, capacity ≥ 1 cu m, length 2–4 × width, ≥ 2 m from the building | open spaces |
| rainwater_harvesting | Pit marker + downpipe | 81 (94) | required on plots over 140 sq m; designs in Appendix H (not in data) | plot area |
| electric_line | Poles, wires, clearance envelope | 52 (69) | low / medium voltage: 2.5 m vertical, 1.2 m horizontal · HV ≤ 33 kV: 3.7 m / 1.2 m · EHV > 33 kV: 15 m corridor (7.5 m each side). Checked against balcony and chhajja projections | voltage class ("11 kV", "HT line"…) |
| water_body | Lake / river + no-build band | 50(b) (68) | 9 m from the highest water level of a water body; 15 m for a river | lake or river |

### Open-space tables used

**Rule 56(1)(a) front (residential ≤ 12.5 m):** street ≤ 9 m → 3.0 m · 9–12 m → 3.6 m ·
12–18 m → 4.5 m · > 18 m → 6.0 m. **56(1)(b):** street under 6 m in developed areas → building
line 6 m from the centre line (shown as a note).

**Rule 56(2)(a) rear:** plot ≤ 40 sq m → nil · 40–150 → 1.50 m · 150–225 → 2.50 m · > 225 → 3.00 m.

**Rule 56(3) side:** detached 3 m both sides (one side 1.5 m if ≤ 7 m tall and frontage < 12 m) ·
semi-detached 3.0 m one side (2.5 m if ≤ 10 m tall and frontage ≤ 10 m) · row none.

**Rule 42 Table 4 (12.5–30 m):**

| Row | Road | Min plot | Frontage | FAR | Height | Front | Sides / rear |
|---|---|---|---|---|---|---|---|
| 1 | 12 m+ | 1000 sq m | 18 m | 1.50 | ≤ 18 m | 7.5 m | 6.0 m |
| 2 | 18 m+ | 1500 sq m | 21 m | 1.75 | ≤ 24 m | 9.0 m | 6.0 m |
| 3 | 24 m+ | 2000 sq m | 30 m | 2.0 | ≤ 30 m | 12.0 m | 7.5 m |

The ground-coverage column and Note 1 were deleted on 06-03-2025.

**Rule 42 Table 5 (high rise):**

| Row | Road | Min plot | Frontage | FAR | Coverage | Height | Front | Sides / rear |
|---|---|---|---|---|---|---|---|---|
| 1 | 30 m+ | 2500 | 30 m | 2.25 | 30% | ≤ 45 m | 15 m | 7.5 m |
| 2 | 36 m+ | 3000 | 40 m | 2.50 | 30% | ≤ 60 m | 18 m | 9 m |
| 3 | 45 m+ | 3500 | 45 m | 2.75 | 30% | ≤ 75 m | 21 m | 9 m |
| 4 | 60 m+ | 4000 | 50 m | 3.0 | 30% | ≤ 90 m | 24 m | 10 m |
| 5 | 75 m+ | 4500 | 60 m | 3.0 | 30% | > 90 m | 30 m | 12 m |

**Rule 57:** educational (not nursery) 6 m all round · institutional 6 m · assembly 12 m front,
6 m others · business / mercantile / storage 6 m front, 4.5 m others · industrial and hazardous
4.5 m up to 16 m, + 0.25 m per metre (or part) above 16 m.

**Rule 61 Table 8 FAR:** residential 1.25 · commercial 2.50 / 2.00 / 1.75 / 1.50 / 1.25 (city
centre → convenience shopping; the lowest is used and noted) · public / semi-public 1.00 ·
industrial per Rule 48.

## Levels (elevation)

Both the Ask scene (plot above or below the road) and Draw plot (road level plus a level for
every plot corner, so a site can slope) use these rules:

| Rule (page) | What it says | What the tool does |
|---|---|---|
| 2(9), 2(34) (4, 8) | Height is measured from the ground or reference level: the centre of the frontage; on a **sloping** site, the road centre line or the average plot level, **whichever is higher** | Draw plot fits a ground plane through the corner levels, picks the reference level, and measures each building from it (plinth included, stilt excluded). This rule height chooses Rule 56 or 42 and decides whether a lift is needed |
| 68(1) (87) | Plinth more than 45 cm above the surrounding ground, placed so the site drains | Plinth ≤ 0.45 m is an error. A floor below the road centre line gets a drainage note |
| 76(2)(b), 76(6) (90–91) | With a basement, plinth at most 0.45 m; habitable space below the reference level counts in FAR | Basement plinth > 0.45 m is an error. A floor below the reference level gets a note |
| 78 (92) | Walls measured **above the centre line of the front street**: front ≤ 1.5 m, others ≤ 2 m | Wall height above the plot = limit − (ground − road level), edge by edge. A raised plot gets a lower wall |
| 16(4)(j) (28) | Sites steeper than 1:20 need contours every 0.3 m on the site plan | The slope of the fitted plane is reported, and contours are drawn on the plan |

Plan, 3D and section views show the road level, the ground, the plinth, the floors, the reference
line and the height that counts for the rules. A plot whose corners are within 0.05 m of each
other is treated as level (stated in `src/layout.rs`).

## Catalogued, not drawn

Each of these is in `catalog.rs`. When a question mentions one, the 3D panel lists it with
the reason it isn't drawn.

| id | Rules | Key values | Why not drawn |
|---|---|---|---|
| podium | 62, 2(54) | plot ≥ 1000 sq m; ≤ 2.4 m clear per floor; ≤ 3 floors; not in FAR or height | multi-building layout |
| parking_floor | 63 | plot ≥ 1000 sq m; ≤ 2.4 m clear | as above |
| service_floor | 64, 2(63) | plot ≥ 1000 sq m; ≤ 2 m clear | as above |
| mezzanine | 73, 2(43) | ≥ 2.2 m high; ≤ ⅓ of the room's plinth area; ≥ 9.5 sq m if a living room | interiors |
| loft / ledge | 72, 2(39) | headroom ≥ 1.5 m; ≤ 25% of the room; kitchen loft leaves ≥ 2.2 m below | interiors |
| habitable_room | 69 | ≥ 2.6 m high (2.4 m under beams); 9.5 sq m, width 2.4 m (second room 7.5 sq m, 2.1 m); educational 3.6 m; industrial 3.6 m / 3 m AC | interiors |
| kitchen | 70 | ≥ 2.6 m; 5.0 sq m and 1.8 m wide with separate dining (4.5 with store); 7.5 sq m and 2.1 m if also dining | interiors |
| bathroom_wc | 71 | ≥ 2.2 m; WC 1.1 sq m and 0.9 m wide; bath 1.5 sq m; combined 2.4 sq m and 1.2 m (71(2)(b) gives different sizes) | interiors |
| store_room | 74 | ≥ 2.2 m; residential ≥ 3 sq m | interiors |
| windows | 82 | openings ≥ 1/10 of floor area (+25% for kitchen); nothing lit beyond 7.5 m from an opening | interiors |
| projecting_room | 58(3) | alternate floors, not into the open space, no depth limit | no size given |
| skyway | 66(1) | ≥ 6.5 m clear above ground; ≤ 4 m wide | needs two buildings |
| subway | 66(2) | pedestrian: ≥ 3.0 m clear height, ≥ 4.5 m wide | below-street |
| access_road | 38, 39, 41(a) | public street ≥ 7.5 m · by access length 7.5 m / 150 m, 9 / 250, 12 / 400, 18 / 1000, 24 / > 1000 · crowd-attracting buildings 12 / 200, 15 / 400, 18 / 600, 24 / > 600 · pathway ≥ 1.5 m up to 30 m · non-residential street ≥ 12 m | layout scale |
| cul_de_sac | 43 | turning space ≥ 121 sq m, no side under 11 m | layout scale |
| alley | 44 | ≥ 3 m | layout scale |
| community_open_space | 47 | 10% of the layout; length ≤ 2.5 × average width; pavilion ≤ 25 sq m (75 sq m if > 2000 sq m) | layout scale |
| group_housing | 60 Table 7 | density 125 / 250 / 425 / 500 / 625 pph → coverage 25 / 30 / 33 / 35 / 35%, FAR 0.75 / 1.25 / 1.50 / 1.75 / 2.00 | special type |
| cinema | 53(3)(ii), 94 | plot ≥ 2000 sq m or 4 sq m per seat; coverage 33%; FAR 1.25; front 15 m, sides and rear 6 m; road ≥ 18 m; 9 m from street line | special type |
| multiplex | 95 | coverage 40%; front 12 m; plot 1200–2500 sq m or 4–5 sq m per seat; road 18 / 24 / 30 m by population; hall ≥ 8 m clear | special type |
| shopping_mall | 96 | FAR 1.50 / 1.75 / 2.00 for roads of 18 / 24 / 30 m; coverage 40%; mall front 9 m, others 6 m; open mall 12 m / 7.5 m; height 18 / 24 / 30 m by population | special type |
| marriage_garden | 53(3)(iii) | plot 4000 / 10000 sq m; road 12 / 18 m; frontage 25 / 40 m; FAR 0.10; coverage 10%; front parking 30 / 40%, then 6 m open space | special type |
| fuel_station | 53(3)(iv) | filling station only 20 × 20 m; with service 35 × 35 m; road 15 / 18 / 24 m by population; built-up ≤ 15% | special type |
| ev_charging | 53(3)(C) | public charging 13.5 × 5.5 m; fast charging 15 × 7 m; battery swap 5.5 × 2.75 m; CNG 36 × 30 m; right of way ≥ 30 m | special type |
| industrial_plot | 48 | 10-row table by plot area (front 3.2–24 m, sides 2.1–12 m, coverage 60%); taller buildings: front = height ÷ 2, sides = height ÷ 3 | the 3D view uses 57(e) |
| servant_quarter | 61 Note (3), 58(4) | ≤ 25 sq m; one on 200–500 sq m, two above; watchman hut / pump room ≤ 6 sq m; sanitary block 2.4 m high, ≤ 4 sq m | position is the owner's choice |
| railway | 50(k) | distance set by the Railway | no number in the rules |

Other values recorded but not drawn:
- **24(2):** an extra 0.15 m per floor of height is not counted.
- **2(9):** parapets and features up to 1.5 m, and rooftop structures up to ⅓ of the roof area, are excluded from height.
- **2(38):** high rise means more than 10 storeys or more than 30 m.
- **55(4):** accessory buildings at least 1.5 m from the main building if it is over 7 m tall.
- **56(6)(c):** outer courtyards at least 2.4 m wide.
- **58(5):** in congested areas, sunshades over a road must be at least 2.8 m up and at most 0.6 m deep, only on roads over 9 m.
- **65:** a corner plot takes the height of its wider street for 24 m along the narrower one.
- **78(b):** corner-plot side wall at most 1.5 m.

## Rules with no 3D content

These rules are procedural, give no spatial values, or defer to other documents:

- **Part I:** 1, 3–5.
- **Organisation, permission and inspection:** 6–16, 18–21, 23, 25–35.
- **Land use and development control:** 36–37 (land-use codes only), 39–40, 45–46 (no fixed numbers; 46 restates a 3 m building line from the means of access), 49 (community-facility land areas).
- **Plot norms:** 51, 53(1)–(2) (plot size and frontage by building type, used as a check), 54–55(1)–(3), 59.
- **Services, fire and structure:** 84–93 (parking norms are in Appendix 1; fire and services follow NBC).
- **Special buildings and certificates:** 94(1), 97–105.

The full per-rule notes from the read-through are summarised in this file; the values above
are the ones the code reads.
