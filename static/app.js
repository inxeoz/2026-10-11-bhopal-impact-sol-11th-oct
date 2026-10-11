const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const slug = (t) => "tier-" + t.toLowerCase().replace(/[^a-z]+/g, "-").replace(/-+$/, "");
const num = (v, d = 1) => (v == null ? "—" : Number(v).toFixed(d).replace(/\.0$/, ""));

let state = { track: "bhumi-vikas", lang: "en", ghatsLoaded: false, ghats: null };

/* ---------- track switching ---------- */
function setTrack(track) {
  state.track = track;
  $$(".tab").forEach((t) => t.classList.toggle("is-on", t.dataset.track === track));
  $$(".view").forEach((v) => (v.hidden = v.id !== "view-" + track));
  if (track === "visarjan" && !state.ghatsLoaded) loadGhats();
}
$$(".tab").forEach((t) => t.addEventListener("click", () => setTrack(t.dataset.track)));

$$(".lang-btn").forEach((b) =>
  b.addEventListener("click", () => {
    if (!b.dataset.lang) return;
    state.lang = b.dataset.lang;
    $$(".lang-btn[data-lang]").forEach((x) => x.classList.toggle("is-on", x === b));
  })
);

/* ---------- track 1: ask ---------- */
$("#ask-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const q = $("#q").value.trim();
  if (q) ask(q);
});
$$(".chip").forEach((c) =>
  c.addEventListener("click", () => {
    $("#q").value = c.textContent;
    ask(c.textContent);
  })
);

async function ask(question) {
  const box = $("#answer");
  box.innerHTML = '<p class="spinner">Looking through the rulebook…</p>';
  try {
    const r = await fetch("/api/ask", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ track: state.track, question, lang: state.lang }),
    });
    if (!r.ok) throw new Error(await r.text());
    renderAnswer(await r.json());
    $("#examples").classList.add("is-hidden");
  } catch (err) {
    box.innerHTML = `<div class="card"><div class="card-head"><p class="verdict">Could not reach the tool.</p>
      <p class="detail">${esc(err.message)}</p></div></div>`;
  }
}

function renderAnswer(a) {
  const cites = (a.sources || [])
    .map(
      (s) => `<div class="cite">
        <div class="cite-head"><span class="rule">${esc(s.label)}</span><span class="page">${esc(s.cite)}</span></div>
        <pre class="quote">${esc(s.quote)}</pre></div>`
    )
    .join("");
  const diagram = setbackSVG(a.facts || {});
  $("#answer").innerHTML = `
    <article class="card ${a.abstained ? "abstained" : "answered"}">
      <div class="card-head">
        <div class="tag">${a.abstained ? "Out of scope" : "Answer"}</div>
        <p class="verdict">${esc(a.answer)}</p>
        <p class="detail">${esc(a.detail)}</p>
      </div>
      ${diagram}
      ${a.note ? `<p class="note">${esc(a.note)}</p>` : ""}
      ${cites}
    </article>`;
}

/* ---------- track 1: setback schematic ---------- */
function setbackSVG(f) {
  const hasFront = f.front_open_space_m != null && f.street_width_m != null;
  const hasRear = f.rear_open_space_m != null;
  if (!hasFront && !hasRear) return "";

  const v = hasFront ? f.front_open_space_m : f.rear_open_space_m;
  const gapLabel = hasFront ? "front open space" : "rear open space";
  const street = hasFront ? `${num(f.street_width_m)} m wide street` : "";
  const gapTop = hasFront; // front gap sits between building and street (below), rear gap on top

  // Plan view: street band at the bottom, plot above.
  const W = 560, H = 236;
  const px = 150, pw = 260;
  const streetY = 196, streetH = 34;
  const plotTop = 40, plotBot = streetY;
  const gapH = 40;
  const bldgTop = gapTop ? plotTop : plotTop + gapH;
  const bldgBot = gapTop ? plotBot - gapH : plotBot;

  return `
  <div class="diagram">
    <svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Setback diagram">
      <rect x="${px}" y="${plotTop}" width="${pw}" height="${plotBot - plotTop}" class="d-plot"/>
      <rect x="${px}" y="${bldgTop}" width="${pw}" height="${bldgBot - bldgTop}" class="d-bldg"/>
      <text x="${px + pw / 2}" y="${(bldgTop + bldgBot) / 2 + 4}" class="d-label-c">building</text>
      <rect x="${px}" y="${gapTop ? bldgBot : plotTop}" width="${pw}" height="${gapH}" class="d-gap"/>
      <text x="${px + pw / 2}" y="${(gapTop ? bldgBot : plotTop) + gapH / 2 + 4}" class="d-label-c d-gap-t">${gapLabel} · ${num(v)} m</text>
      <rect x="60" y="${streetY}" width="440" height="${streetH}" class="d-street"/>
      <text x="280" y="${streetY + 22}" class="d-label-c d-street-t">${street || "street"}</text>
      ${dim(px - 34, gapTop ? bldgBot : plotTop, gapTop ? bldgBot + gapH : plotTop + gapH, `${num(v)} m`)}
      <text x="${px + pw + 14}" y="${plotTop + 14}" class="d-note">plot</text>
    </svg>
  </div>`;
}

