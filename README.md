# Bhumi Rule checker

*(formerly Rules Checker / Bhopal Sahayak)* — ask about the M.P. Bhumi Vikas Rules, 2012 and see the answer in 3D,
or draw your own plot and check it against the rules.

One lookup tool for two Bhopal decisions: **building-permission questions** (Track 1) and
**pre-visarjan ghat water risk** (Track 2). Built at Bhopal Impact Lab 2026 with Claude Code and
Cerebras inference. Every answer shows its source page; when the source is silent, the tool says so.

> **Fill in before submitting:** team name, members, and the track you are entering.

| | |
|---|---|
| **Team** | _<your team name>_ |
| **Members** | _<name · role>, <name · role>, …_ |
| **Track** | Both — Track 1 (Bhumi Vikas Sahayak) and Track 2 (Visarjan Ghat Watch) on a shared core |
| **User we built for** | The **BMC building-permission clerk** (Track 1) and the **BMC officer planning Durga visarjan on 20 Oct 2026** (Track 2) |

## The one workflow

- **Track 1:** a clerk types a question in English or Hindi and gets a short answer **with the rule
  number and the exact quoted text**, or a clear “not found”.
- **Track 2:** the officer gets the seven Bhopal ghats **ranked by risk, each with a plain-word
  reason, page trace, and a short EN/HI brief** to forward.

## How to run (5 steps)

1. Install Rust (stable) — `cargo --version` should work.
2. `cp .env.example .env` and set `CEREBRASAPI=<your key>` (optional — see below). Add
   `SARVAMAPI=<your key>` to turn on the mic button on Building rules (Sarvam speech-to-text, EN/HI).
3. `cargo run`
4. Open <http://127.0.0.1:8080> (tabs: Building rules, Draw plot, Document). Use another port with
   `cargo run -- --port 8081`.
5. `cargo test` runs the sample questions and the 25 example scenarios;
   `cargo test report_all_samples -- --ignored --nocapture` prints every sample result and
   `cargo test --test examples dump -- --ignored --nocapture` prints every example scene.

No key? The tool still runs entirely on its deterministic core (retrieval + table parsing + CPCB
comparison). The key only powers the language steps: understanding the question and writing the
answer / Hindi brief.

## How it works (the grounding rule)

> The model never produces a number. Numbers come from **code** — table parsing, arithmetic, CPCB
> thresholds. The model does only the two things code can't: **understand** the question and
> **speak** the answer.

```
parse  (Cerebras)      question (EN/HI) -> English search phrase + intent   [never a number]
solve  (Rust)          retrieval + table parse + thresholds + ranking        [all numbers here]
render (Cerebras)      facts + sources -> short prose, in EN or HI           [exact text kept verbatim]
```

Cerebras models: `gpt-oss-120b` (parse), `qwen-3.8-27b` (Hindi). Exact source text is never
translated — it is always shown verbatim in the citation block.

## What works

- **Track 1, deterministic:** front open space (Rule 56(1)(a)), rear open space (Rule 56(2)(a)),
  duration of sanction (Rule 23), deemed sanction (Rule 28), fee schedule (Rule 21), and retrieval
  + exact quote for the rest. Out-of-scope questions abstain.
- **Track 1, language:** Hindi questions are normalised to English for retrieval, and answers are
  rendered back in Hindi (numbers preserved).
- **Track 2:** all 1,524 readings loaded; ghats classified pass / fail / missing vs CPCB Class B;
  ranked with the worst dissolved-oxygen reading as the headline; `not sampled` is shown as missing,
  never as clean; EN/HI brief generated.

- **Track 2 in the web UI:** the Ghat water risk tab was removed. The ranking still runs on the
  server (`src/track2.rs`, `GET /api/ghats`) and is covered by the tests.
