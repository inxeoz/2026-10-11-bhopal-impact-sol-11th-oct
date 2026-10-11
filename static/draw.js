/* Draw plot: draw any plot outline, place and move blocks, and check the layout against the
 * rules — live while you drag, or on demand. The checking is all in Rust (POST /api/check,
 * src/layout.rs); this file is the editor and a 3D preview.
 * Plan coordinates are metres, x to the right, y downwards. */
import * as THREE from "three";
import { OrbitControls } from "three/addons/OrbitControls.js";

const STORE = "bhopal-draw-v1";
const FLOOR_H = 3; // matches FLOOR_HEIGHT in src/layout.rs; stated in the UI
const PLINTH = 0.5; // matches DEFAULT_PLINTH in src/layout.rs
const ROAD_LEVEL = 500; // a starting road level in metres above sea level (Bhopal is ~500 m)

/** Default sizes are starting points the user resizes; the rules check them. */
const KINDS = {
  building: { name: "Building", w: 6, d: 10, floors: 2, fill: "#e7ecf3", stroke: "#1f3a5f" },
  balcony: { name: "Balcony", w: 3, d: 1.2, fill: "#d3d9e1", stroke: "#5b6166" },
  garage: { name: "Garage", w: 2.5, d: 5, fill: "#ece6d9", stroke: "#8a7a64" },
  staircase: { name: "Staircase", w: 1.8, d: 3.6, width: 0.85, fill: "#efdfc4", stroke: "#b8925c" },
  lift: { name: "Lift", w: 2, d: 2, fill: "#c8d3e6", stroke: "#1f3a5f" },
  shaft: { name: "Shaft", w: 0.9, d: 1.4, fill: "#8e98a6", stroke: "#2a3440" },
  courtyard: { name: "Courtyard", w: 3, d: 3, fill: "#dff3f9", stroke: "#3d8fb3" },
  septic_tank: { name: "Septic tank", w: 0.75, d: 1.5, fill: "#dccdb6", stroke: "#8a7a64" },
  parking_bay: { name: "Parking bay", w: 2.5, d: 5, fill: "#f4f4ef", stroke: "#8a9095" },
  tree: { name: "Tree", w: 1.4, d: 1.4, round: true, fill: "#a6d6af", stroke: "#3c7a4a" },
  rwh_pit: { name: "Rainwater pit", w: 1.2, d: 1.2, round: true, fill: "#c4e3f2", stroke: "#3d7fa8" },
  gate: { name: "Gate", w: 3, d: 0.3, fill: "#3a3f44", stroke: "#1b1d1f" },
  water_body: { name: "Lake / river", w: 30, d: 12, water: "lake", fill: "#a8cfea", stroke: "#3d7fa8" },
  electric_line: { name: "Power line", line: true, voltage: "lv", stroke: "#b23b3b" },
};
const ADD = [
  ["building", "Building"], ["balcony", "Balcony"], ["garage", "Garage"], ["staircase", "Stair"], ["lift", "Lift"],
  ["shaft", "Shaft"], ["courtyard", "Courtyard"], ["septic_tank", "Septic"], ["parking_bay", "Parking"], ["tree", "Tree"],
  ["rwh_pit", "RWH pit"], ["gate", "Gate"], ["water_body", "Lake/river"], ["electric_line", "Power line"],
];
const INTERIOR = new Set(["staircase", "lift", "shaft", "courtyard"]);
const ORDER = ["water_body", "building", "courtyard", "shaft", "staircase", "lift", "balcony", "garage", "parking_bay", "septic_tank", "rwh_pit", "tree", "gate", "electric_line"];
const EDGE_COLOR = { front: "#2f6f4f", rear: "#8a5a00", side: "#3d6fa8" };

const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const fmt = (v) => (v == null ? "—" : Number(v).toFixed(2).replace(/\.?0+$/, ""));
const clone = (o) => JSON.parse(JSON.stringify(o));

function template() {
  return {
    plot: [[0, 0], [12, 0], [12, 20], [0, 20]],
    road_edges: [2],
    street_width: 9,
    road_level: ROAD_LEVEL,
    plot_levels: [ROAD_LEVEL, ROAD_LEVEL, ROAD_LEVEL, ROAD_LEVEL],
    use_type: "residential",
    building_type: "detached",
    wall: null,
    items: [{ id: "building-1", kind: "building", name: "Building 1", x: 6, y: 10, w: 6, d: 12, floors: 2 }],
  };
}

let app = null;
window.drawPlot = {
  mount() {
    const root = document.getElementById("draw-root");
    if (!app) app = new DrawApp(root);
    else app.onShow();
  },
  /** Open a layout (e.g. converted from an Ask scene). */
  open(layout) {
    if (!app) app = new DrawApp(document.getElementById("draw-root"));
    app.replace(layout);
  },
};

class DrawApp {
  constructor(root) {
    this.root = root;
    this.layout = this.load() ?? template();
    this.sel = null; // {item: id} | {vertex: i} | {plot: true}
    this.tool = "select";
    this.draft = null; // plot / line points being drawn
    this.live = true;
    this.snap = 0.5;
    this.check = null;
    this.seq = 0;
    this.past = [];
    this.future = [];
    this.vb = null;

    root.innerHTML = `
      <div class="dp">
        <div class="dp-tools">
          <div class="dp-group">
            <button type="button" data-tool="select" title="Select, move, resize (V)">Select</button>
            <button type="button" data-tool="plot" title="Draw a new plot outline: click corners, click the first corner to close (P)">Draw plot</button>
            <button type="button" data-tool="road" title="Click a plot edge to mark it as facing the road (R)">Road edge</button>
          </div>
          <div class="dp-group dp-add">${ADD.map(([k, t]) => `<button type="button" data-tool="${k}" title="Add ${esc(KINDS[k].name)}">+ ${t}</button>`).join("")}</div>
          <div class="dp-group">
            <button type="button" data-act="undo" title="Undo (Ctrl+Z)">Undo</button>
            <button type="button" data-act="redo" title="Redo (Ctrl+Shift+Z)">Redo</button>
            <button type="button" data-act="fit" title="Fit the plot in view">Fit</button>
            <button type="button" data-act="reset" title="Start again from the sample plot">Reset</button>
            <button type="button" data-act="export" title="Download this layout as JSON">Export</button>
            <label class="dp-import" title="Load a layout JSON">Import<input type="file" accept="application/json" hidden></label>
          </div>
        </div>
        <div class="dp-main">
          <div class="dp-canvas">
            <svg class="dp-svg" preserveAspectRatio="xMidYMid meet"></svg>
            <div class="dp-hint"></div>
          </div>
          <aside class="dp-side">
            <div class="dp-check">
              <label class="dp-live"><input type="checkbox" checked> Live check</label>
              <button type="button" class="primary dp-run">Check now</button>
            </div>
            <div class="dp-status"></div>
            <div class="dp-props"></div>
            <div class="dp-issues"></div>
          </aside>
        </div>
        <div class="dp-3d-head"><span class="tag">Section</span><span class="dp-sub">Cut through the selected building, from the road inwards · levels in m above sea level</span></div>
        <div class="dp-section"></div>
        <div class="dp-3d-head"><span class="tag">3D preview</span><span class="dp-sub">Red = breaks a rule · floors drawn ${FLOOR_H} m apart</span></div>
        <div class="dp-3d"></div>
      </div>`;
    this.$ = (s) => root.querySelector(s);
    this.svg = this.$(".dp-svg");

    root.querySelectorAll("[data-tool]").forEach((b) => b.addEventListener("click", () => this.setTool(b.dataset.tool)));
    root.querySelectorAll("[data-act]").forEach((b) => b.addEventListener("click", () => this.act(b.dataset.act)));
    this.$(".dp-import input").addEventListener("change", (e) => this.importFile(e.target.files[0]));
    this.$(".dp-live input").addEventListener("change", (e) => {
      this.live = e.target.checked;
      if (this.live) this.runCheck();
      this.renderStatus();
    });
    this.$(".dp-run").addEventListener("click", () => this.runCheck());

    this.svg.addEventListener("pointerdown", (e) => this.down(e));
    this.svg.addEventListener("pointermove", (e) => this.move(e));
    this.svg.addEventListener("pointerup", (e) => this.up(e));
    this.svg.addEventListener("dblclick", (e) => this.dbl(e));
    this.svg.addEventListener("wheel", (e) => this.wheel(e), { passive: false });
    this.onKey = (e) => this.key(e);
    window.addEventListener("keydown", this.onKey);

    this.preview = new Preview(this.$(".dp-3d"));
    this.fit();
    this.setTool("select");
    this.runCheck();
  }

