const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const slug = (t) => "tier-" + t.toLowerCase().replace(/[^a-z]+/g, "-").replace(/-+$/, "");
const num = (v, d = 1) => (v == null ? "—" : Number(v).toFixed(d).replace(/\.0$/, ""));

let state = { track: "bhumi-vikas", heard: null };

/* The answer speaks the question's language: Hindi when the question is in Devanagari (typed or
 * spoken), English otherwise. A spoken question keeps the language Sarvam heard while its text is
 * unedited. Technical parts — rule numbers, quoted rule text, units, the 3D view — stay English. */
const DEVANAGARI = /[\u0900-\u097F]/;
function langOf(question) {
  if (state.heard?.text === question) return state.heard.lang;
  return DEVANAGARI.test(question) ? "hi" : "en";
}
/** Pick the English or Hindi wording. */
const tr = (lang, en, hi) => (lang === "hi" ? hi : en);

/* ---------- track switching ---------- */
function setTrack(track) {
  state.track = track;
  $$(".tab").forEach((t) => t.classList.toggle("is-on", t.dataset.track === track));
  $$(".view").forEach((v) => (v.hidden = v.id !== "view-" + track));
  document.body.classList.toggle("is-wide", track === "draw" || track === "doc");
  if (track === "doc") window.docViewer?.mount();
  if (track === "draw") mountDraw();
  history.replaceState(null, "", track === "bhumi-vikas" ? location.pathname : "#" + track);
}

/* Draw plot lives in static/draw.js (a module, loaded after this script). */
function mountDraw(tries = 0) {
  if (window.drawPlot) return window.drawPlot.mount();
  if (tries < 60) setTimeout(() => mountDraw(tries + 1), 50);
}
$$(".tab").forEach((t) => t.addEventListener("click", () => setTrack(t.dataset.track)));


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
  const groups = [...new Set(all.map((e) => e.group))].filter((g) => g !== "Quick");
  const box = $("#examples");
  const id = window.testIds();
  // Three quiet suggestions up front; the full gallery only on request.
  box.innerHTML = `
    <div class="ex-bar" data-testid="examples-bar">
      <span class="ex-try">Try</span>
      ${QUICK.slice(0, 3)
        .map((e) => `<button type="button" class="ex-chip" data-testid="${id("example-chip", e.title)}" data-i="${all.indexOf(e)}" title="${esc(e.q)}">${esc(e.title)}</button>`)
        .join("")}
      <button type="button" class="ex-toggle" data-testid="examples-toggle" aria-expanded="false">More examples</button>
    </div>
    <div class="ex-gallery" data-testid="examples-gallery">
      <div class="ex-tabs" role="tablist" data-testid="examples-tabs">${groups
        .map((g) => `<button type="button" class="ex-tab" data-testid="${id("example-tab", window.slug(g) || "hindi")}" data-group="${esc(g)}">${esc(g)}</button>`)
        .join("")}</div>
      <div class="ex-grid" data-testid="examples-grid"></div>
    </div>`;
  const pick = (i) => {
    const e = all[i];
    $("#q").value = e.q;
    ask(e.q);
    window.scrollTo({ top: 0, behavior: "smooth" });
  };
  const show = (g) => {
    // Cards are keyed by title; Hindi ones (non-Latin titles) by their place in the group.
    const cid = window.testIds();
    const cardId = (e) => (e.lang === "hi" ? cid("example-card-hindi", all.filter((x) => x.group === e.group).indexOf(e) + 1) : cid("example-card", e.title));
    $$(".ex-tab", box).forEach((t) => t.classList.toggle("is-on", t.dataset.group === g));
    $(".ex-grid", box).innerHTML = all
      .map((e, i) => [e, i])
      .filter(([e]) => e.group === g)
      .map(([e, i]) => {
        const t = cardId(e);
        return `<button type="button" class="ex-card" data-testid="${t}" data-i="${i}" ${e.lang === "hi" ? 'lang="hi"' : ""}>
          <span class="ex-title" data-testid="${t}-title">${esc(e.title)}</span><span class="ex-q" data-testid="${t}-question">${esc(e.q)}</span></button>`;
      })
      .join("");
    $$(".ex-card", box).forEach((c) => c.addEventListener("click", () => pick(c.dataset.i)));
  };
  $$(".ex-chip", box).forEach((c) => c.addEventListener("click", () => pick(c.dataset.i)));
  $$(".ex-tab", box).forEach((t) => t.addEventListener("click", () => show(t.dataset.group)));
  $(".ex-toggle", box).addEventListener("click", (ev) => {
    const open = box.classList.toggle("is-open");
    box.classList.remove("is-collapsed");
    ev.currentTarget.setAttribute("aria-expanded", open);
    ev.currentTarget.textContent = open ? "Fewer examples" : "More examples";
  });
  show(groups[0]);
}