function dim(x, y1, y2, label) {
  const mid = (y1 + y2) / 2;
  return `
    <line x1="${x}" y1="${y1}" x2="${x}" y2="${y2}" class="d-dim"/>
    <line x1="${x - 5}" y1="${y1}" x2="${x + 5}" y2="${y1}" class="d-dim"/>
    <line x1="${x - 5}" y1="${y2}" x2="${x + 5}" y2="${y2}" class="d-dim"/>
    <text x="${x - 8}" y="${mid + 3}" class="d-dim-t">${label}</text>`;
}

/* ---------- track 2 ---------- */
async function loadGhats() {
  $("#ghats").innerHTML = '<p class="spinner">Ranking ghats…</p>';
  try {
    const data = await (await fetch("/api/ghats")).json();
    state.ghats = data;
    state.ghatsLoaded = true;
    renderGhats(data);
  } catch (err) {
    $("#ghats").innerHTML = `<p class="spinner">Could not load ghat data: ${esc(err.message)}</p>`;
  }
}

function renderGhats(data) {
  $("#score-chart").innerHTML = `
    <div class="panel">
      <h2 class="panel-title">Risk score by ghat</h2>
      <p class="panel-sub">Weighted breaches of CPCB Class B, dissolved oxygen weighted heaviest.</p>
      ${scoreBars(data.ghats)}
    </div>`;

  $("#brief").innerHTML = `
    <div class="brief">
      <div class="brief-head">
        <h2>Brief to forward</h2>
        <div class="lang"><button type="button" class="lang-btn is-on" data-brief="en">EN</button>
          <button type="button" class="lang-btn" data-brief="hi">हिं</button></div>
      </div>
      <pre id="brief-text">${esc(data.brief_en)}</pre>
    </div>`;
  $$("[data-brief]").forEach((b) =>
    b.addEventListener("click", () => {
      $$("[data-brief]").forEach((x) => x.classList.toggle("is-on", x === b));
      $("#brief-text").textContent = b.dataset.brief === "hi" ? data.brief_hi : data.brief_en;
    })
  );

  $("#ghats").innerHTML = data.ghats
    .map((g) => {
      const ev = g.evidence
        .map(
          (e) => `<tr><td>${esc(e.parameter)}</td><td>${esc(e.value)}</td><td>${esc(e.phase)}</td>
            <td class="v-${esc(e.verdict)}">${esc(e.verdict)}</td><td>${esc(e.cite)}</td></tr>`
        )
        .join("");
      return `<article class="ghat${g.rank === 1 ? " open" : ""}">
        <div class="ghat-head">
          <span class="rank">${g.rank}</span>
          <div><div class="ghat-name">${esc(g.ghat)}</div><div class="ghat-body">${esc(g.water_body)}</div></div>
          <span class="tier ${slug(g.tier)}">${esc(g.tier)}</span>
        </div>
        <div class="ghat-bar">${readBar(g.counts)}</div>
        <div class="ghat-detail">
          <p class="reason">${esc(g.reason)}</p>
          <div class="chart-block">
            <div class="chart-title">Dissolved oxygen across test points <span class="hint">(limit ≥5 mg/l)</span></div>
            ${svgDO(g.do_series)}
          </div>
          ${ev ? `<table class="ev"><thead><tr><th>Parameter</th><th>Value</th><th>Phase</th><th>Verdict</th><th>Source</th></tr></thead><tbody>${ev}</tbody></table>` : ""}
        </div>
      </article>`;
    })
    .join("");
  $$(".ghat-head").forEach((h) => h.addEventListener("click", () => h.parentElement.classList.toggle("open")));
  $("#ghats-note").textContent = data.note;
}