  onShow() {
    this.preview.fit();
    this.render();
  }

  /* ---------- persistence and history ---------- */
  load() {
    try {
      const v = JSON.parse(localStorage.getItem(STORE));
      return v && Array.isArray(v.plot) ? v : null;
    } catch {
      return null;
    }
  }
  save() {
    try {
      localStorage.setItem(STORE, JSON.stringify(this.layout));
    } catch {}
  }
  /** Record the state before a change, then apply it. */
  commit() {
    this.save();
    this.render();
    this.changed();
  }
  snapshot() {
    this.past.push(JSON.stringify(this.layout));
    if (this.past.length > 100) this.past.shift();
    this.future = [];
  }
  replace(layout) {
    this.snapshot();
    this.layout = layout;
    this.sel = null;
    this.fit();
    this.commit();
  }

  act(a) {
    if (a === "undo" && this.past.length) {
      this.future.push(JSON.stringify(this.layout));
      this.layout = JSON.parse(this.past.pop());
      this.sel = null;
      this.commit();
    } else if (a === "redo" && this.future.length) {
      this.past.push(JSON.stringify(this.layout));
      this.layout = JSON.parse(this.future.pop());
      this.sel = null;
      this.commit();
    } else if (a === "fit") {
      this.fit();
      this.render();
    } else if (a === "reset") {
      this.replace(template());
    } else if (a === "export") {
      const blob = new Blob([JSON.stringify(this.layout, null, 2)], { type: "application/json" });
      const link = document.createElement("a");
      link.href = URL.createObjectURL(blob);
      link.download = "plot-layout.json";
      link.click();
      URL.revokeObjectURL(link.href);
    }
  }

  async importFile(f) {
    if (!f) return;
    try {
      const v = JSON.parse(await f.text());
      if (!Array.isArray(v.plot)) throw new Error("no plot");
      this.replace({ ...template(), ...v });
    } catch (err) {
      alert(`Could not read that file: ${err.message}`);
    }
  }

  /* ---------- checking ---------- */
  changed(dragging = false) {
    if (!this.live) {
      this.stale = true;
      this.renderStatus();
      return;
    }
    // While dragging, check at most every 120 ms; otherwise right away.
    clearTimeout(this.timer);
    if (dragging) {
      const now = performance.now();
      if (!this.last || now - this.last > 120) {
        this.last = now;
        this.runCheck();
      } else this.timer = setTimeout(() => this.runCheck(), 120);
    } else this.runCheck();
  }

