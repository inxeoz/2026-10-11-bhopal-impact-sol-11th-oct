const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const slug = (t) => "tier-" + t.toLowerCase().replace(/[^a-z]+/g, "-").replace(/-+$/, "");
const num = (v, d = 1) => (v == null ? "—" : Number(v).toFixed(d).replace(/\.0$/, ""));

let state = { track: "bhumi-vikas", lang: "en" };

/* ---------- track switching ---------- */
function setTrack(track) {
  state.track = track;
  $$(".tab").forEach((t) => t.classList.toggle("is-on", t.dataset.track === track));
  $$(".view").forEach((v) => (v.hidden = v.id !== "view-" + track));
  $("main").classList.toggle("is-wide", track === "draw");
  if (track === "draw") mountDraw();
  history.replaceState(null, "", track === "bhumi-vikas" ? location.pathname : "#" + track);
}

/* Draw plot lives in static/draw.js (a module, loaded after this script). */
function mountDraw(tries = 0) {
  if (window.drawPlot) return window.drawPlot.mount();
  if (tries < 60) setTimeout(() => mountDraw(tries + 1), 50);
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

/* ---------- track 1: example gallery (static/examples.json, also run by tests/examples.rs) ---------- */
const QUICK = [
  { title: "Front open space", q: "My house faces a 10 m wide street and will be 9 m tall. How much open space at the front?" },
  { title: "Rear open space", q: "What is the minimum rear open space for a 150 sq m residential plot?" },
  { title: "Permission validity", q: "How long is a development permission valid, and how many times can I renew it?" },
  { title: "Out of scope", q: "How do I pay my property tax online?" },
].map((e) => ({ ...e, group: "Quick", lang: "en" }));

async function loadExamples() {
  let rich = [];
  try {
    rich = await (await fetch("/examples.json")).json();
  } catch {}
  const all = [...QUICK, ...rich];
  const groups = [...new Set(all.map((e) => e.group))];
  const box = $("#examples");
  box.innerHTML = `
    <div class="ex-bar">
      <div class="ex-tabs" role="tablist">${groups
        .map((g, i) => `<button type="button" class="ex-tab ${i === 1 ? "is-on" : ""}" data-group="${esc(g)}">${esc(g)}
          <span class="ex-n">${all.filter((e) => e.group === g).length}</span></button>`)
        .join("")}</div>
      <button type="button" class="ex-toggle">Browse ${all.length} examples</button>
    </div>
    <div class="ex-grid"></div>`;
  const show = (g) => {
    $$(".ex-tab", box).forEach((t) => t.classList.toggle("is-on", t.dataset.group === g));
    $(".ex-grid", box).innerHTML = all
      .map((e, i) => [e, i])
      .filter(([e]) => e.group === g)
      .map(([e, i]) => `<button type="button" class="ex-card" data-i="${i}" ${e.lang === "hi" ? 'lang="hi"' : ""}>
          <span class="ex-title">${esc(e.title)}</span><span class="ex-q">${esc(e.q)}</span></button>`)
      .join("");
    $$(".ex-card", box).forEach((c) =>
      c.addEventListener("click", () => {
        const e = all[c.dataset.i];
        setLang(e.lang || "en");
        $("#q").value = e.q;
        ask(e.q);
        window.scrollTo({ top: 0, behavior: "smooth" });
      })
    );
  };
  $$(".ex-tab", box).forEach((t) => t.addEventListener("click", () => (box.classList.remove("is-collapsed"), show(t.dataset.group))));
  $(".ex-toggle", box).addEventListener("click", () => box.classList.toggle("is-collapsed"));
  show(groups[1] ?? groups[0]);
}

function setLang(lang) {
  state.lang = lang;
  $$(".lang-btn[data-lang]").forEach((x) => x.classList.toggle("is-on", x.dataset.lang === lang));
}

loadExamples();

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
    $("#examples").classList.add("is-collapsed");
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
  const scene = a.facts && a.facts.scene;
  const diagram = scene
    ? `<div class="scene3d"><div class="s3-head"><span class="tag">Your plot in 3D</span>
         <span class="s3-sub">Built from your question · every size from the rule text</span></div><div class="s3-body"></div></div>`
    : setbackSVG(a.facts || {});
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
  if (scene) mount3D($("#answer .s3-body"), scene);
}

/* ---------- track 1: 3D plot (static/scene3d.js) ---------- */
function mount3D(el, scene, tries = 0) {
  // The module script loads deferred; wait briefly if it has not run yet.
  if (window.mountScene) return window.mountScene(el, scene);
  if (tries < 40) return setTimeout(() => mount3D(el, scene, tries + 1), 50);
  el.innerHTML = setbackSVG(scene.front ? { front_open_space_m: scene.front.m, street_width_m: scene.street_width_m } : {});
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

setTrack(location.hash === "#draw" ? "draw" : "bhumi-vikas");