function scoreBars(ghats) {
  const max = Math.max(...ghats.map((g) => g.score), 1);
  return ghats
    .map(
      (g) => `<div class="sb">
        <span class="sb-name">${esc(g.ghat)}</span>
        <span class="sb-track"><span class="sb-fill ${slug(g.tier)}" style="width:${(g.score / max) * 100}%"></span></span>
        <span class="sb-val">${g.score.toFixed(1)}</span>
      </div>`
    )
    .join("");
}

function readBar(c) {
  const t = c.pass + c.fail + c.low + c.missing || 1;
  const seg = (n, cls) => (n ? `<span class="r-${cls}" style="width:${(n / t) * 100}%"></span>` : "");
  return `<span class="rbar" title="fail ${c.fail} · low ${c.low} · missing ${c.missing} · pass ${c.pass}">
    ${seg(c.fail, "fail")}${seg(c.low, "low")}${seg(c.missing, "miss")}${seg(c.pass, "pass")}</span>
    <span class="rbar-key">${c.fail} fail · ${c.missing} missing · ${c.pass} pass</span>`;
}

/* ---------- track 2: dissolved-oxygen line chart ---------- */
function svgDO(series) {
  if (!series || !series.length) return "";
  const W = 660, H = 210, pl = 34, pr = 12, pt = 14, pb = 30;
  const iw = W - pl - pr, ih = H - pt - pb;
  const present = series.map((p, i) => [i, p.value]).filter(([, v]) => v != null);
  const maxY = Math.max(10, ...present.map(([, v]) => v), 5);
  const X = (i) => pl + (series.length > 1 ? (iw * i) / (series.length - 1) : iw / 2);
  const Y = (v) => pt + ih * (1 - v / maxY);

  let grid = "";
  for (const g of [0, 5, 10]) {
    if (g > maxY) continue;
    grid += `<line x1="${pl}" y1="${Y(g)}" x2="${W - pr}" y2="${Y(g)}" class="c-grid"/>
      <text x="${pl - 6}" y="${Y(g) + 3}" class="c-axis">${g}</text>`;
  }

  // Contiguous runs of present values -> one polyline each (gaps stay gaps).
  let runs = [], cur = [];
  series.forEach((p, i) => {
    if (p.value == null) {
      if (cur.length) runs.push(cur), (cur = []);
    } else cur.push([X(i), Y(p.value)]);
  });
  if (cur.length) runs.push(cur);
  const lines = runs
    .filter((r) => r.length > 1)
    .map((r) => `<polyline points="${r.map((p) => p.join(",")).join(" ")}" class="c-line"/>`)
    .join("");

  const dots = present
    .map(([i, v]) => `<circle cx="${X(i)}" cy="${Y(v)}" r="3.2" class="c-dot ${v < 5 ? "bad" : "good"}"/>`)
    .join("");

  const misses = series
    .map((p, i) => (p.value == null ? `<line x1="${X(i)}" y1="${Y(0)}" x2="${X(i)}" y2="${Y(0) - 5}" class="c-miss"/>` : ""))
    .join("");

  const labels = series
    .map((p, i) => (p.label.endsWith("pre") ? `<text x="${X(i)}" y="${H - 8}" class="c-axis">${esc(p.label.replace(" pre", ""))}</text>` : ""))
    .join("");

  return `<svg viewBox="0 0 ${W} ${H}" class="chart" role="img" aria-label="Dissolved oxygen over test points">
    ${grid}
    <line x1="${pl}" y1="${Y(5)}" x2="${W - pr}" y2="${Y(5)}" class="c-limit"/>
    <text x="${W - pr}" y="${Y(5) - 4}" class="c-limit-t">limit ≥5</text>
    ${misses}${lines}${dots}${labels}
  </svg>`;
}

setTrack(location.hash === "#visarjan" ? "visarjan" : "bhumi-vikas");