  async runCheck() {
    const seq = ++this.seq;
    try {
      const r = await fetch("/api/check", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(this.layout),
      });
      if (!r.ok) throw new Error(await r.text());
      const c = await r.json();
      if (seq !== this.seq) return; // a newer check is on its way
      this.check = c;
      this.stale = false;
    } catch (err) {
      if (seq !== this.seq) return;
      this.check = { error: err.message };
    }
    this.render();
  }

  /* ---------- tools ---------- */
  setTool(t) {
    this.tool = t;
    this.draft = null;
    this.root.querySelectorAll("[data-tool]").forEach((b) => b.classList.toggle("is-on", b.dataset.tool === t));
    const hints = {
      select: "Drag blocks to move them, drag a corner to resize, drag plot corners to reshape. Double-click a plot edge to add a corner. Delete removes the selection. Scroll to zoom, drag empty space to pan.",
      plot: "Click to place each corner of the plot. Click the first corner (or press Enter) to close it. Esc cancels.",
      road: "Click a plot edge to mark it as facing the road. Click again to unmark. Corner plots can have two.",
      electric_line: "Click two points: where the power line starts and where it ends.",
    };
    this.$(".dp-hint").textContent = hints[t] ?? `Click on the plan to place a ${KINDS[t]?.name.toLowerCase() ?? "block"}.`;
    this.svg.dataset.tool = t;
    this.render();
  }

  key(e) {
    if (!this.root.offsetParent) return; // tab hidden
    if (/input|select|textarea/i.test(document.activeElement?.tagName)) return;
    const k = e.key.toLowerCase();
    if ((e.ctrlKey || e.metaKey) && k === "z") {
      e.preventDefault();
      this.act(e.shiftKey ? "redo" : "undo");
    } else if ((e.ctrlKey || e.metaKey) && k === "y") {
      e.preventDefault();
      this.act("redo");
    } else if (k === "delete" || k === "backspace") {
      if (this.sel) {
        e.preventDefault();
        this.remove();
      }
    } else if (k === "escape") {
      this.draft = null;
      this.sel = null;
      this.setTool("select");
    } else if (k === "enter" && this.tool === "plot") {
      this.finishPlot();
    } else if (k === "v") this.setTool("select");
    else if (k === "p") this.setTool("plot");
    else if (k === "r") this.setTool("road");
    else if (k.startsWith("arrow") && this.sel?.item) {
      e.preventDefault();
      const it = this.item(this.sel.item);
      const step = this.snap || 0.1;
      this.snapshot();
      if (it.kind === "electric_line") return;
      if (k === "arrowleft") it.x -= step;
      if (k === "arrowright") it.x += step;
      if (k === "arrowup") it.y -= step;
      if (k === "arrowdown") it.y += step;
      this.commit();
    }
  }

  remove() {
    const L = this.layout;
    this.snapshot();
    if (this.sel?.item) L.items = L.items.filter((i) => i.id !== this.sel.item);
    else if (this.sel?.vertex != null && L.plot.length > 3) {
      const i = this.sel.vertex;
      L.plot.splice(i, 1);
      L.plot_levels?.splice(i, 1);
      L.road_edges = L.road_edges.filter((e) => e !== i).map((e) => (e > i ? e - 1 : e)).filter((e) => e < L.plot.length);
    }
    this.sel = null;
    this.commit();
  }

  item(id) {
    return this.layout.items.find((i) => i.id === id);
  }

  addItem(kind, p, b) {
    const k = KINDS[kind];
    const n = this.layout.items.filter((i) => i.kind === kind).length + 1;
    const id = `${kind}-${Date.now().toString(36)}${n}`;
    const it = { id, kind, name: `${k.name} ${n}` };
    if (k.line) Object.assign(it, { a: p, b, voltage: k.voltage });
    else {
      Object.assign(it, { x: p[0], y: p[1], w: k.w, d: k.d });
      if (kind === "building") it.floors = k.floors;
      if (kind === "staircase") it.width = k.width;
      if (kind === "water_body") it.water = k.water;
    }
    this.snapshot();
    this.layout.items.push(it);
    this.sel = { item: id };
    if (kind !== "tree") this.setTool("select");
    this.commit();
  }

  finishPlot() {
    const pts = this.draft;
    if (!pts || pts.length < 3) return;
    this.snapshot();
    const lv = this.layout.plot_levels ?? [];
    const avg = lv.length ? lv.reduce((a, b) => a + b, 0) / lv.length : this.layout.road_level ?? ROAD_LEVEL;
    this.layout.plot = pts;
    this.layout.plot_levels = pts.map(() => Math.round(avg * 100) / 100);
    // The edge lowest on screen faces the road until the user says otherwise.
    let best = 0, by = -Infinity;
    pts.forEach((p, i) => {
      const q = pts[(i + 1) % pts.length];
      const y = (p[1] + q[1]) / 2;
      if (y > by) (by = y), (best = i);
    });
    this.layout.road_edges = [best];
    this.draft = null;
    this.setTool("select");
    this.sel = { plot: true };
    this.commit();
  }

  /* ---------- pointer ---------- */
  world(e, m = this.svg.getScreenCTM().inverse()) {
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(m);
    return [p.x, p.y];
  }
  sn(v) {
    return this.snap ? Math.round(v / this.snap) * this.snap : Math.round(v * 100) / 100;
  }
  snp(p) {
    return [this.sn(p[0]), this.sn(p[1])];
  }
  px() {
    // metres per screen pixel
    const r = this.svg.getBoundingClientRect();
    return Math.max(this.vb.w / r.width, this.vb.h / r.height);
  }

  down(e) {
    const p = this.world(e);
    const t = e.target.closest("[data-item],[data-handle],[data-vertex],[data-edge],[data-end]");
    const tool = this.tool;
    if (tool === "plot") {
      const sp = this.snp(p);
      if (!this.draft) this.draft = [sp];
      else {
        const f = this.draft[0];
        if (this.draft.length >= 3 && Math.hypot(sp[0] - f[0], sp[1] - f[1]) < 10 * this.px()) return this.finishPlot();
        this.draft.push(sp);
      }
      return this.render();
    }
    if (tool === "road") {
      const i = this.nearestEdge(p);
      if (i != null) {
        this.snapshot();
        const r = this.layout.road_edges;
        this.layout.road_edges = r.includes(i) ? r.filter((x) => x !== i) : [...r, i];
        this.commit();
      }
      return;
    }
    if (tool === "electric_line") {
      const sp = this.snp(p);
      if (!this.draft) this.draft = [sp];
      else this.addItem("electric_line", this.draft[0], sp), (this.draft = null);
      return this.render();
    }
    if (KINDS[tool]) return this.addItem(tool, this.snp(p));

    // select tool
    try {
      this.svg.setPointerCapture(e.pointerId);
    } catch {}
    this.moved = false;
    if (t?.dataset.vertex != null) {
      this.sel = { vertex: +t.dataset.vertex };
      this.drag = { type: "vertex", i: +t.dataset.vertex };
    } else if (t?.dataset.handle) {
      const it = this.item(t.dataset.handle);
      const c = t.dataset.corner;
      const fx = c.includes("w") ? it.x + it.w / 2 : it.x - it.w / 2;
      const fy = c.includes("n") ? it.y + it.d / 2 : it.y - it.d / 2;
      this.drag = { type: "resize", id: it.id, fx, fy };
    } else if (t?.dataset.end) {
      this.drag = { type: "end", id: t.dataset.line, end: t.dataset.end };
    } else if (t?.dataset.item) {
      const it = this.item(t.dataset.item);
      this.sel = { item: it.id };
      this.drag = it.kind === "electric_line"
        ? { type: "line", id: it.id, p0: p, a0: [...it.a], b0: [...it.b] }
        : { type: "move", id: it.id, dx: p[0] - it.x, dy: p[1] - it.y };
    } else if (t?.dataset.edge != null || e.target.closest(".dp-plot")) {
      this.sel = { plot: true };
      this.drag = { type: "pan", p0: p, vb0: { ...this.vb }, m: this.svg.getScreenCTM().inverse() };
    } else {
      this.drag = { type: "pan", p0: p, vb0: { ...this.vb }, m: this.svg.getScreenCTM().inverse(), deselect: true };
    }
    this.before = JSON.stringify(this.layout);
    this.render();
  }

  move(e) {
    if (this.draft && (this.tool === "plot" || this.tool === "electric_line")) {
      this.cursor = this.snp(this.world(e));
      return this.renderSVG();
    }
    const d = this.drag;
    if (!d) return;
    const p = d.type === "pan" ? this.world(e, d.m) : this.world(e);
    this.moved = true;
    const L = this.layout;
    if (d.type === "pan") {
      this.vb = { ...d.vb0, x: d.vb0.x - (p[0] - d.p0[0]), y: d.vb0.y - (p[1] - d.p0[1]) };
      return this.renderSVG();
    }
    if (d.type === "vertex") L.plot[d.i] = this.snp(p);
    else if (d.type === "move") {
      const it = this.item(d.id);
      it.x = this.sn(p[0] - d.dx);
      it.y = this.sn(p[1] - d.dy);
    } else if (d.type === "resize") {
      const it = this.item(d.id);
      const q = this.snp(p);
      const w = Math.max(0.3, Math.abs(q[0] - d.fx)), h = Math.max(0.3, Math.abs(q[1] - d.fy));
      it.w = Math.round(w * 100) / 100;
      it.d = Math.round(h * 100) / 100;
      it.x = d.fx + (q[0] >= d.fx ? w / 2 : -w / 2);
      it.y = d.fy + (q[1] >= d.fy ? h / 2 : -h / 2);
    } else if (d.type === "end") {
      this.item(d.id)[d.end] = this.snp(p);
    } else if (d.type === "line") {
      const it = this.item(d.id);
      const dx = this.sn(p[0] - d.p0[0]), dy = this.sn(p[1] - d.p0[1]);
      it.a = [d.a0[0] + dx, d.a0[1] + dy];
      it.b = [d.b0[0] + dx, d.b0[1] + dy];
    }
    this.renderSVG();
    this.changed(true); // the preview refreshes when the check comes back
  }

  up(e) {
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    try {
      this.svg.releasePointerCapture(e.pointerId);
    } catch {}
    if (d.type === "pan") {
      if (!this.moved && d.deselect) {
        this.sel = null;
        this.render();
      }
      return;
    }
    if (this.moved && this.before !== JSON.stringify(this.layout)) {
      this.past.push(this.before);
      this.future = [];
      this.commit();
    } else this.render();
  }

  dbl(e) {
    if (this.tool === "plot") return this.finishPlot();
    if (this.tool !== "select") return;
    const p = this.world(e);
    const i = this.nearestEdge(p);
    if (i == null) return;
    this.snapshot();
    const L = this.layout;
    L.plot.splice(i + 1, 0, this.snp(p));
    const lv = L.plot_levels ?? [];
    if (lv.length) lv.splice(i + 1, 0, Math.round(((lv[i] + lv[(i + 1) % lv.length]) / 2) * 100) / 100);
    // Edges after the new corner shift by one; the split road edge keeps both halves.
    L.road_edges = L.road_edges.flatMap((r) => (r === i ? [i, i + 1] : r > i ? [r + 1] : [r]));
    this.sel = { vertex: i + 1 };
    this.commit();
  }

  wheel(e) {
    e.preventDefault();
    const p = this.world(e);
    const f = Math.exp(e.deltaY * 0.0015);
    const vb = this.vb;
    this.vb = { x: p[0] - (p[0] - vb.x) * f, y: p[1] - (p[1] - vb.y) * f, w: vb.w * f, h: vb.h * f };
    this.renderSVG();
  }

  nearestEdge(p) {
    const P = this.layout.plot, n = P.length;
    let best = null, bd = 12 * this.px();
    for (let i = 0; i < n; i++) {
      const dd = segDist(p, P[i], P[(i + 1) % n]);
      if (dd < bd) (bd = dd), (best = i);
    }
    return best;
  }

  fit() {
    const L = this.layout;
    const pts = [...L.plot];
    for (const i of L.road_edges) {
      const [a, b] = roadQuad(L.plot, i, L.street_width);
      pts.push(a, b);
    }
    for (const it of L.items) {
      if (it.a) pts.push(it.a, it.b);
      else pts.push([it.x - it.w / 2, it.y - it.d / 2], [it.x + it.w / 2, it.y + it.d / 2]);
    }
    const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const pad = Math.max(x1 - x0, y1 - y0) * 0.12 + 2;
    this.vb = { x: x0 - pad, y: y0 - pad, w: x1 - x0 + 2 * pad, h: y1 - y0 + 2 * pad };
  }

  /** Older layouts and edits that add or drop corners: one level per corner. */
  syncLevels() {
    const L = this.layout;
    if (typeof L.road_level !== "number") L.road_level = ROAD_LEVEL;
    if (!Array.isArray(L.plot_levels)) L.plot_levels = [];
    const avg = L.plot_levels.length ? L.plot_levels.reduce((a, b) => a + b, 0) / L.plot_levels.length : L.road_level;
    while (L.plot_levels.length < L.plot.length) L.plot_levels.push(avg);
    L.plot_levels.length = L.plot.length;
  }

  /* ---------- rendering ---------- */
  render() {
    this.syncLevels();
    this.renderSVG();
    this.renderStatus();
    this.renderProps();
    this.renderIssues();
    this.renderSection();
    this.preview.update(this.layout, this.check);
  }

  renderSVG() {
    const L = this.layout, vb = this.vb, c = this.check && !this.check.error ? this.check : null;
    const k = this.px(); // metres per pixel
    const fs = 12 * k; // 12 px text
    const bad = new Set(c?.bad ?? []);
    const P = L.plot, n = P.length;
    let s = "";

    // Text halos are in plan units (metres), so size them per pixel.
    s += `<style>.dp-svg text{stroke-width:${3 * k}px}</style>`;

    // grid over everything visible (the canvas is usually wider than the view box)
    const r = this.svg.getBoundingClientRect();
    const vw = r.width * k, vh = r.height * k;
    const gx = vb.x + vb.w / 2 - vw / 2, gy = vb.y + vb.h / 2 - vh / 2;
    const step = vw > 120 ? 10 : vw > 40 ? 5 : 1;
    let grid = "";
    for (let x = Math.floor(gx / step) * step; x < gx + vw; x += step)
      grid += `<line x1="${x}" y1="${gy}" x2="${x}" y2="${gy + vh}" class="${Math.round(x) % (step * 5) === 0 ? "g2" : "g1"}"/>`;
    for (let y = Math.floor(gy / step) * step; y < gy + vh; y += step)
      grid += `<line x1="${gx}" y1="${y}" x2="${gx + vw}" y2="${y}" class="${Math.round(y) % (step * 5) === 0 ? "g2" : "g1"}"/>`;
    s += `<g class="dp-grid">${grid}</g>`;

    const items = [...L.items].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind));
    for (const it of items.filter((i) => i.kind === "water_body")) s += this.svgItem(it, bad, fs);

    // road bands
    for (const i of L.road_edges.filter((i) => i < n)) {
      const [a2, b2] = roadQuad(P, i, L.street_width);
      const a = P[i], b = P[(i + 1) % n];
      s += `<polygon points="${[a, b, b2, a2].map((p) => p.join(",")).join(" ")}" class="dp-road"/>`;
      const m = [(a2[0] + b2[0] + a[0] + b[0]) / 4, (a2[1] + b2[1] + a[1] + b[1]) / 4];
      s += `<text x="${m[0]}" y="${m[1]}" class="dp-road-t" font-size="${fs}">${fmt(L.street_width)} m road · ${fmt(L.road_level)} m</text>`;
    }

    // plot
    s += `<polygon points="${P.map((p) => p.join(",")).join(" ")}" class="dp-plot ${this.sel?.plot ? "is-sel" : ""}"/>`;
    if (c?.buildable?.length) s += `<polygon points="${c.buildable.map((p) => p.join(",")).join(" ")}" class="dp-buildable"/>`;
    // contours of the fitted ground plane, clipped to the plot
    if (c?.sloping) s += contours(P, c.plane, fs);
    // open-space lines
    for (const e of c?.edges ?? []) {
      if (e.required <= 0) continue;
      const [a, b] = e.line;
      s += `<line x1="${a[0]}" y1="${a[1]}" x2="${b[0]}" y2="${b[1]}" class="dp-setback" stroke="${EDGE_COLOR[e.class]}"/>`;
    }
    // edge labels: class, required distance, length
    for (let i = 0; i < n; i++) {
      const a = P[i], b = P[(i + 1) % n];
      const e = c?.edges?.[i];
      const m = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
      const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
      const ang = (Math.atan2(b[1] - a[1], b[0] - a[0]) * 180) / Math.PI;
      const up = ang > 90 || ang < -90 ? ang + 180 : ang;
      const txt = e ? `${e.class}${e.required ? ` ≥ ${fmt(e.required)} m` : ""} · ${fmt(len)} m` : `${fmt(len)} m`;
      s += `<line x1="${a[0]}" y1="${a[1]}" x2="${b[0]}" y2="${b[1]}" class="dp-edge ${L.road_edges.includes(i) ? "is-road" : ""}" data-edge="${i}"/>`;
      s += `<text x="${m[0]}" y="${m[1]}" transform="rotate(${up} ${m[0]} ${m[1]}) translate(0 ${-0.6 * fs})" class="dp-edge-t" font-size="${fs * 0.95}" fill="${e ? EDGE_COLOR[e.class] : "#5b6166"}">${txt}</text>`;
    }

    for (const it of items.filter((i) => i.kind !== "water_body")) s += this.svgItem(it, bad, fs);

    // corner levels (m above sea level)
    P.forEach((p, i) => {
      const lv = L.plot_levels?.[i];
      if (lv == null) return;
      s += `<text x="${p[0]}" y="${p[1]}" dx="${9 * k}" dy="${-7 * k}" class="dp-level-t" font-size="${fs * 0.9}">▲ ${fmt(lv)}</text>`;
    });
    // plot corners
    if (this.tool === "select")
      P.forEach((p, i) => {
        s += `<circle cx="${p[0]}" cy="${p[1]}" r="${6 * k}" class="dp-vertex ${this.sel?.vertex === i ? "is-sel" : ""}" data-vertex="${i}"/>`;
      });

    // drafts
    if (this.draft) {
      const pts = [...this.draft, ...(this.cursor ? [this.cursor] : [])];
      s += `<polyline points="${pts.map((p) => p.join(",")).join(" ")}" class="dp-draft"/>`;
      this.draft.forEach((p, i) => (s += `<circle cx="${p[0]}" cy="${p[1]}" r="${(i ? 4 : 7) * k}" class="dp-draft-pt"/>`));
    }

    this.svg.setAttribute("viewBox", `${vb.x} ${vb.y} ${vb.w} ${vb.h}`);
    this.svg.innerHTML = s;
  }

  svgItem(it, bad, fs) {
    const K = KINDS[it.kind] ?? {};
    const sel = this.sel?.item === it.id;
    const cls = `dp-item ${bad.has(it.id) ? "is-bad" : ""} ${sel ? "is-sel" : ""}`;
    const k = this.px();
    if (K.line) {
      let s = `<line x1="${it.a[0]}" y1="${it.a[1]}" x2="${it.b[0]}" y2="${it.b[1]}" class="dp-line-hit" data-item="${it.id}"/>`;
      s += `<line x1="${it.a[0]}" y1="${it.a[1]}" x2="${it.b[0]}" y2="${it.b[1]}" class="dp-line ${cls}" data-item="${it.id}"/>`;
      const m = [(it.a[0] + it.b[0]) / 2, (it.a[1] + it.b[1]) / 2];
      s += `<text x="${m[0]}" y="${m[1]}" class="dp-item-t" font-size="${fs}" dy="${-0.5 * fs}">${esc(it.name)} · ${esc({ lv: "LV", hv: "≤33 kV", ehv: ">33 kV" }[it.voltage] ?? "")}</text>`;
      if (sel) for (const end of ["a", "b"]) s += `<circle cx="${it[end][0]}" cy="${it[end][1]}" r="${6 * k}" class="dp-handle" data-end="${end}" data-line="${it.id}"/>`;
      return s;
    }
    const x = it.x - it.w / 2, y = it.y - it.d / 2;
    let s = K.round
      ? `<ellipse cx="${it.x}" cy="${it.y}" rx="${it.w / 2}" ry="${it.d / 2}" fill="${K.fill}" stroke="${K.stroke}" class="${cls}" data-item="${it.id}"/>`
      : `<rect x="${x}" y="${y}" width="${it.w}" height="${it.d}" fill="${K.fill}" stroke="${K.stroke}" class="${cls} dp-k-${it.kind}" data-item="${it.id}"/>`;
    const big = Math.min(it.w, it.d) / k > 34;
    if (it.kind === "building" || (big && it.kind !== "tree"))
      s += `<text x="${it.x}" y="${it.y}" class="dp-item-t" font-size="${fs}" dy="${0.35 * fs}">${esc(it.name)}${it.kind === "building" ? ` · ${it.floors || 1}F${(() => {
        const lv = this.check?.levels?.find((x) => x.id === it.id);
        return lv ? ` · ${fmt(lv.rule_height)} m` : "";
      })()}` : ""}</text>`;
    if (sel) {
      s += `<text x="${it.x}" y="${y - 0.5 * fs}" class="dp-dim-t" font-size="${fs}">${fmt(it.w)} × ${fmt(it.d)} m</text>`;
      if (!K.round)
        for (const [cx, cy, c] of [[x, y, "nw"], [x + it.w, y, "ne"], [x + it.w, y + it.d, "se"], [x, y + it.d, "sw"]])
          s += `<rect x="${cx - 5 * k}" y="${cy - 5 * k}" width="${10 * k}" height="${10 * k}" class="dp-handle" data-handle="${it.id}" data-corner="${c}"/>`;
    }
    return s;
  }

  /** Elevation drawing: road, ground line, plinth, floors, reference level, rule height. */
  renderSection() {
    const box = this.$(".dp-section");
    const L = this.layout, c = this.check;
    const b = (this.sel?.item && this.item(this.sel.item)?.kind === "building" && this.item(this.sel.item)) || L.items.find((i) => i.kind === "building");
    if (!c || c.error || !c.plane || !b || !L.road_edges.length) {
      box.innerHTML = '<p class="dp-small" style="padding:10px 12px">Add a building and a road edge to see the section.</p>';
      return;
    }
    const P = L.plot, n = P.length;
    const [ga, gb, gc] = c.plane;
    const gz = (p) => ga * p[0] + gb * p[1] + gc;
    // Axis u points from the road into the plot.
    const ri = L.road_edges[0];
    const A = P[ri], B = P[(ri + 1) % n];
    const sg = signedArea(P) > 0 ? 1 : -1;
    const len = Math.hypot(B[0] - A[0], B[1] - A[1]) || 1;
    const u = [(-sg * (B[1] - A[1])) / len, (sg * (B[0] - A[0])) / len];
    const ctr = [b.x, b.y];
    const proj = (p) => (p[0] - ctr[0]) * u[0] + (p[1] - ctr[1]) * u[1];
    const front = proj([(A[0] + B[0]) / 2, (A[1] + B[1]) / 2]);
    const rear = Math.max(...P.map(proj));
    const at = (sv) => gz([ctr[0] + u[0] * sv, ctr[1] + u[1] * sv]);
    const half = (Math.abs(u[0]) * b.w + Math.abs(u[1]) * b.d) / 2;
    const lv = c.levels.find((x) => x.id === b.id);
    if (!lv) return (box.innerHTML = "");
    const road = c.road_level, ref = c.reference_level;
    const s0 = front - L.street_width, s1 = rear + 2;
    const zs = [road, ref, lv.roof, at(front), at(rear), lv.ground - 1];
    const z0 = Math.min(...zs) - 1, z1 = Math.max(...zs) + 2;
    const W = 1100, H = 300, pad = 46;
    const k = Math.min((W - 2 * pad) / (s1 - s0), (H - 2 * pad) / (z1 - z0));
    const X = (sv) => pad + (sv - s0) * k, Y = (z) => H - pad - (z - z0) * k;
    const bx0 = -half, bx1 = half;
    const stH = b.stilt ? 2.6 : 0;
    let g = "";
    // ground: road then the plot profile
    g += `<polygon points="${X(s0)},${Y(road)} ${X(front)},${Y(road)} ${X(front)},${Y(at(front))} ${X(rear)},${Y(at(rear))} ${X(rear)},${Y(z0)} ${X(s0)},${Y(z0)}" class="ds-earth"/>`;
    g += `<line x1="${X(s0)}" y1="${Y(road)}" x2="${X(front)}" y2="${Y(road)}" class="ds-road"/>`;
    g += `<text x="${X(s0) + 4}" y="${Y(road) + 18}" class="ds-t">road ${fmt(road)}</text>`;
    // plinth, stilt, floors
    g += `<rect x="${X(bx0)}" y="${Y(lv.floor)}" width="${(bx1 - bx0) * k}" height="${(lv.floor - Math.min(at(bx0), at(bx1))) * k}" class="ds-plinth"/>`;
    const floors = Math.max(1, b.floors || 1);
    if (stH) g += `<rect x="${X(bx0)}" y="${Y(lv.floor + stH)}" width="${(bx1 - bx0) * k}" height="${stH * k}" class="ds-stilt"/>`;
    for (let f = 0; f < floors; f++) {
      const zb = lv.floor + stH + f * FLOOR_H;
      g += `<rect x="${X(bx0)}" y="${Y(zb + FLOOR_H)}" width="${(bx1 - bx0) * k}" height="${FLOOR_H * k}" class="ds-floor ${c.bad?.includes(b.id) ? "bad" : ""}"/>`;
    }
    if (b.basement) g += `<rect x="${X(bx0)}" y="${Y(lv.floor)}" width="${(bx1 - bx0) * k}" height="${2.7 * k}" class="ds-basement"/>`;
    // reference level and the height that counts
    g += `<line x1="${X(s0)}" y1="${Y(ref)}" x2="${X(s1)}" y2="${Y(ref)}" class="ds-ref"/>`;
    g += `<text x="${X(s1) - 4}" y="${Y(ref) + 16}" class="ds-t ds-ref-t" text-anchor="end">reference ${fmt(ref)} (Rule 2(34))</text>`;
    const dx = X(bx1) + 22;
    g += `<line x1="${dx}" y1="${Y(ref)}" x2="${dx}" y2="${Y(lv.roof)}" class="ds-dim"/>`;
    g += `<line x1="${dx - 5}" y1="${Y(lv.roof)}" x2="${dx + 5}" y2="${Y(lv.roof)}" class="ds-dim"/><line x1="${dx - 5}" y1="${Y(ref)}" x2="${dx + 5}" y2="${Y(ref)}" class="ds-dim"/>`;
    g += `<text x="${dx + 8}" y="${(Y(ref) + Y(lv.roof)) / 2}" class="ds-t ds-h">${fmt(lv.rule_height)} m for the rules${stH ? " (stilt excluded)" : ""}</text>`;
    g += `<text x="${X(bx0) - 6}" y="${Y(lv.floor) + 4}" class="ds-t" text-anchor="end">floor ${fmt(lv.floor)}</text>`;
    g += `<text x="${X(bx0) - 6}" y="${Y(lv.roof) + 4}" class="ds-t" text-anchor="end">roof ${fmt(lv.roof)}</text>`;
    g += `<text x="${X(front)}" y="${H - 14}" class="ds-t" text-anchor="middle">plot edge</text>`;
    box.innerHTML = `<svg viewBox="0 0 ${W} ${H}" class="ds-svg">${g}</svg>`;
  }

  renderStatus() {
    const c = this.check;
    const box = this.$(".dp-status");
    if (!c) return (box.innerHTML = '<p class="spinner">Checking…</p>');
    if (c.error) return (box.innerHTML = `<p class="s3-warn">Could not check: ${esc(c.error)}</p>`);
    const meter = (m) =>
      m && m.limit != null
        ? `<div class="s3-far ${m.ok ? "" : "over"}"><div class="s3-far-head"><span>${esc(m.label)}</span><b>${fmt(m.used)} / ${fmt(m.limit)}</b></div>
           <span class="s3-far-bar"><span style="width:${Math.min(100, (m.used / Math.max(m.limit, 0.01)) * 100)}%"></span></span></div>`
        : m
          ? `<div class="s3-small">${esc(m.label)}: <b>${fmt(m.used)}</b></div>`
          : "";
    box.innerHTML = `
      <div class="dp-badge ${c.ok ? "ok" : "no"}">${c.ok ? "Within these rules" : `${c.errors} rule${c.errors === 1 ? "" : "s"} broken`}${this.stale ? " · changed since last check" : ""}</div>
      <div class="s3-small">${esc(c.envelope)}</div>
      <div class="s3-small">Plot ${fmt(c.plot_area)} sq m · road frontage ${fmt(c.frontage)} m · tallest ${fmt(c.max_height)} m for the rules</div>
      <div class="s3-small">Road ${fmt(c.road_level)} m · reference level <b>${fmt(c.reference_level)} m</b> · ${c.sloping ? `sloping, about 1:${c.slope ? Math.round(1 / c.slope) : "—"}` : "level site"}</div>
      ${meter(c.far)}${meter(c.coverage)}
      ${c.assumed?.length ? `<p class="s3-assumed"><b>Assumed:</b> ${c.assumed.map(esc).join(" · ")}</p>` : ""}`;
  }

  renderIssues() {
    const c = this.check;
    const box = this.$(".dp-issues");
    if (!c || c.error) return (box.innerHTML = "");
    const row = (i) => `
      <li class="dp-issue ${i.level}" ${i.item ? `data-goto="${esc(i.item)}"` : ""}>
        <span class="dp-ico">${i.level === "error" ? "✗" : "i"}</span>
        <div><p>${esc(i.text)}</p>${i.source ? `<details><summary>${esc(i.source.label)} · ${esc(i.source.cite)}</summary><pre class="quote">${esc(i.source.quote)}</pre></details>` : ""}</div>
      </li>`;
    const errs = c.issues.filter((i) => i.level === "error"), infos = c.issues.filter((i) => i.level !== "error");
    const reqs = (c.edges ?? []).filter((e) => e.source);
    const seen = new Set();
    const req = reqs
      .filter((e) => !seen.has(e.label) && seen.add(e.label))
      .map((e) => `<details class="dp-req"><summary><i style="background:${EDGE_COLOR[e.class]}"></i>${esc(e.label)}</summary><pre class="quote">${esc(e.source.quote)}</pre></details>`)
      .join("");
    box.innerHTML = `
      ${errs.length ? `<h4>Breaks a rule</h4><ul>${errs.map(row).join("")}</ul>` : '<p class="dp-allgood">No rule broken by this layout.</p>'}
      ${infos.length ? `<h4>Notes</h4><ul>${infos.map(row).join("")}</ul>` : ""}
      ${req ? `<h4>Open spaces required</h4>${req}` : ""}`;
    box.querySelectorAll("[data-goto]").forEach((li) =>
      li.addEventListener("click", (e) => {
        if (e.target.closest("details")) return;
        this.sel = { item: li.dataset.goto };
        this.render();
      })
    );
  }

  renderProps() {
    const box = this.$(".dp-props");
    const L = this.layout;
    const num = (key, label, v, step = 0.1, min = 0) =>
      `<label class="s3-row"><span class="s3-lbl">${label}</span><input type="number" data-k="${key}" value="${fmt(v)}" step="${step}" min="${min}"></label>`;
    let html = "";
    const it = this.sel?.item && this.item(this.sel.item);
    if (it) {
      const K = KINDS[it.kind];
      html = `<h4>${esc(K.name)}</h4>
        <label class="s3-row"><span class="s3-lbl">Name</span><input type="text" data-k="name" value="${esc(it.name)}"></label>`;
      if (K.line) {
        html += num("a0", "Start x", it.a[0]) + num("a1", "Start y", it.a[1]) + num("b0", "End x", it.b[0]) + num("b1", "End y", it.b[1]);
        html += `<label class="s3-row"><span class="s3-lbl">Voltage</span><select data-k="voltage">
          ${[["lv", "Low / medium (LT)"], ["hv", "High, up to 33 kV"], ["ehv", "Extra high, above 33 kV"]].map(([v, t]) => `<option value="${v}" ${it.voltage === v ? "selected" : ""}>${t}</option>`).join("")}</select></label>`;
      } else {
        html += num("x", "Centre x", it.x) + num("y", "Centre y", it.y) + num("w", "Width", it.w, 0.1, 0.1) + num("d", "Depth", it.d, 0.1, 0.1);
        if (it.kind === "building") {
          html += num("floors", "Floors", it.floors || 1, 1, 1);
          html += num("plinth", "Plinth", it.plinth ?? PLINTH, 0.05, 0);
          html += `<label class="s3-row"><span class="s3-lbl">Basement</span><input type="checkbox" data-k="basement" ${it.basement ? "checked" : ""}></label>`;
          const lv = this.check?.levels?.find((x) => x.id === it.id);
          html += lv
            ? `<p class="dp-small">Ground ${fmt(lv.ground)} m → floor ${fmt(lv.floor)} m → roof ${fmt(lv.roof)} m. Height for the rules: <b>${fmt(lv.rule_height)} m</b> above the reference level (${FLOOR_H} m per floor).</p>`
            : `<p class="dp-small">${FLOOR_H} m per floor.</p>`;
          html += `<label class="s3-row"><span class="s3-lbl">Stilt parking</span><input type="checkbox" data-k="stilt" ${it.stilt ? "checked" : ""}></label>`;
        }
        if (it.kind === "staircase") html += num("width", "Flight width", it.width ?? 0.85, 0.05, 0.3);
        if (it.kind === "water_body")
          html += `<label class="s3-row"><span class="s3-lbl">Type</span><select data-k="water">
            <option value="lake" ${it.water !== "river" ? "selected" : ""}>Lake / pond / nala</option><option value="river" ${it.water === "river" ? "selected" : ""}>River</option></select></label>`;
      }
      html += `<button type="button" class="dp-del">Delete ${esc(K.name.toLowerCase())}</button>`;
    } else if (this.sel?.vertex != null) {
      const p = L.plot[this.sel.vertex];
      html = `<h4>Plot corner ${this.sel.vertex + 1}</h4>${num("vx", "x", p[0])}${num("vy", "y", p[1])}
        ${num("vlevel", "Ground level", L.plot_levels?.[this.sel.vertex] ?? L.road_level, 0.1)}
        <p class="dp-small">Metres above sea level. Give corners different levels for a sloping site.</p>
        ${L.plot.length > 3 ? '<button type="button" class="dp-del">Delete corner</button>' : ""}`;
    } else {
      const uses = [["residential", "Residential"], ["business", "Shop / office"], ["educational", "School / college"], ["institutional", "Hospital"], ["assembly", "Hall / cinema"], ["industrial", "Industrial"], ["farm_house", "Farm house"]];
      html = `<h4>Plot</h4>
        ${num("street_width", "Road width", L.street_width, 0.5, 1)}
        ${num("road_level", "Road level", L.road_level, 0.1)}
        ${num("plot_level_all", "Plot level", (L.plot_levels ?? []).reduce((a, b) => a + b, 0) / Math.max(1, (L.plot_levels ?? []).length), 0.1)}
        <p class="dp-small">Levels in metres above sea level. "Plot level" sets every corner; select a corner to slope the plot.</p>
        <label class="s3-row"><span class="s3-lbl">Use</span><select data-k="use_type">${uses.map(([v, t]) => `<option value="${v}" ${L.use_type === v ? "selected" : ""}>${t}</option>`).join("")}</select></label>
        <label class="s3-row" ${L.use_type === "residential" ? "" : "hidden"}><span class="s3-lbl">Type</span><select data-k="building_type">
          ${[["detached", "Detached"], ["semi-detached", "Semi-detached"], ["row", "Row"]].map(([v, t]) => `<option value="${v}" ${L.building_type === v ? "selected" : ""}>${t}</option>`).join("")}</select></label>
        <label class="s3-row"><span class="s3-lbl">Boundary wall</span><input type="checkbox" data-k="wall" ${L.wall ? "checked" : ""}></label>
        ${L.wall ? num("wall_front", "Front wall h", L.wall.front_h, 0.1, 0) + num("wall_other", "Side/rear h", L.wall.other_h, 0.1, 0) : ""}
        <label class="s3-row"><span class="s3-lbl">Snap</span><select data-k="snap">
          ${[[0, "Off"], [0.1, "0.1 m"], [0.5, "0.5 m"], [1, "1 m"]].map(([v, t]) => `<option value="${v}" ${this.snap === v ? "selected" : ""}>${t}</option>`).join("")}</select></label>
        <p class="dp-small">Select a block to edit it, or a plot corner to move it exactly.</p>`;
    }
    box.innerHTML = html;
    box.querySelector(".dp-del")?.addEventListener("click", () => this.remove());
    box.querySelectorAll("[data-k]").forEach((inp) =>
      inp.addEventListener("change", () => {
        const k = inp.dataset.k;
        const v = inp.type === "checkbox" ? inp.checked : inp.type === "number" ? Number(inp.value) : inp.value;
        if (k === "snap") return (this.snap = Number(v));
        this.snapshot();
        if (it) {
          if (["a0", "a1", "b0", "b1"].includes(k)) it[k[0]][+k[1]] = v;
          else it[k] = v;
        } else if (this.sel?.vertex != null) {
          if (k === "vlevel") L.plot_levels[this.sel.vertex] = v;
          else L.plot[this.sel.vertex][k === "vx" ? 0 : 1] = v;
        } else if (k === "plot_level_all") L.plot_levels = L.plot.map(() => v);
        else if (k === "wall") L.wall = v ? { front_h: 1.5, other_h: 2 } : null;
        else if (k === "wall_front") L.wall.front_h = v;
        else if (k === "wall_other") L.wall.other_h = v;
        else L[k] = v;
        this.commit();
      })
    );
  }
}