loadExamples();

/* Reset: back to an empty question with the example gallery open. */
$("#reset").addEventListener("click", () => {
  $("#q").value = "";
  window.disposeScene?.();
  $("#answer").innerHTML = "";
  $("#examples").classList.remove("is-collapsed");
  $("#reset").hidden = true;
  discardClip();
  voiceStatus("");
  $("#q").focus();
});

/* ---------- track 1: voice input (Sarvam speech-to-text via /api/transcribe) ----------
 * Click the mic, speak, press Stop. The recording stays in the browser until you press
 * "Send recording" (play it back, delete it or record again first). The words then land in the
 * question box for review — nothing is asked until you press Ask — and the answer comes back in
 * the language heard.
 * The system default input can be the wrong one (a muted laptop mic while a headset is on), so
 * the bar lists every microphone, remembers the choice, shows a live level, and says so when a
 * recording came through silent. */
const MAX_REC_S = 29; // Sarvam's REST call takes up to 30 s of audio
const MIC_KEY = "rc-mic"; // chosen microphone's deviceId
const SILENT_RMS = 0.01; // below this for the whole take = nothing reached the mic (about -40 dBFS)
const voice = { rec: null, chunks: [], t0: 0, timer: null, cancelled: false, clip: null, audio: null, meter: null, peak: 0, mic: "" };
const clock = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

async function initVoice() {
  if (!navigator.mediaDevices?.getUserMedia || !window.MediaRecorder) return;
  try {
    if (!(await (await fetch("/api/voice")).json()).available) return;
  } catch {
    return;
  }
  const mic = $("#mic");
  mic.hidden = false;
  mic.addEventListener("click", () => {
    if (mic.classList.contains("is-busy")) return;
    voice.rec ? voice.rec.stop() : startVoice();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && voice.rec) cancelVoice();
  });
  $("#clip-stop").addEventListener("click", () => voice.rec?.stop());
  $("#clip-cancel").addEventListener("click", cancelVoice);
  $("#clip-play").addEventListener("click", togglePlay);
  $("#clip-send").addEventListener("click", sendClip);
  $("#clip-discard").addEventListener("click", () => (discardClip(), voiceStatus("")));
  $("#clip-again").addEventListener("click", () => startVoice());
  $("#mic-device").addEventListener("change", (e) => {
    localStorage.setItem(MIC_KEY, e.target.value);
    if (voice.rec) cancelVoice(), startVoice(); // switch mid-take: start over on the new mic
  });
  navigator.mediaDevices.addEventListener?.("devicechange", listMics);
}

function setMic(mode) {
  const mic = $("#mic");
  mic.classList.toggle("is-recording", mode === "recording");
  mic.classList.toggle("is-busy", mode === "busy");
  mic.setAttribute("aria-pressed", mode === "recording");
  const label = { recording: "Stop recording", busy: "Turning speech into text" }[mode] ?? "Record your question";
  mic.title = label;
  mic.setAttribute("aria-label", label);
}

function voiceStatus(text, error = false) {
  const el = $("#voice-status");
  el.textContent = text;
  el.hidden = !text;
  el.classList.toggle("is-error", error);
}

function showBar(state) {
  const bar = $("#voice-clip");
  bar.dataset.state = state;
  bar.hidden = !state;
}