- **Track 1, 3D plot (Three.js, vendored in `static/vendor/`):** every plot-shaped question builds
  a 3D scene composed from a **catalog of components read out of all 106 rules**
  ([`docs/3d-catalog.md`](docs/3d-catalog.md)). Open spaces come from the rule that governs the
  case: Rule 56 for homes up to 12.5 m, Rule 42 Tables 4 and 5 above that, Rule 57 for other uses,
  Rule 17 for farm houses. FAR is checked against Rule 61, 42 or 17. On top of that, 18 components
  can be drawn: balcony, chhajja, parapet, staircase (real treads and risers), lift, ventilation
  shaft, courtyard, stilt parking, basement, boundary wall and gate, garage, trees, septic tank,
  rainwater harvesting, overhead line clearance and lake/river buffer. A free-form question
  ("G+2 house on 180 sq m, 12 m road, balcony, garage, 11 kV line in front") switches on what it
  names, and a palette lets the user add or remove any of them. Every number is read from the rule
  text by pattern (`src/facts.rs`, checked by tests). Where a rule sets no size, the inspector says
  "drawn, not ruled". Components that break a rule are drawn in red with the reason. The model, if
  present, only picks catalog ids; it never supplies a dimension.
- **Draw plot (new tab):** draw the plot as it really is (any outline, road on any side, corner
  plots with two roads), then place and move blocks: buildings with floors and stilt parking,
  balcony, garage, staircase, lift, shaft, courtyard, septic tank, parking bay, trees, rainwater
  pit, gate, boundary wall, lake/river and power line. Each change is checked live (or on
  demand with *Check now*) by `POST /api/check` (`src/layout.rs`):
  - Every plot edge is classed front (road), rear or side, and its required open-space line is
    drawn, with the buildable area shaded.
  - Each block is measured against the edges and against the other blocks: balcony depth and
    boundary gap, garage size and position, septic distance, line clearance, lake buffer, stair
    width, shaft and courtyard sizes.
  - FAR, ground coverage, lift above 12.5 m, trees, rainwater harvesting, wall heights and the
    gate width are checked.
  - Every problem names the block (drawn red in 2D and 3D) and quotes the rule.

  **Levels:** set the road level and the ground level at each plot corner (a sloping site). The
  checker finds the reference level from Rule 2(34): the frontage centre, or on a slope the higher
  of the road centre line and the average plot level. Each building is measured from there,
  plinth included, which can move it into Rule 42 or require a lift. The plinth is checked
  (68(1), 76(2)(b)), walls are measured above the street centre line (78), and contours are
  required above 1:20 (16(4)(j)). A section view draws the road, the ground, the plinth, the
  floors and the height that counts. Ask questions understand "plot is 1.5 m above the road".
  **Custom components:** add a block and name it anything. If the name matches something
  the rules regulate (for example "car shed" → garage, "watchman cabin" → watchman hut, "water
  tank" → rooftop structure, "sunshade", "porch", "ramp", "servant quarter", "pump room"), the
  tool says what it read the block as and applies that rule. A *Check in rules* switch turns
  this off per block. A name no rule covers is drawn but not checked, and the closest rule text
  is cited.
  **Heights and slopes:** +/− steppers (and the + / − / [ / ] keys) for floors, floor height
  (Rule 69 room height), roof slope (the ridge counts in height), parapet (Rule 77; above 1.5 m
  it counts in height under 2(9)), which floor a balcony is on (58(1)(c): first floor and up),
  garage height (75(1)), septic depth (79), and gate, tree and custom heights.
  Roads are selectable and deletable like any other block.
  Undo/redo, snap, import/export JSON and autosave are included. **Edit in Draw plot** on any
  3D answer opens that scene in the editor.
- **Document viewer (Document tab):** the full rulebook text, page by page, with a rule index,
  go-to-page, previous/next, find in document, and a link to the original PDF at the same page
  (`GET /api/doc`, `/doc/rules-en.pdf`). Every rule citation in the app (answers, 3D inspector,
  Draw plot issues and open-space list) has an **Open Document** button. It jumps to the cited
  page and highlights the quoted line, matched whitespace-insensitively, or passage by passage
  for quotes stitched from several sentences. *Back* returns you to where you were.
- **From an answer to a drawing:** every 3D answer has a *Want to customize? Open in Draw plot*
  button that opens the same plot, building and components in Draw plot. *Reset* next to Ask
  clears the question and answer.
- **Track 1, example gallery:** 25 complex scenarios in [`static/examples.json`](static/examples.json),
  grouped as Homes, Tall buildings, Other uses, Surroundings and Hindi. Examples include a row
  house that wants stilt parking on a plot too small for it, a 24 m building on a road too narrow
  for its height, a 45 m tower capped at 30% ground coverage, a factory where Rule 42 outweighs
  Rule 57(e), a balcony near an 11 kV line, and a house too close to a lake. The UI shows them as
  clickable cards. `tests/examples.rs` runs each one and checks its open spaces, rule table,
  components and rule breaches against values checked by hand against the rule tables.

Evidence it is right: **Bairagarh (Sant Hirdaram) kund** ranks first with dissolved oxygen as low as
**0 mg/l** (limit ≥5) in Durga 2023 (p.60), Ganesh 2023 (p.26) and Ganesh 2025 (p.28) — four anoxic
readings, matching the problem statement.

## What doesn’t (honest gaps)

- **Q11** (“when the Development Plan and these rules disagree”) retrieves a weak rule (Rule 7). We
  point to a rule, not a legal interpretation.
- Answers are **rule pointers, not legal advice**. The clerk decides.
- The **28 Aug 2026 amendment** to Rule 2(5)(c) is not applied; every answer carries a version note.
- Track 2 uses the **Bhopal** file only; the statewide 21,600-row file is unused background.
- Risk tiers use fixed score cut-offs (`>25` High, `>12` Medium). The weighting is documented in
  `src/track2.rs` and is a judgement call, not a rule.
- Hindi rendering quality depends on the model; the citation text stays English on purpose.
- The 3D plot is a rectangle (1 : 1.5 deep unless frontage is given) and floors are 3 m storeys.
  Corner plots, room interiors, podiums and layout-scale rules (roads, cul-de-sacs, community open
  space) are catalogued but not drawn — see `docs/3d-catalog.md`.

## Data and sources

All data is the public Bhopal Impact Lab 2026 participant pack (used unchanged):

- Track 1: `track-1-bhumi-vikas-sahayak/data/` — 106 rules (JSONL) from M.P. Bhumi Vikas Rules 2012,
  official EN/HI PDFs.
- Track 2: `track-2-visarjan-ghat-watch/data/` — 1,524 Bhopal ghat readings 2023–2025, CPCB
  water-quality criteria; MPPCB visarjan reports 2023–2025.

Original pack overview and links: [`PACK-README.md`](PACK-README.md).

## Layout

```
src/llm.rs       Cerebras client (env CEREBRASAPI)
src/stt.rs       Sarvam speech-to-text for the mic button (env SARVAMAPI); Hindi number words -> digits
src/track1.rs    rules: retrieval + deterministic table parsers
src/catalog.rs   3D component catalog (ids, rules, EN/HI trigger words)
src/facts.rs     every 3D number, read from the rule text by pattern
src/scene.rs     composes a question into a 3D scene (open spaces, FAR, components)
src/layout.rs    Draw plot: checks a hand-drawn layout (any plot shape) against the rules
src/track2.rs    ghats: CPCB classification + ranking
src/corpus.rs    BM25 over the rule texts
src/models.rs    Answer / Source / Evidence types
src/main.rs      axum routes + JSON API
static/          hand-written HTML/CSS/JS UI (scene3d.js = 3D view, components3d.js = drawers,
                 draw.js = Draw plot editor)
docs/            3d-catalog.md — every spatial value in the rules and what is drawn
tests/           sample-question assertions, every example in static/examples.json, drawn layouts
```

## License

MIT — see [`LICENSE`](LICENSE).