/* ---------- 3D preview ---------- */
class Preview {
  constructor(host) {
    this.host = host;
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    host.appendChild(this.renderer.domElement);
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0xf6f6f3);
    this.camera = new THREE.PerspectiveCamera(40, 1, 0.1, 2000);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.maxPolarAngle = Math.PI / 2 - 0.03;
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0xb9bcb3, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.8);
    sun.position.set(-40, 70, 45);
    sun.castShadow = true;
    sun.shadow.mapSize.set(2048, 2048);
    Object.assign(sun.shadow.camera, { left: -80, right: 80, top: 80, bottom: -80, far: 300 });
    this.scene.add(sun);
    const ground = new THREE.Mesh(new THREE.PlaneGeometry(1000, 1000), new THREE.MeshStandardMaterial({ color: 0xeceee8 }));
    ground.rotation.x = -Math.PI / 2;
    ground.position.y = -0.01;
    ground.receiveShadow = true;
    this.scene.add(ground);
    this.group = new THREE.Group();
    this.scene.add(this.group);
    this.framed = false;
    new ResizeObserver(() => this.fit()).observe(host);
    const loop = () => {
      requestAnimationFrame(loop);
      if (!this.host.offsetParent) return;
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    loop();
  }

  fit() {
    const w = this.host.clientWidth, h = this.host.clientHeight;
    if (!w || !h) return;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  update(L, check) {
    let g = this.group;
    g.traverse((o) => {
      o.geometry?.dispose();
      o.material?.dispose?.();
    });
    g.clear();
    const bad = new Set(check?.bad ?? []);
    const P = L.plot;
    if (P.length < 3) return;
    const cx = P.reduce((s, p) => s + p[0], 0) / P.length, cz = P.reduce((s, p) => s + p[1], 0) / P.length;
    g.position.set(-cx, 0, -cz);
    // Heights are drawn relative to the road level (y = 0 is the road).
    const road = L.road_level ?? 0;
    const [pa, pb, pc] = check?.plane ?? [0, 0, (L.plot_levels?.[0] ?? road)];
    const gy = (x, z) => pa * x + pb * z + pc - road; // ground height at plan point (x, z)
    const levelOf = (id) => check?.levels?.find((x) => x.id === id);
    const mat = (color, extra = {}) => new THREE.MeshStandardMaterial({ color, roughness: 0.85, ...extra });
    const flat = (pts, y, color, extra) => {
      const shape = new THREE.Shape(pts.map((p) => new THREE.Vector2(p[0], -p[1])));
      const m = new THREE.Mesh(new THREE.ShapeGeometry(shape), mat(color, extra));
      m.rotation.x = -Math.PI / 2;
      m.position.y = y;
      m.receiveShadow = true;
      return m;
    };
    // A surface that follows the ground plane, lifted by dy.
    const terrain = (pts, dy, color, extra) => {
      const geo = new THREE.ShapeGeometry(new THREE.Shape(pts.map((p) => new THREE.Vector2(p[0], p[1]))));
      const pos = geo.attributes.position;
      for (let i = 0; i < pos.count; i++) {
        const x = pos.getX(i), z = pos.getY(i);
        pos.setXYZ(i, x, gy(x, z) + dy, z);
      }
      geo.computeVertexNormals();
      const m = new THREE.Mesh(geo, mat(color, { side: THREE.DoubleSide, ...extra }));
      m.receiveShadow = true;
      return m;
    };
    const box = (w, h, d, color, x, y, z, extra) => {
      const m = new THREE.Mesh(new THREE.BoxGeometry(Math.max(w, 0.02), Math.max(h, 0.02), Math.max(d, 0.02)), mat(color, extra));
      m.position.set(x, y, z);
      m.castShadow = m.receiveShadow = true;
      g.add(m);
      return m;
    };

    g.add(terrain(P, 0.02, 0xfbfbf8));
    // Earth faces between the plot edge and road level, so a raised or sunken plot reads.
    for (let i = 0; i < P.length; i++) {
      const a = P[i], b = P[(i + 1) % P.length];
      const ya = gy(a[0], a[1]), yb = gy(b[0], b[1]);
      if (Math.abs(ya) < 0.02 && Math.abs(yb) < 0.02) continue;
      const geo = new THREE.BufferGeometry();
      geo.setAttribute("position", new THREE.Float32BufferAttribute([a[0], ya, a[1], b[0], yb, b[1], b[0], 0, b[1], a[0], 0, a[1]], 3));
      geo.setIndex([0, 1, 2, 0, 2, 3]);
      geo.computeVertexNormals();
      g.add(new THREE.Mesh(geo, mat(0xb9a27f, { side: THREE.DoubleSide })));
    }
    for (const i of L.road_edges.filter((i) => i < P.length)) {
      const [a2, b2] = roadQuad(P, i, L.street_width);
      g.add(flat([P[i], P[(i + 1) % P.length], b2, a2], 0.01, 0x4a4f55));
    }
    if (check?.buildable?.length) g.add(terrain(check.buildable, 0.05, 0xbfe3c8, { transparent: true, opacity: 0.7 }));
    for (const e of check?.edges ?? []) {
      if (e.required <= 0) continue;
      const pts = e.line.map((p) => new THREE.Vector3(p[0], gy(p[0], p[1]) + 0.08, p[1]));
      const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineDashedMaterial({ color: EDGE_COLOR[e.class], dashSize: 0.6, gapSize: 0.4 }));
      line.computeLineDistances();
      g.add(line);
    }
    // Boundary wall around the plot.
    if (L.wall) {
      for (let i = 0; i < P.length; i++) {
        const a = P[i], b = P[(i + 1) % P.length];
        const h = L.road_edges.includes(i) ? L.wall.front_h : L.wall.other_h;
        const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
        const base = gy((a[0] + b[0]) / 2, (a[1] + b[1]) / 2);
        const wall = box(len, h, 0.2, 0xe8e2d6, (a[0] + b[0]) / 2, base + h / 2, (a[1] + b[1]) / 2);
        wall.rotation.y = -Math.atan2(b[1] - a[1], b[0] - a[0]);
      }
    }

    const buildings = L.items.filter((i) => i.kind === "building");
    const hasInside = (b) => L.items.some((i) => INTERIOR.has(i.kind) && Math.abs(i.x - b.x) < b.w / 2 && Math.abs(i.y - b.y) < b.d / 2);
    // Every block sits on the ground under it; buildings on their plinth.
    const baseG = g;
    for (const it of L.items) {
      const red = bad.has(it.id);
      const lift = new THREE.Group();
      const lv = it.kind === "building" ? levelOf(it.id) : null;
      const groundHere = it.a ? 0 : gy(it.x, it.y);
      lift.position.y = it.kind === "building"
        ? (lv ? lv.floor - road : groundHere + (it.plinth ?? PLINTH))
        : it.kind === "electric_line" || it.kind === "water_body" ? 0 : groundHere;
      if (it.kind === "building") {
        const gmin = Math.min(...[[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([sx, sz]) => gy(it.x + (sx * it.w) / 2, it.y + (sz * it.d) / 2)));
        const ph = lift.position.y - gmin;
        if (ph > 0.01) {
          const pl = new THREE.Mesh(new THREE.BoxGeometry(it.w + 0.3, ph, it.d + 0.3), mat(0xbfb6a6));
          pl.position.set(it.x, gmin + ph / 2, it.y);
          pl.castShadow = pl.receiveShadow = true;
          baseG.add(pl);
        }
      }
      const host = (b) => {
        const l2 = levelOf(b?.id);
        return l2 ? l2.floor - road : gy(b?.x ?? 0, b?.y ?? 0) + (b?.plinth ?? PLINTH);
      };
      const inner = INTERIOR.has(it.kind) || it.kind === "balcony";
      if (inner) {
        const hb = buildings.find((b) => Math.abs(b.x - it.x) < (b.w + it.w) / 2 + 0.1 && Math.abs(b.y - it.y) < (b.d + it.d) / 2 + 0.1);
        lift.position.y = hb ? host(hb) : groundHere;
      }
      baseG.add(lift);
      g = lift;
      const tint = (c) => (red ? 0xd46a6a : c);
      switch (it.kind) {
        case "building": {
          const floors = Math.max(1, it.floors || 1);
          const base = it.stilt ? 2.6 : 0;
          const xray = hasInside(it);
          for (let f = 0; f < floors; f++) {
            box(it.w, FLOOR_H - 0.2, it.d, tint(0xdfe6f0), it.x, base + f * FLOOR_H + (FLOOR_H - 0.2) / 2, it.y, xray ? { transparent: true, opacity: 0.35, depthWrite: false } : {});
            box(it.w + 0.2, 0.2, it.d + 0.2, 0x9fb1c9, it.x, base + (f + 1) * FLOOR_H - 0.1, it.y, xray ? { transparent: true, opacity: 0.5 } : {});
          }
          if (it.stilt)
            for (const [dx, dz] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) box(0.3, base, 0.3, 0xc9ccc4, it.x + (dx * (it.w - 0.4)) / 2, base / 2, it.y + (dz * (it.d - 0.4)) / 2);
          break;
        }
        case "balcony": {
          const host = buildings.find((b) => Math.abs(b.x - it.x) < (b.w + it.w) / 2 + 0.1 && Math.abs(b.y - it.y) < (b.d + it.d) / 2 + 0.1);
          const floors = host ? Math.max(1, host.floors || 1) : 2;
          const base = host?.stilt ? 2.6 : 0;
          for (let f = 1; f < floors; f++) {
            box(it.w, 0.15, it.d, tint(0xc9ccc4), it.x, base + f * FLOOR_H, it.y);
            box(it.w, 1.0, it.d, tint(0x5b6166), it.x, base + f * FLOOR_H + 0.6, it.y, { transparent: true, opacity: 0.25, depthWrite: false });
          }
          break;
        }
        case "garage":
          box(it.w, 2.6, it.d, tint(0xd8d2c4), it.x, 1.3, it.y);
          break;
        case "parking_bay":
          box(it.w, 0.03, it.d, 0xf4f4ef, it.x, 0.05, it.y);
          box(it.w - 0.6, 1.3, it.d - 1, 0x3d6fa8, it.x, 0.7, it.y);
          break;
        case "staircase":
        case "lift":
        case "shaft": {
          const host = buildings.find((b) => Math.abs(b.x - it.x) < b.w / 2 && Math.abs(b.y - it.y) < b.d / 2);
          const h = host ? (host.floors || 1) * FLOOR_H + (host.stilt ? 2.6 : 0) : FLOOR_H;
          const col = { staircase: 0xb8925c, lift: 0x1f3a5f, shaft: 0x2a3440 }[it.kind];
          box(it.w, h + (it.kind === "shaft" ? 0.5 : 2.4), it.d, tint(col), it.x, (h + (it.kind === "shaft" ? 0.5 : 2.4)) / 2, it.y, { transparent: true, opacity: 0.75 });
          break;
        }
        case "courtyard": {
          const host = buildings.find((b) => Math.abs(b.x - it.x) < b.w / 2 && Math.abs(b.y - it.y) < b.d / 2);
          const h = host ? (host.floors || 1) * FLOOR_H : FLOOR_H;
          box(it.w, h, it.d, tint(0x8fd0e8), it.x, h / 2, it.y, { transparent: true, opacity: 0.3, depthWrite: false });
          break;
        }
        case "septic_tank":
          box(it.w, 1, it.d, tint(0x8a7a64), it.x, -0.45, it.y, { transparent: true, opacity: 0.85 });
          box(it.w + 0.1, 0.08, it.d + 0.1, tint(0x9a8f80), it.x, 0.06, it.y);
          break;
        case "rwh_pit": {
          const m = new THREE.Mesh(new THREE.CylinderGeometry(it.w / 2, it.w / 2, 0.12, 20), mat(tint(0x5aa0c8)));
          m.position.set(it.x, 0.08, it.y);
          g.add(m);
          break;
        }
        case "tree": {
          const trunk = new THREE.Mesh(new THREE.CylinderGeometry(0.12, 0.16, 1.6, 6), mat(0x7a5a3a));
          trunk.position.set(it.x, 0.8, it.y);
          const crown = new THREE.Mesh(new THREE.SphereGeometry(it.w / 2 + 0.3, 10, 8), mat(0x4c8a5a));
          crown.position.set(it.x, 2.3, it.y);
          trunk.castShadow = crown.castShadow = true;
          g.add(trunk, crown);
          break;
        }
        case "gate":
          box(it.w, 1.4, Math.max(it.d, 0.1), tint(0x3a3f44), it.x, 0.7, it.y);
          break;
        case "water_body":
          box(it.w, 0.05, it.d, 0x4f8fc0, it.x, 0.03, it.y, { transparent: true, opacity: 0.85 });
          break;
        case "electric_line": {
          const H = { lv: 7, hv: 9, ehv: 22 }[it.voltage] ?? 7;
          for (const p of [it.a, it.b]) box(0.25, H, 0.25, 0x6b6f73, p[0], H / 2, p[1]);
          const pts = [new THREE.Vector3(it.a[0], H, it.a[1]), new THREE.Vector3(it.b[0], H, it.b[1])];
          g.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineBasicMaterial({ color: 0x222222 })));
          break;
        }
      }
    }

    g = baseG;
    if (!this.framed) this.frame(L);
  }

  frame(L) {
    const P = L.plot;
    const xs = P.map((p) => p[0]), ys = P.map((p) => p[1]);
    const span = Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)) + 10;
    this.camera.position.set(span * 0.6, span * 0.7, span * 0.9);
    this.controls.target.set(0, 2, 0);
    this.controls.update();
    this.framed = true;
  }
}