/* Fill the microphone picker; device names are only readable after mic permission. */
async function listMics() {
  const sel = $("#mic-device");
  const mics = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === "audioinput" && d.deviceId);
  sel.innerHTML = mics
    .map((d, i) => `<option value="${esc(d.deviceId)}" data-testid="${esc("ask-voice-device-" + (window.slug(d.label) || i + 1))}">${esc(d.label || `Microphone ${i + 1}`)}</option>`)
    .join("");
  if (voice.mic) sel.value = voice.mic;
  sel.hidden = mics.length < 2;
}
const micName = () => $("#mic-device").selectedOptions[0]?.textContent || "the microphone";

async function openMic() {
  const saved = localStorage.getItem(MIC_KEY);
  if (saved) {
    try {
      return await navigator.mediaDevices.getUserMedia({ audio: { deviceId: { exact: saved } } });
    } catch {
      localStorage.removeItem(MIC_KEY); // unplugged or renamed: fall back to the default
    }
  }
  return navigator.mediaDevices.getUserMedia({ audio: true });
}

async function startVoice() {
  discardClip();
  let stream;
  try {
    stream = await openMic();
  } catch {
    return voiceStatus("The microphone is blocked. Allow it for this site, then try again.", true);
  }
  voice.mic = stream.getAudioTracks()[0]?.getSettings().deviceId ?? "";
  await listMics();
  startMeter(stream);
  const rec = new MediaRecorder(stream);
  Object.assign(voice, { rec, chunks: [], t0: Date.now(), cancelled: false, peak: 0 });
  rec.ondataavailable = (e) => e.data.size && voice.chunks.push(e.data);
  rec.onstop = () => {
    stream.getTracks().forEach((t) => t.stop());
    clearInterval(voice.timer);
    stopMeter();
    if (voice.rec === rec) voice.rec = null;
    setMic("idle");
    if (voice.cancelled) return;
    keepClip(new Blob(voice.chunks, { type: rec.mimeType || "audio/webm" }), (Date.now() - voice.t0) / 1000);
  };
  rec.start();
  setMic("recording");
  showBar("recording");
  voiceStatus("");
  const tick = () => {
    const s = (Date.now() - voice.t0) / 1000;
    $("#clip-length").textContent = clock(s);
    if (s >= MAX_REC_S) rec.stop();
  };
  tick();
  voice.timer = setInterval(tick, 250);
  $("#clip-stop").focus();
}

function cancelVoice() {
  if (!voice.rec) return;
  voice.cancelled = true;
  voice.rec.stop();
  voice.rec = null;
  showBar("");
}

/* Live input level: drives the pulsing dot and records the loudest moment of the take. */
function startMeter(stream) {
  try {
    const ctx = new AudioContext();
    const an = ctx.createAnalyser();
    an.fftSize = 1024;
    ctx.createMediaStreamSource(stream).connect(an);
    const buf = new Float32Array(an.fftSize);
    const dot = $(".clip-meter i");
    const loop = () => {
      an.getFloatTimeDomainData(buf);
      const rms = Math.sqrt(buf.reduce((a, v) => a + v * v, 0) / buf.length);
      voice.peak = Math.max(voice.peak, rms);
      dot.style.setProperty("--lv", Math.min(1, rms * 8).toFixed(2));
      voice.meter.raf = requestAnimationFrame(loop);
    };
    voice.meter = { ctx, raf: 0 };
    loop();
  } catch {
    voice.meter = null; // no Web Audio: record anyway, just without the level
    voice.peak = 1;
  }
}

function stopMeter() {
  if (!voice.meter) return;
  cancelAnimationFrame(voice.meter.raf);
  voice.meter.ctx.close();
  voice.meter = null;
}

