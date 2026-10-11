const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

let state = { track: "bhumi-vikas", lang: "en", ghatsLoaded: false, ghats: null };

/* ---- track switching ---- */
function setTrack(track) {
  state.track = track;
  $$(".tab").forEach((t) => t.classList.toggle("is-on", t.dataset.track === track));
  $$(".view").forEach((v) => (v.hidden = v.id !== "view-" + track));
  if (track === "visarjan" && !state.ghatsLoaded) loadGhats();
}
$$(".tab").forEach((t) => t.addEventListener("click", () => setTrack(t.dataset.track)));

/* ---- language toggle ---- */
$$(".lang-btn").forEach((b) =>
  b.addEventListener("click", () => {
    state.lang = b.dataset.lang;
    $$(".lang-btn").forEach((x) => x.classList.toggle("is-on", x === b));
  })
);

/* ---- ask ---- */
$("#ask-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const q = $("#q").value.trim();
  if (!q) return;
  ask(q);
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
  $("#answer").innerHTML = `
    <article class="card ${a.abstained ? "abstained" : "answered"}">
      <div class="card-head">
        <div class="tag">${a.abstained ? "Out of scope" : "Answer"}</div>
        <p class="verdict">${esc(a.answer)}</p>
        <p class="detail">${esc(a.detail)}</p>
      </div>
      ${a.note ? `<p class="note">${esc(a.note)}</p>` : ""}
      ${cites}
    </article>`;
}

/* ---- track 2 ---- */
async function loadGhats() {
  const box = $("#ghats");
  box.innerHTML = '<p class="spinner">Ranking ghats…</p>';
  try {
    const r = await fetch("/api/ghats");
    const data = await r.json();
    state.ghats = data;
    state.ghatsLoaded = true;
    renderGhats(data);
  } catch (err) {
    box.innerHTML = `<p class="spinner">Could not load ghat data: ${esc(err.message)}</p>`;
  }
}

function slug(t) {
  return "tier-" + t.toLowerCase().replace(/[^a-z]+/g, "-").replace(/-+$/, "");
}

function renderGhats(data) {
  $("#brief").innerHTML = `
    <div class="brief">
      <div class="brief-head">
        <h2>Brief to forward</h2>
        <div class="lang" role="group">
          <button type="button" class="lang-btn is-on" data-brief="en">EN</button>
          <button type="button" class="lang-btn" data-brief="hi">हिं</button>
        </div>
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
      return `<article class="ghat">
        <div class="ghat-head">
          <span class="rank">${g.rank}</span>
          <div><div class="ghat-name">${esc(g.ghat)}</div><div class="ghat-body">${esc(g.water_body)}</div></div>
          <span class="tier ${slug(g.tier)}">${esc(g.tier)}</span>
        </div>
        <div class="ghat-detail">
          <p class="reason">${esc(g.reason)}</p>
          ${ev ? `<table class="ev"><thead><tr><th>Parameter</th><th>Value</th><th>Phase</th><th>Verdict</th><th>Source</th></tr></thead><tbody>${ev}</tbody></table>` : ""}
        </div>
      </article>`;
    })
    .join("");
  $$(".ghat-head").forEach((h) => h.addEventListener("click", () => h.parentElement.classList.toggle("open")));
  $("#ghats-note").textContent = data.note;
}

setTrack("bhumi-vikas");