/* ---------- geometry helpers ---------- */

/** Iso-level lines of the ground plane z = a·x + b·y + c, clipped to the plot outline. */
function contours(P, [a, b, c], fs) {
  const xs = P.map((p) => p[0]), ys = P.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const zs = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(([x, y]) => a * x + b * y + c);
  const lo = Math.min(...zs), hi = Math.max(...zs);
  const steps = [0.1, 0.2, 0.3, 0.5, 1, 2, 5];
  const step = steps.find((v) => (hi - lo) / v <= 14) ?? 10;
  let s = `<clipPath id="dp-clip"><polygon points="${P.map((p) => p.join(",")).join(" ")}"/></clipPath><g clip-path="url(#dp-clip)" class="dp-contours">`;
  for (let z = Math.ceil(lo / step) * step; z <= hi; z += step) {
    let p1, p2;
    if (Math.abs(b) > Math.abs(a)) p1 = [x0, (z - c - a * x0) / b], p2 = [x1, (z - c - a * x1) / b];
    else p1 = [(z - c - b * y0) / a, y0], p2 = [(z - c - b * y1) / a, y1];
    s += `<line x1="${p1[0]}" y1="${p1[1]}" x2="${p2[0]}" y2="${p2[1]}"/>`;
    const m = [(p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2];
    s += `<text x="${m[0]}" y="${m[1]}" font-size="${fs * 0.8}" class="dp-contour-t">${fmt(z)}</text>`;
  }
  return s + "</g>";
}
function segDist(p, a, b) {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const l2 = dx * dx + dy * dy;
  const t = l2 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
}

function signedArea(P) {
  let s = 0;
  for (let i = 0; i < P.length; i++) {
    const a = P[i], b = P[(i + 1) % P.length];
    s += a[0] * b[1] - b[0] * a[1];
  }
  return s / 2;
}

/** The far edge of the road band drawn outside plot edge i. */
function roadQuad(P, i, width) {
  const a = P[i], b = P[(i + 1) % P.length];
  const sg = signedArea(P) > 0 ? 1 : -1;
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len = Math.hypot(dx, dy) || 1;
  const nx = (sg * dy) / len, ny = (-sg * dx) / len;
  return [[a[0] + nx * width, a[1] + ny * width], [b[0] + nx * width, b[1] + ny * width]];
}