/* The saved recording: kept in the browser only, until "Send recording". */
function keepClip(blob, secs) {
  if (!blob.size) return showBar(""), voiceStatus("Nothing was recorded. Try again.", true);
  const url = URL.createObjectURL(blob);
  const audio = new Audio(url);
  audio.onended = audio.onpause = () => setPlaying(false);
  audio.onplay = () => setPlaying(true);
  voice.clip = { blob, url, secs };
  voice.audio = audio;
  $("#clip-length").textContent = clock(secs);
  showBar("saved");
  if (voice.peak < SILENT_RMS)
    voiceStatus(`No sound came through “${micName()}”. It may be muted or not the mic you are using — pick another microphone and record again.`, true);
  else voiceStatus("");
  $("#clip-send").focus();
}

function discardClip() {
  voice.audio?.pause();
  if (voice.clip) URL.revokeObjectURL(voice.clip.url);
  voice.clip = voice.audio = null;
  if (!voice.rec) showBar("");
}

function togglePlay() {
  const a = voice.audio;
  if (!a) return;
  if (a.paused) {
    a.currentTime = 0;
    a.play();
  } else a.pause();
}

function setPlaying(on) {
  const b = $("#clip-play");
  b.classList.toggle("is-playing", on);
  b.setAttribute("aria-label", on ? "Stop playback" : "Play recording");
  b.title = on ? "Stop playback" : "Play recording";
}

async function sendClip() {
  if (!voice.clip) return;
  voice.audio?.pause();
  const send = $("#clip-send");
  send.disabled = true;
  setMic("busy");
  voiceStatus("Turning your speech into text…");
  try {
    const r = await fetch("/api/transcribe", { method: "POST", headers: { "content-type": voice.clip.blob.type }, body: voice.clip.blob });
    if (!r.ok) throw new Error(await r.text());
    const { transcript, language_code } = await r.json();
    if (!transcript?.trim())
      return voiceStatus(`Didn't catch any words. Check that “${micName()}” is the mic you are speaking into, then record again.`, true);
    $("#q").value = transcript.trim();
    state.heard = { text: transcript.trim(), lang: language_code?.startsWith("hi") ? "hi" : "en" };
    discardClip();
    $("#q").focus();
    voiceStatus("Check the question, then press Ask.");
  } catch (err) {
    // Keep the recording so it can be sent again.
    voiceStatus(`Could not turn speech into text: ${err.message}`, true);
  } finally {
    send.disabled = false;
    setMic("idle");
  }
}

initVoice();

/** Ask a question. `details` are plot details filled in after asking; without them, an answer
 * that would rest on assumed inputs first asks for those inputs, step by step. */
async function ask(question, details = null) {
  const box = $("#answer");
  voiceStatus("");
  if (!details) state.flow = null; // a new question starts its own steps
  const lang = langOf(question);
  box.innerHTML = `<p class="spinner" data-testid="answer-loading">${
    details ? tr(lang, "Working out your plot…", "आपके प्लॉट का हिसाब लगा रहे हैं…") : tr(lang, "Looking through the rulebook…", "नियम पुस्तिका में देख रहे हैं…")
  }</p>`;
  try {
    const r = await fetch("/api/ask", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ track: state.track, question, lang, ...(details ? { details } : {}) }),
    });
    if (!r.ok) throw new Error(await r.text());
    const a = await r.json();
    const steps = details ? [] : detailSteps(a.facts?.scene);
    if (steps.length) startDetails({ question, lang, answer: a, steps, details: {} });
    else renderAnswer(a, details && { question, lang, details });
    collapseExamples();
  } catch (err) {
    box.innerHTML = `<div class="card" data-testid="answer-error"><div class="card-head"><p class="verdict" data-testid="answer-error-title">Could not reach the tool.</p>
      <p class="detail" data-testid="answer-error-detail">${esc(err.message)}</p></div></div>`;
  }
  $("#reset").hidden = false;
}

/* ---------- track 1: plot details, asked instead of assumed ----------
 * The rules need the plot's use, size, road and height. When the question leaves one out, the
 * server would fill a typical value and label it "Assumed"; instead the missing ones are asked
 * here, one per step, then the question is answered again with them (POST /api/ask `details`). */
