# Bhopal Sahayak

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
2. `cp .env.example .env` and set `CEREBRASAPI=<your key>` (optional — see below).
3. `cargo run`
4. Open <http://127.0.0.1:8080> and pick a track.
5. `cargo test` runs the sample questions; `cargo test report_all_samples -- --ignored --nocapture` prints every result.

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
src/track1.rs    rules: retrieval + deterministic table parsers
src/track2.rs    ghats: CPCB classification + ranking
src/corpus.rs    BM25 over the rule texts
src/models.rs    Answer / Source / Evidence types
src/main.rs      axum routes + JSON API
static/          hand-written HTML/CSS/JS UI
tests/           sample-question assertions
```

## License

MIT — see [`LICENSE`](LICENSE).
