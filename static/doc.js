/* Document viewer: the rulebook text page by page (GET /api/doc), with "Open Document" buttons
 * anywhere a rule is cited. Opening jumps to the cited page and highlights the quoted line —
 * exactly when the quote is verbatim, otherwise the longest run of its words found on the page.
 * Loaded as a plain script before app.js so every view can call window.docButton(). */
(() => {
  const esc = (s) =>
    String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

  let data = null;
  let root = null;
  let built = false;
  let marked = []; // pages whose <pre> currently holds a <mark>
  let find = { q: "", hits: [], i: -1 };
  let backTo = null;

  /** "Open Document" button for a Source {label, cite, quote}; "" if it has no page. */
  window.docButton = (src, text = "Open Document") => {
    const m = /(\d+)/.exec(src?.cite ?? "");
    if (!m) return "";
    return `<button type="button" class="open-doc" data-page="${m[1]}" data-quote="${esc(src.quote)}" data-label="${esc(src.label)}"
      title="Show ${esc(src.label)} on PDF page ${m[1]}">${text}</button>`;
  };

  document.addEventListener("click", (e) => {
    const b = e.target.closest(".open-doc");
    if (!b) return;
    e.preventDefault();
    e.stopPropagation();
    open({ page: +b.dataset.page, quote: b.dataset.quote, label: b.dataset.label });
  }, true);

  async function load() {
    if (!data) data = await (await fetch("/api/doc")).json();
    return data;
  }

  async function mount() {
    root = document.getElementById("doc-root");
    if (built) return;
    root.innerHTML = '<p class="spinner">Loading the rulebook…</p>';
    try {
      await load();
    } catch (err) {
      root.innerHTML = `<p class="s3-warn">Could not load the rulebook: ${esc(err.message)}</p>`;
      return;
    }
    build();
    built = true;
  }

  function build() {
    const n = data.pages.length;
    root.innerHTML = `
      <div class="dv">
        <aside class="dv-side">
          <div class="dv-side-head">
            <b>Rules</b>
            <input type="search" class="dv-filter" placeholder="Filter: number or words">
          </div>
          <ol class="dv-rules">${data.rules
            .map((r) => `<li data-page="${r.page}" title="Rule ${esc(r.rule)} starts on page ${r.page}">
                <span class="dv-rn">${esc(r.rule)}</span><span class="dv-rt">${esc(r.title)}</span><span class="dv-rp">p.${r.page}</span></li>`)
            .join("")}</ol>
        </aside>
        <div class="dv-main">
          <div class="dv-bar">
            <button type="button" class="ghost dv-back" hidden>← Back</button>
            <div class="dv-nav">
              <button type="button" data-nav="-1" title="Previous page">↑</button>
              <label>Page <input type="number" class="dv-pnum" min="1" max="${n}" value="1"> of ${n}</label>
              <button type="button" data-nav="1" title="Next page">↓</button>
            </div>
            <div class="dv-find">
              <input type="search" class="dv-q" placeholder="Find in document">
              <span class="dv-count"></span>
              <button type="button" data-find="-1" title="Previous match">‹</button>
              <button type="button" data-find="1" title="Next match">›</button>
            </div>
            <a class="dv-pdf" target="_blank" rel="noopener" href="${data.pdf}#page=1">Original PDF ↗</a>
          </div>
          <div class="dv-note" hidden></div>
          <div class="dv-scroll">${data.pages
            .map((p) => `<section class="dv-page" id="dv-p${p.page}" data-page="${p.page}">
                <div class="dv-ph"><span>PDF page ${p.page}</span><a href="${data.pdf}#page=${p.page}" target="_blank" rel="noopener">open in PDF ↗</a></div>
                <pre>${esc(p.text)}</pre></section>`)
            .join("")}</div>
        </div>
      </div>`;
    const $ = (s) => root.querySelector(s);
    const scroll = $(".dv-scroll");

    // Current page follows the scroll position.
    const io = new IntersectionObserver(
      (entries) => {
        const vis = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
        if (vis[0]) setCurrent(+vis[0].target.dataset.page);
      },
      { root: scroll, rootMargin: "0px 0px -70% 0px" }
    );
    root.querySelectorAll(".dv-page").forEach((s) => io.observe(s));

    root.querySelectorAll("[data-nav]").forEach((b) =>
      b.addEventListener("click", () => goPage(current() + Number(b.dataset.nav)))
    );
    $(".dv-pnum").addEventListener("change", (e) => goPage(Number(e.target.value)));
    $(".dv-rules").addEventListener("click", (e) => {
      const li = e.target.closest("li");
      if (li) goPage(+li.dataset.page);
    });
    $(".dv-filter").addEventListener("input", (e) => {
      const q = e.target.value.trim().toLowerCase();
      root.querySelectorAll(".dv-rules li").forEach((li) => (li.hidden = q && !li.textContent.toLowerCase().includes(q)));
    });
    $(".dv-q").addEventListener("keydown", (e) => {
      if (e.key === "Enter") step(e.shiftKey ? -1 : 1);
    });
    $(".dv-q").addEventListener("input", () => {
      find = { q: "", hits: [], i: -1 };
      $(".dv-count").textContent = "";
    });
    root.querySelectorAll("[data-find]").forEach((b) => b.addEventListener("click", () => step(Number(b.dataset.find))));
    $(".dv-back").addEventListener("click", () => {
      if (backTo) window.setTrack?.(backTo);
      backTo = null;
      $(".dv-back").hidden = true;
    });
  }

  const q = (s) => root.querySelector(s);
  const current = () => Number(q(".dv-pnum").value) || 1;

  function setCurrent(n) {
    q(".dv-pnum").value = n;
    q(".dv-pdf").href = `${data.pdf}#page=${n}`;
    q(".dv-pdf").textContent = `Original PDF, page ${n} ↗`;
  }

  function goPage(n, el) {
    const max = data.pages.length;
    n = Math.max(1, Math.min(max, n || 1));
    const scroll = q(".dv-scroll");
    const target = el ?? q(`#dv-p${n}`);
    if (!target) return;
    const top = target.getBoundingClientRect().top - scroll.getBoundingClientRect().top + scroll.scrollTop;
    scroll.scrollTo({ top: el ? top - scroll.clientHeight / 3 : top - 8, behavior: "smooth" });
    setCurrent(n);
  }

  function clearMarks() {
    for (const n of marked) {
      const pre = q(`#dv-p${n} pre`);
      const page = data.pages.find((p) => p.page === n);
      if (pre && page) pre.textContent = page.text;
    }
    marked = [];
  }

  /** Highlight one or more ranges of a page's text; returns the first <mark>. */
  function markRange(n, start, end, ranges = [{ start, end }]) {
    const page = data.pages.find((p) => p.page === n);
    const pre = q(`#dv-p${n} pre`);
    if (!page || !pre) return null;
    const t = page.text;
    let html = "", at = 0;
    for (const r of ranges) {
      if (r.start < at) continue;
      html += `${esc(t.slice(at, r.start))}<mark class="dv-hl">${esc(t.slice(r.start, r.end))}</mark>`;
      at = r.end;
    }
    pre.innerHTML = html + esc(t.slice(at));
    marked.push(n);
    return pre.querySelector("mark");
  }

  /** Lowercased text with runs of whitespace collapsed, plus a map back to original offsets. */
  function normalize(t) {
    let out = "", map = [], space = false;
    for (let i = 0; i < t.length; i++) {
      const c = t[i];
      if (/\s/.test(c)) {
        if (!space && out.length) (out += " "), map.push(i);
        space = true;
      } else {
        out += c.toLowerCase();
        map.push(i);
        space = false;
      }
    }
    return { out, map };
  }

  /** Where a quote sits in a page: exact (whitespace-insensitive), else the longest run of its words. */
  function locate(text, quote) {
    const { out, map } = normalize(text);
    const norm = (s) => s.replace(/\s+/g, " ").trim().toLowerCase();
    // The app sometimes prefixes quotes ("Front open spaces — …", "Table 4 row 1: …").
    const variants = [quote, quote.split(" — ").slice(1).join(" — "), quote.split(": ").slice(1).join(": ")]
      .map(norm)
      .filter((v) => v.length >= 8);
    const span = (i, len) => ({ start: map[i], end: map[i + len - 1] + 1 });
    for (const v of variants) {
      const i = out.indexOf(v);
      if (i >= 0) {
        const r = span(i, v.length);
        return { ...r, ranges: [r], exact: true };
      }
    }
    // Quotes stitched from several sentences: cover them passage by passage, longest first.
    const words = norm(quote).split(" ");
    const find = (a, b) => {
      for (let len = b - a; len >= Math.min(4, b - a); len--)
        for (let s = a; s + len <= b; s++) {
          const piece = words.slice(s, s + len).join(" ");
          if (piece.length < 15) continue;
          const i = out.indexOf(piece);
          if (i >= 0) return { s, len, ...span(i, piece.length) };
        }
      return null;
    };
    let segs = [[0, words.length]], ranges = [], got = 0;
    for (let k = 0; k < 6 && segs.length; k++) {
      const best = segs.map(([a, b]) => find(a, b)).filter(Boolean).sort((x, y) => y.len - x.len)[0];
      if (!best) break;
      ranges.push({ start: best.start, end: best.end });
      got += best.len;
      segs = segs.flatMap(([a, b]) => (best.s >= a && best.s + best.len <= b ? [[a, best.s], [best.s + best.len, b]] : [[a, b]])).filter(([a, b]) => b - a >= 4);
    }
    if (ranges.length) {
      ranges.sort((x, y) => x.start - y.start);
      return { ranges, start: ranges[0].start, end: ranges[0].end, exact: false, words: got, of: words.length };
    }
    return null;
  }

  function note(html) {
    const el = q(".dv-note");
    el.innerHTML = html;
    el.hidden = !html;
  }

  /** Open the viewer at a page and highlight the quote (from an "Open Document" button). */
  async function open({ page, quote, label }) {
    const from = document.querySelector(".tab.is-on")?.dataset.track;
    window.setTrack?.("doc");
    await mount();
    if (from && from !== "doc") {
      backTo = from;
      q(".dv-back").hidden = false;
      q(".dv-back").textContent = `← Back to ${from === "draw" ? "Draw plot" : "Building rules"}`;
    }
    clearMarks();
    // The quote may start on the cited page or spill onto a neighbour.
    let hit = null, at = page;
    for (const n of [page, page + 1, page - 1]) {
      const p = data.pages.find((x) => x.page === n);
      const r = p && quote ? locate(p.text, quote) : null;
      if (r && (!hit || r.exact || (r.words ?? 0) > (hit.words ?? 0))) {
        hit = r;
        at = n;
        if (r.exact) break;
      }
    }
    if (hit) {
      const mark = markRange(at, hit.start, hit.end, hit.ranges);
      requestAnimationFrame(() => goPage(at, mark));
      const parts = hit.ranges.length > 1 ? ` in ${hit.ranges.length} passages` : "";
      const how = hit.exact
        ? "exact text highlighted"
        : hit.words >= hit.of * 0.9
          ? `quoted text highlighted${parts}`
          : `closest match highlighted${parts} (${hit.words} of ${hit.of} words)`;
      note(`<b>${esc(label)}</b> · PDF page ${at} · ${how}`);
    } else {
      requestAnimationFrame(() => goPage(page));
      note(`<b>${esc(label)}</b> · PDF page ${page}${quote ? " · the quoted line could not be matched on this page" : ""}`);
    }
  }

  /** Find in document: step through matches across all pages. */
  function step(dir) {
    const query = q(".dv-q").value.trim();
    if (!query) return;
    if (find.q !== query) {
      const lq = query.toLowerCase();
      const hits = [];
      for (const p of data.pages) {
        const t = p.text.toLowerCase();
        for (let i = t.indexOf(lq); i >= 0; i = t.indexOf(lq, i + lq.length)) hits.push({ page: p.page, start: i, end: i + lq.length });
      }
      find = { q: query, hits, i: -1 };
    }
    const c = q(".dv-count");
    if (!find.hits.length) {
      c.textContent = "no matches";
      return;
    }
    find.i = (find.i + dir + find.hits.length) % find.hits.length;
    const h = find.hits[find.i];
    c.textContent = `${find.i + 1} / ${find.hits.length}`;
    clearMarks();
    note("");
    const mark = markRange(h.page, h.start, h.end);
    goPage(h.page, mark);
  }

  window.docViewer = { mount, open };
})();