const DETAIL_ORDER = ["use_type", "plot_area", "frontage", "street_width", "height", "floors", "building_type"];
// Same uses as the 3D view's Use menu (static/scene3d.js): value, [English, Hindi] label and note.
const USE_OPTIONS = [
  ["residential", ["Home", "घर"], ["House or flats", "मकान या फ़्लैट"]],
  ["business", ["Shop / office", "दुकान / दफ़्तर"], ["Commercial", "व्यावसायिक"]],
  ["educational", ["School / college", "स्कूल / कॉलेज"], ["", ""]],
  ["institutional", ["Hospital", "अस्पताल"], ["Or other institution", "या कोई और संस्था"]],
  ["assembly", ["Hall / cinema", "हॉल / सिनेमा"], ["Gathering place", "सभा स्थल"]],
  ["industrial", ["Factory", "फ़ैक्टरी"], ["Industrial", "औद्योगिक"]],
  ["farm_house", ["Farm house", "फ़ार्म हाउस"], ["", ""]],
];
const HOUSE_TYPES = [
  ["detached", ["Detached", "डिटैच्ड (अलग)"], ["Open on all four sides", "चारों ओर खुला"], ["detached", "डिटैच्ड"]],
  ["semi-detached", ["Semi-detached", "सेमी-डिटैच्ड"], ["Shares one side wall", "एक तरफ़ की दीवार साझा"], ["semi-detached", "सेमी-डिटैच्ड"]],
  ["row", ["Row house", "रो हाउस"], ["Shares both side walls", "दोनों तरफ़ की दीवारें साझा"], ["row house", "रो हाउस"]],
];
const SQFT = 10.764; // sq ft per sq m
const FT = 0.3048; // m per ft
const half = (v) => Math.round(v * 2) / 2;

/** The steps a scene needs: one per input the server had to assume. */
function detailSteps(scene) {
  const missing = new Set((scene?.assumed ?? []).map((x) => x.field));
  return DETAIL_ORDER.filter((f) => missing.has(f));
}

/** What one step asks, with its quick choices and the free-entry units. Numbers and units stay
 * as written ("200 sq m"); the words around them follow the question's language. */
function stepDef(field, flow) {
  const d = flow.details, s = flow.answer.facts.scene, L = flow.lang;
  const t = (en, hi) => tr(L, en, hi);
  const area = d.plot_area ?? s.plot_area_sqm;
  const len = [["m", 1], ["ft", FT]];
  switch (field) {
    case "use_type":
      return { key: "use_type", title: t("What is the building for?", "इमारत किस काम के लिए है?"), options: USE_OPTIONS.map(([v, l, sub]) => ({ v, l: t(...l), sub: t(...sub) })) };
    case "plot_area":
      return {
        key: "plot_area", title: t("How big is the plot?", "प्लॉट कितना बड़ा है?"), hint: t("The whole plot, not the built area.", "पूरा प्लॉट, सिर्फ़ बना हुआ हिस्सा नहीं।"),
        options: [100, 150, 200, 300, 500].map((v) => ({ v, l: `${v} sq m`, sub: `≈ ${Math.round(v * SQFT).toLocaleString("en-IN")} sq ft` })),
        units: [["sq m", 1], ["sq ft", 1 / SQFT]],
      };
    case "frontage": {
      const fs = [...new Set([1, 1.5, 2].map((r) => half(Math.sqrt(area / r))))];
      return {
        key: "frontage", title: t("How wide is the plot along the road?", "सड़क की तरफ़ प्लॉट कितना चौड़ा है?"),
        hint: t(`For a ${fmtNum(area)} sq m plot.`, `${fmtNum(area)} sq m के प्लॉट के लिए।`),
        options: fs.map((v) => ({ v, l: t(`${v} m wide`, `${v} m चौड़ा`), sub: t(`${fmtNum(area / v)} m deep`, `${fmtNum(area / v)} m गहरा`) })),
        units: len,
      };
    }
    case "street_width":
      return {
        key: "street_width", title: t("How wide is the road in front?", "सामने की सड़क कितनी चौड़ी है?"),
        hint: t("Edge to edge, as on the plot's papers.", "एक किनारे से दूसरे किनारे तक, जैसा प्लॉट के काग़ज़ों में है।"),
        options: [4.5, 6, 7.5, 9, 12, 18, 24].map((v) => ({ v, l: `${v} m`, sub: `≈ ${Math.round(v / FT)} ft` })),
        units: len,
      };
    case "height":
      return {
        key: "height", title: t("How tall will the building be?", "इमारत कितनी ऊँची होगी?"),
        hint: t("Up to the roof. The choices use 3 m per floor.", "छत तक। विकल्पों में हर मंज़िल 3 m की है।"),
        options: [1, 2, 3, 4, 5].map((n) => ({
          v: n * 3,
          l: n === 1 ? t("Ground floor only", "सिर्फ़ भूतल") : `G+${n - 1}`,
          sub: t(`${n} floor${n > 1 ? "s" : ""}`, `${n} मंज़िल`) + ` · ${n * 3} m`,
        })),
        units: len,
      };
    case "floors": {
      const n = s.floors;
      return {
        key: "height", title: t(`How tall is each of the ${n} floors?`, `${n} मंज़िलों में हर मंज़िल कितनी ऊँची है?`),
        hint: t("Floor to floor, including the slab.", "फ़र्श से फ़र्श तक, स्लैब समेत।"),
        options: [3, 3.2, 3.5].map((v) => ({ v: Math.round(n * v * 100) / 100, l: t(`${v} m each`, `हर एक ${v} m`), sub: t(`${fmtNum(n * v)} m in all`, `कुल ${fmtNum(n * v)} m`) })),
        units: [[t("m per floor", "m प्रति मंज़िल"), n], [t("ft per floor", "ft प्रति मंज़िल"), n * FT]],
      };
    }
    case "building_type":
      return {
        key: "building_type", title: t("How does the house sit on the plot?", "मकान प्लॉट पर कैसे बना है?"),
        options: HOUSE_TYPES.map(([v, l, sub]) => ({ v, l: t(...l), sub: t(...sub) })),
      };
  }
}
const fmtNum = (v) => String(Math.round(v * 10) / 10);

function startDetails(flow) {
  state.flow = { ...flow, i: 0 };
  renderStep();
}

/** Steps still to show: a building type only matters for homes. */
function liveSteps(flow) {
  const home = (flow.details.use_type ?? flow.answer.facts.scene.use_type) === "residential";
  return flow.steps.filter((f) => f !== "building_type" || home);
}

function renderStep() {
  const flow = state.flow;
  const steps = liveSteps(flow);
  if (flow.i >= steps.length) return ask(flow.question, flow.details);
  const field = steps[flow.i];
  const st = stepDef(field, flow);
  const cur = flow.details[st.key];
  const L = flow.lang;
  const t = `ask-details-${field.replace(/_/g, "-")}`;
  $("#answer").innerHTML = `
    <article class="card details" data-testid="ask-details" lang="${L}">
      <div class="card-head">
        <p class="details-top"><span>${tr(L, "A few details about your plot", "आपके प्लॉट के बारे में कुछ जानकारी")}</span><span data-testid="ask-details-progress">${tr(L, `Step ${flow.i + 1} of ${steps.length}`, `चरण ${flow.i + 1} / ${steps.length}`)}</span></p>
        <div class="details-bar" aria-hidden="true"><i style="--p:${(flow.i / steps.length).toFixed(3)}"></i></div>
        <h2 class="details-q" data-testid="${t}-question">${esc(st.title)}</h2>
        ${st.hint ? `<p class="detail" data-testid="${t}-hint">${esc(st.hint)}</p>` : ""}
      </div>
      <div class="details-options" role="radiogroup" aria-label="${esc(st.title)}" data-testid="${t}-options">
        ${st.options
          .map((o) => `<button type="button" role="radio" aria-checked="${o.v === cur}" class="details-opt ${o.v === cur ? "is-on" : ""}" data-v="${esc(o.v)}"
              data-testid="${t}-option-${window.slug(String(o.v))}"><b>${esc(o.l)}</b>${o.sub ? `<span>${esc(o.sub)}</span>` : ""}</button>`)
          .join("")}
      </div>
      ${st.units ? `<form class="details-other" data-testid="${t}-other">
          <label for="details-num">${tr(L, "Other", "अन्य")}</label>
          <input id="details-num" type="number" inputmode="decimal" min="0" step="any" data-testid="${t}-input" placeholder="${tr(L, "Type a number", "संख्या लिखें")}">
          <select aria-label="Unit" data-testid="${t}-unit">${st.units.map(([u], k) => `<option value="${k}">${u}</option>`).join("")}</select>
          <button type="submit" class="primary" data-testid="${t}-next">${tr(L, "Next", "आगे")}</button>
        </form>` : ""}
      <div class="details-foot">
        ${flow.i ? `<button type="button" class="clip-link" data-back data-testid="ask-details-back">← ${tr(L, "Back", "पीछे")}</button>` : "<span></span>"}
        <button type="button" class="clip-link" data-skip data-testid="ask-details-skip" title="${tr(L, "Answer now with typical values, shown as assumed", "अभी जवाब दें, सामान्य मान मानकर")}">${tr(L, "Skip, use typical values", "छोड़ें, सामान्य मान लें")}</button>
      </div>
    </article>`;
  const box = $("#answer");
  const take = (v) => {
    flow.details[st.key] = v;
    flow.i++;
    renderStep();
  };
  box.querySelectorAll(".details-opt").forEach((b) => {
    const o = st.options.find((x) => String(x.v) === b.dataset.v);
    b.addEventListener("click", () => take(o.v));
  });
  box.querySelector(".details-other")?.addEventListener("submit", (e) => {
    e.preventDefault();
    const n = Number(box.querySelector("#details-num").value);
    if (!(n > 0)) return box.querySelector("#details-num").focus();
    take(Math.round(n * st.units[box.querySelector(".details-other select").value][1] * 100) / 100);
  });
  box.querySelector("[data-back]")?.addEventListener("click", () => (flow.i--, renderStep()));
  box.querySelector("[data-skip]").addEventListener("click", () => renderAnswer(flow.answer));
  (box.querySelector(".details-opt.is-on") ?? box.querySelector(".details-opt"))?.focus({ preventScroll: true });
}

/** "Your plot: 200 sq m · 10 m wide …" under an answer built from filled-in details. */
function detailsSummary(d, L) {
  const t = (en, hi) => tr(L, en, hi);
  const use = USE_OPTIONS.find(([v]) => v === d.use_type)?.[1];
  const type = HOUSE_TYPES.find(([v]) => v === d.building_type)?.[3];
  return [use && t(...use), d.plot_area && `${fmtNum(d.plot_area)} sq m`, d.frontage && t(`${fmtNum(d.frontage)} m wide`, `${fmtNum(d.frontage)} m चौड़ा`),
    d.street_width && t(`${fmtNum(d.street_width)} m road`, `${fmtNum(d.street_width)} m सड़क`), d.height && t(`${fmtNum(d.height)} m tall`, `${fmtNum(d.height)} m ऊँचा`), type && t(...type)]
    .filter(Boolean).join(" · ");
}

function collapseExamples() {
  const box = $("#examples");
  box.classList.remove("is-open");
  box.classList.add("is-collapsed");
  const t = $(".ex-toggle", box);
  if (t) (t.textContent = "Examples"), t.setAttribute("aria-expanded", "false");
}

function renderAnswer(a, given = null) {
  const id = window.testIds();
  const cites = (a.sources || [])
    .map((s) => {
      const t = id("answer-cite", s.label);
      return `<div class="cite" data-testid="${t}">
        <div class="cite-head"><span class="rule" data-testid="${t}-rule">${esc(s.label)}</span><span class="page" data-testid="${t}-page">${esc(s.cite)}</span>${window.docButton ? window.docButton(s, `${t}-show-in-document`) : ""}</div>
        <pre class="quote" data-testid="${t}-quote">${esc(s.quote)}</pre></div>`;
    })
    .join("");
  const scene = a.facts && a.facts.scene;
  const L = a.lang === "hi" ? "hi" : "en";
  // Order: the answer, the rules it applied, then the plot to adjust (changes wait for "Apply changes").
  const rules = cites || a.note
    ? `<section class="rules" data-testid="answer-rules">
        ${cites ? `<h3 class="s3-title" data-testid="answer-rules-title">${tr(L, "Rules applied", "लागू नियम")}</h3>${cites}` : ""}
        ${a.note ? `<p class="note" data-testid="answer-note">${esc(a.note)}</p>` : ""}
      </section>`
    : "";
  const diagram = scene
    ? `<div class="scene3d" data-testid="answer-3d"><h3 class="s3-title" data-testid="answer-3d-title">Your plot in 3D</h3><div class="s3-body" data-testid="answer-3d-body"></div></div>`
    : setbackSVG(a.facts || {});
  $("#answer").innerHTML = `
    <article class="card ${a.abstained ? "abstained" : "answered"}" data-testid="answer-card">
      <div class="card-head" data-testid="answer-head" lang="${L}">
        ${a.abstained ? `<div class="tag" data-testid="answer-out-of-scope">${tr(a.lang, "Out of scope", "दायरे से बाहर")}</div>` : ""}
        <p class="verdict" data-testid="answer-verdict">${esc(a.answer)}</p>
        <p class="detail" data-testid="answer-detail">${esc(a.detail)}</p>
        ${given ? `<p class="details-summary" data-testid="answer-plot-details">${tr(L, "Your plot:", "आपका प्लॉट:")} ${esc(detailsSummary(given.details, L))}
          ${state.flow?.steps?.length ? `<button type="button" class="clip-link" data-testid="answer-plot-details-change">${tr(L, "Change", "बदलें")}</button>` : ""}</p>` : ""}
      </div>
      ${rules}
      ${diagram}
    </article>`;
  // Apply changes: answer the same question again with the plot as now set.
  if (scene) mount3D($("#answer .s3-body"), scene, { onApply: (details) => ask(a.question, details) });
  // Change: run the same steps again, starting from the answers already given.
  $('[data-testid="answer-plot-details-change"]')?.addEventListener("click", () => {
    state.flow = { ...state.flow, question: given.question, lang: given.lang, details: { ...given.details }, i: 0 };
    renderStep();
  });
}

/* ---------- track 1: 3D plot (static/scene3d.js) ---------- */
function mount3D(el, scene, opts, tries = 0) {
  // The module script loads deferred; wait briefly if it has not run yet.
  if (window.mountScene) return window.mountScene(el, scene, opts);
  if (tries < 40) return setTimeout(() => mount3D(el, scene, opts, tries + 1), 50);
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
  <div class="diagram" data-testid="answer-diagram">
    <svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Setback diagram" data-testid="answer-diagram-svg">
      <rect x="${px}" y="${plotTop}" width="${pw}" height="${plotBot - plotTop}" class="d-plot" data-testid="answer-diagram-plot"/>
      <rect x="${px}" y="${bldgTop}" width="${pw}" height="${bldgBot - bldgTop}" class="d-bldg" data-testid="answer-diagram-building"/>
      <text x="${px + pw / 2}" y="${(bldgTop + bldgBot) / 2 + 4}" class="d-label-c">building</text>
      <rect x="${px}" y="${gapTop ? bldgBot : plotTop}" width="${pw}" height="${gapH}" class="d-gap" data-testid="answer-diagram-open-space"/>
      <text x="${px + pw / 2}" y="${(gapTop ? bldgBot : plotTop) + gapH / 2 + 4}" class="d-label-c d-gap-t">${gapLabel} · ${num(v)} m</text>
      <rect x="60" y="${streetY}" width="440" height="${streetH}" class="d-street" data-testid="answer-diagram-street"/>
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

setTrack({ "#draw": "draw", "#doc": "doc" }[location.hash] ?? "bhumi-vikas");
