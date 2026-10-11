/* 3D plot view for Building rules.
 * Every distance comes from /api/scene (computed in Rust from the rule text). This file lays out
 * geometry; components3d.js draws each catalog component.
 * Axes: x = along the frontage, z = plot depth (street at +z), y = up. 1 unit = 1 m. */
import * as THREE from "three";
import { OrbitControls } from "three/addons/OrbitControls.js";
import { BUILDERS } from "/components3d.js";

const C = {
  ground: 0xeceee8,
  plot: 0xfbfbf8,
  plotEdge: 0x8a9095,
  street: 0x4a4f55,
  streetLine: 0xf2f2ea,
  building: 0xdfe6f0,
  buildingEdge: 0x1f3a5f,
  slab: 0x9fb1c9,
  bad: 0xe3c9c9,
  neighbour: 0xd3d5cf,
  front: 0x4c9a6d,
  rear: 0xc58a1e,
  side: 0x3d6fa8,
  roof: 0x9b5b3c,
};
const ZONE_OPACITY = 0.45;
/** Components that sit inside the building: switching one on turns X-ray on. */
const INSIDE = new Set(["staircase", "lift", "ventilation_shaft", "basement", "courtyard"]);
const USES = [
  ["residential", "Residential"],
  ["business", "Shop / office"],
  ["educational", "School / college"],
  ["institutional", "Hospital"],
  ["assembly", "Hall / cinema"],
  ["industrial", "Industrial"],
  ["farm_house", "Farm house"],
];

const slug = (v) => window.slug(v);
const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const fmt = (v) => (v == null ? "—" : Number(v).toFixed(2).replace(/\.?0+$/, ""));

let catalogP = null; // GET /api/catalog, once
const catalog = () => (catalogP ??= fetch("/api/catalog").then((r) => r.json()).catch(() => ({ components: [] })));

let active = null; // only one live scene at a time
let LABEL_SCALE = 1; // labels grow with the plot so they stay readable

/** Mount an interactive 3D scene for `scene` (the /api/scene shape) into `host`. */
export function mountScene(host, scene) {
  if (active) active.dispose();
  active = new PlotView(host, scene);
}
window.mountScene = mountScene;
window.disposeScene = () => {
  active?.dispose();
  active = null;
};

class PlotView {
  constructor(host, scene) {
    this.host = host;
    this.scene = scene;
    this.input = initialInput(scene);
    this.pinned = null;
    this.hovered = null;
    this.pickables = [];
    this.animated = [];
    this.anim = null;
    this.xray = scene.components.some((c) => INSIDE.has(c.id));

    host.innerHTML = `
      <button type="button" class="s3-edit" data-testid="scene-edit-in-draw-plot" data-edit title="Move things around and check them against the rules as you go">Edit in Draw plot →</button>
      <div class="s3-stage" data-testid="scene-stage">
        <div class="s3-canvas" data-testid="scene-canvas"></div>
        <div class="s3-tip" data-testid="scene-tooltip" hidden></div>
        <div class="s3-views" data-testid="scene-views">
          <button type="button" data-testid="scene-view-3d" data-view="3d" class="is-on">3D</button>
          <button type="button" data-testid="scene-view-top" data-view="top">Top</button>
          <button type="button" data-testid="scene-view-street" data-view="street">Street</button>
          <button type="button" data-testid="scene-xray" data-xray class="${this.xray ? "is-on" : ""}" title="See inside the building">X-ray</button>
        </div>
        <div class="s3-status" data-testid="scene-status"></div>
        <p class="s3-hint" data-testid="scene-hint">Drag to orbit · scroll to zoom · click anything for its rule</p>
      </div>
      <div class="s3-side" data-testid="scene-side">
        <div class="s3-readout" data-testid="scene-readout"></div>
        <div class="s3-info" data-testid="scene-info"></div>
        <form class="s3-controls" data-testid="scene-controls" autocomplete="off"></form>
      </div>
      <div class="s3-palette" data-testid="scene-palette"></div>`;
    this.$ = (s) => host.querySelector(s);

    if (!webglOK()) {
      this.$(".s3-canvas").innerHTML = '<p class="s3-nogl" data-testid="scene-no-webgl">3D view needs WebGL, which this browser has turned off.</p>';
      this.renderPanels();
      return;
    }

    const canvasHost = this.$(".s3-canvas");
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    canvasHost.appendChild(this.renderer.domElement);

    this.three = new THREE.Scene();
    this.three.background = new THREE.Color(0xf6f6f3);
    this.three.fog = new THREE.Fog(0xf6f6f3, 160, 340);

    this.camera = new THREE.PerspectiveCamera(40, 1, 0.1, 1200);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.maxPolarAngle = Math.PI / 2 - 0.03;
    this.controls.minDistance = 4;
    this.controls.maxDistance = 400;

    this.three.add(new THREE.HemisphereLight(0xffffff, 0xb9bcb3, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.9);
    sun.position.set(-40, 70, 45);
    sun.castShadow = true;
    sun.shadow.mapSize.set(2048, 2048);
    Object.assign(sun.shadow.camera, { left: -90, right: 90, top: 90, bottom: -90, far: 300 });
    this.three.add(sun);

    this.ground = new THREE.Mesh(
      new THREE.PlaneGeometry(900, 900),
      new THREE.MeshStandardMaterial({ color: C.ground, roughness: 1, transparent: true, opacity: 1 })
    );
    this.ground.rotation.x = -Math.PI / 2;
    this.ground.receiveShadow = true;
    this.three.add(this.ground);

    this.group = new THREE.Group();
    this.three.add(this.group);

    this.raycaster = new THREE.Raycaster();
    this.pointer = new THREE.Vector2();
    const el = this.renderer.domElement;
    this.onMove = (e) => this.pick(e, false);
    this.onClick = (e) => this.pick(e, true);
    this.onLeave = () => this.setHover(null);
    el.addEventListener("pointermove", this.onMove);
    el.addEventListener("click", this.onClick);
    el.addEventListener("pointerleave", this.onLeave);

    this.host.querySelectorAll("[data-view]").forEach((b) =>
      b.addEventListener("click", () => {
        this.host.querySelectorAll("[data-view]").forEach((x) => x.classList.toggle("is-on", x === b));
        this.frame(b.dataset.view);
      })
    );
    this.$("[data-edit]").addEventListener("click", () => {
      // Show the tab first so the editor can size itself, then load the layout.
      document.querySelector('.tab[data-track="draw"]')?.click();
      window.scrollTo({ top: 0 });
      const layout = sceneToLayout(this.scene);
      const go = (n = 0) => (window.drawPlot ? window.drawPlot.open(layout) : n < 60 && setTimeout(() => go(n + 1), 50));
      setTimeout(go, 0);
    });
    this.$("[data-xray]").addEventListener("click", (e) => {
      this.xray = !this.xray;
      e.currentTarget.classList.toggle("is-on", this.xray);
      this.build(false);
    });

    this.resize = new ResizeObserver(() => this.fit());
    this.resize.observe(canvasHost);
    this.fit();

    this.build(true);
    this.frame("3d");
    this.renderPanels();

    const loop = (t) => {
      this.raf = requestAnimationFrame(loop);
      this.tick(t);
      this.controls.update();
      this.renderer.render(this.three, this.camera);
    };
    this.raf = requestAnimationFrame(loop);
  }

  dispose() {
    cancelAnimationFrame(this.raf);
    clearTimeout(this.debounce);
    this.resize?.disconnect();
    this.controls?.dispose();
    if (this.renderer) {
      const el = this.renderer.domElement;
      el.removeEventListener("pointermove", this.onMove);
      el.removeEventListener("click", this.onClick);
      el.removeEventListener("pointerleave", this.onLeave);
      disposeTree(this.three);
      this.renderer.dispose();
      el.remove();
    }
  }

  fit() {
    const box = this.$(".s3-canvas");
    const w = box.clientWidth, h = box.clientHeight;
    if (!w || !h) return;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  /* ---------- geometry ---------- */
  build(grow) {
    disposeTree(this.group);
    this.group.clear();
    this.pickables = [];
    this.animated = [];
    const s = this.scene;
    const g = this.group;
    const W = s.frontage_m, D = s.depth_m;
    LABEL_SCALE = Math.max(1, Math.max(W, D, s.height_m) / 28);
    const front = s.front?.m ?? 0, rear = s.rear?.m ?? 0;
    const left = s.side_left?.m ?? 0, right = s.side_right?.m ?? 0;
    const has = (id) => s.components.find((c) => c.id === id);

    // Ground goes see-through when there is something below it.
    const below = has("basement") || has("septic_tank");
    this.ground.material.opacity = below ? 0.55 : 1;
    this.ground.material.depthWrite = !below;

    // Levels: the plot stays at y = 0; the road (and the land around) sits rise metres lower.
    const rise = s.plot_rise_m || 0;
    this.ground.position.y = rise > 0 ? -rise : 0;
    if (rise > 0.01) {
      // Raised plot on an earth bank.
      const bank = box(W, rise, D, 0xb9a27f);
      bank.position.set(0, -rise / 2, 0);
      g.add(bank);
    } else if (rise < -0.01) {
      // Road on an embankment above the plot, with a retaining face at the boundary.
      const len = Math.max(W * 3, 60);
      const emb = box(len, -rise, s.street_width_m + 2, 0xb9a27f);
      emb.position.set(0, -rise / 2, D / 2 + s.street_width_m / 2 + 1);
      g.add(emb);
    }
    if (Math.abs(rise) > 0.01)
      g.add(label(`plot ${rise > 0 ? "+" : "−"}${fmt(Math.abs(rise))} m ${rise > 0 ? "above" : "below"} road`, { x: W / 2 + 3, y: Math.max(0, -rise) + 0.8, z: D / 2, size: 0.85, ink: "#6b4a00", bg: "rgba(255,255,255,.9)" }));

    // Street along the front edge.
    const streetW = s.street_width_m;
    const streetLen = Math.max(W * 3, 60);
    const street = box(streetLen, 0.04, streetW, C.street, { roughness: 0.95 });
    street.position.set(0, 0.02 - rise, D / 2 + streetW / 2);
    street.receiveShadow = true;
    g.add(street);
    for (let x = -streetLen / 2 + 1.5; x < streetLen / 2; x += 4) {
      const dash = box(2, 0.01, 0.15, C.streetLine);
      dash.position.set(x + 1, 0.05 - rise, D / 2 + streetW / 2);
      g.add(dash);
    }
    g.add(label(`${fmt(streetW)} m wide street`, { x: -W / 2 - 4, y: 0.6 - rise, z: D / 2 + streetW / 2, size: 1.0, ink: "#ffffff", bg: "rgba(31,58,95,.85)" }));

    // Plot slab and boundary.
    const plot = box(W, 0.1, D, C.plot);
    plot.position.y = 0.05;
    plot.receiveShadow = true;
    g.add(plot);
    g.add(outline(W, D, 0.12, C.plotEdge));

    // Open spaces: coloured strips on the plot, pickable.
    const zoneY = 0.13;
    const midD = D - front - rear, midZ = (rear - front) / 2;
    if (front > 0) this.zone(s.front, C.front, W, front, 0, D / 2 - front / 2, zoneY);
    if (rear > 0) this.zone(s.rear, C.rear, W, rear, 0, -D / 2 + rear / 2, zoneY);
    if (left > 0 && midD > 0) this.zone(s.side_left, C.side, left, midD, -W / 2 + left / 2, midZ, zoneY);
    if (right > 0 && midD > 0) this.zone(s.side_right, C.side, right, midD, W / 2 - right / 2, midZ, zoneY);
    if (front > 0) g.add(label(`front ${fmt(front)} m`, { x: 0, y: 0.5, z: D / 2 - front / 2, size: 0.9, ink: "#1f5137" }));
    if (rear > 0) g.add(label(`rear ${fmt(rear)} m`, { x: 0, y: 0.5, z: -D / 2 + rear / 2, size: 0.9, ink: "#6b4a00" }));
    if (left > 0 && midD > 0) g.add(label(`${fmt(left)} m`, { x: -W / 2 + left / 2, y: 0.5, z: midZ, size: 0.8, ink: "#1f3a5f" }));
    if (right > 0 && midD > 0) g.add(label(`${fmt(right)} m`, { x: W / 2 - right / 2, y: 0.5, z: midZ, size: 0.8, ink: "#1f3a5f" }));
    g.add(label(`frontage ${fmt(W)} m`, { x: 0, y: 0.4, z: D / 2 + 0.9, size: 0.8, ink: "#5b6166", bg: "rgba(255,255,255,.85)" }));
    g.add(label(`depth ${fmt(D)} m`, { x: W / 2 + 2.4, y: 0.4, z: 0, size: 0.8, ink: "#5b6166", bg: "rgba(255,255,255,.85)" }));

    // Layout context shared with the component builders.
    const fw = s.footprint_w_m, fd = s.footprint_d_m;
    const stilt = has("stilt_parking");
    const stiltH = stilt ? stilt.params.clear_h + 0.2 : 0;
    const k = {
      s, W, D, fw, fd, front, rear, left, right,
      bx: (left - right) / 2,
      bz: midZ,
      plinth: s.plinth_m,
      y0: s.plinth_m + stiltH,
      floors: s.floors,
      floorH: s.floor_h_m,
      H: s.height_m,
      stairW: has("staircase")?.params.core_w,
    };
    k.roofY = k.y0 + s.height_m;
    this.k = k;

    // Building.
    this.building = null;
    if (fw > 0 && fd > 0) {
      const b = new THREE.Group();
      // Plinth across the footprint.
      const pl = box(fw + 0.3, s.plinth_m, fd + 0.3, 0xbfb6a6);
      pl.position.set(k.bx, s.plinth_m / 2, k.bz);
      g.add(pl);
      const court = has("courtyard");
      const hole = court && court.ok ? court.params.side : 0;
      const bad = !s.covered || !s.permitted;
      const wallMat = new THREE.MeshStandardMaterial({
        color: bad ? C.bad : C.building,
        roughness: 0.75,
        transparent: this.xray || !s.covered,
        opacity: this.xray ? 0.18 : s.covered ? 1 : 0.7,
        depthWrite: !this.xray,
      });
      const slabMat = new THREE.MeshStandardMaterial({ color: C.slab, roughness: 0.8, transparent: this.xray, opacity: this.xray ? 0.35 : 1 });
      const fh = k.floorH;
      for (let i = 0; i < s.floors; i++) {
        for (const [w, d, x, z] of ring(fw, fd, hole)) {
          const storey = new THREE.Mesh(new THREE.BoxGeometry(w, fh - 0.18, d), wallMat);
          storey.position.set(x, k.y0 + i * fh + (fh - 0.18) / 2, z);
          storey.castShadow = !this.xray;
          storey.receiveShadow = true;
          b.add(storey);
          const slab = new THREE.Mesh(new THREE.BoxGeometry(w + 0.2, 0.18, d + 0.2), slabMat);
          slab.position.set(x, k.y0 + (i + 1) * fh - 0.09, z);
          slab.castShadow = !this.xray;
          b.add(slab);
        }
      }
      if (stilt) {
        // stilt deck: the first slab above the open parking floor
        const deck = new THREE.Mesh(new THREE.BoxGeometry(fw + 0.2, 0.2, fd + 0.2), slabMat);
        deck.position.set(k.bx, k.y0 - 0.1, k.bz);
        b.add(deck);
      }
      if (s.roof === "sloping") b.add(gable(fw, fd, k.bx, k.roofY, k.bz));
      const edges = new THREE.LineSegments(
        new THREE.EdgesGeometry(new THREE.BoxGeometry(fw, s.height_m, fd)),
        new THREE.LineBasicMaterial({ color: C.buildingEdge, transparent: true, opacity: 0.5 })
      );
      edges.position.set(k.bx, k.y0 + s.height_m / 2, k.bz);
      b.add(edges);
      const tag = `${fmt(s.height_m)} m · ${s.floors} floor${s.floors > 1 ? "s" : ""}${stilt ? " + stilt" : ""}`;
      g.add(label(tag, { x: k.bx, y: k.roofY + 1.6, z: k.bz, size: 0.95, ink: "#1f3a5f", bg: "rgba(255,255,255,.9)" }));
      g.add(b);
      this.building = b;
      if (grow) {
        b.scale.y = 0.001;
        this.anim = { t0: performance.now(), dur: 900 };
      }
    }

    // Components from the catalog.
    for (const comp of s.components) {
      const fn = BUILDERS[comp.id];
      if (!fn || !(fw > 0 && fd > 0) && !["boundary_wall", "trees", "electric_line", "water_body"].includes(comp.id)) continue;
      const obj = fn(k, comp);
      obj.traverse((o) => {
        if (o.isMesh) this.pickables.push(o);
        if (o.userData.animate) this.animated.push(o);
      });
      g.add(obj);
    }

    // Neighbours make shared walls readable for semi-detached and row houses.
    const nH = Math.min(s.height_m, 9);
    const neighbour = (side) => {
      const n = box(W * 0.9, nH, D * 0.6, C.neighbour, { transparent: true, opacity: 0.55 });
      n.position.set(side * (W + 0.05), nH / 2 + 0.1, -D * 0.05);
      g.add(n);
    };
    if (s.building_type === "row") neighbour(-1), neighbour(1);
    if (s.building_type === "semi-detached") neighbour(1);
  }

  zone(setback, color, w, d, x, z, y) {
    const mat = new THREE.MeshStandardMaterial({ color, transparent: true, opacity: ZONE_OPACITY, roughness: 1 });
    const m = new THREE.Mesh(new THREE.BoxGeometry(w, 0.04, d), mat);
    m.position.set(x, y, z);
    m.receiveShadow = true;
    m.userData.setback = setback;
    this.group.add(m);
    this.pickables.push(m);
  }

  tick(t) {
    for (const o of this.animated) o.userData.animate(t);
    if (!this.anim || !this.building) return;
    const p = Math.min(1, (performance.now() - this.anim.t0) / this.anim.dur);
    this.building.scale.y = Math.max(0.001, 1 - Math.pow(1 - p, 3));
    if (p >= 1) this.anim = null;
  }

  frame(view) {
    const s = this.scene;
    const span = Math.max(s.frontage_m, s.depth_m + s.street_width_m * 0.5, s.height_m * 1.9) + 6;
    const target = new THREE.Vector3(0, s.height_m * 0.4, s.street_width_m * 0.25);
    const pos = {
      "3d": new THREE.Vector3(span * 0.62, span * 0.58, span * 0.8),
      top: new THREE.Vector3(0, span * 1.45, 0.01),
      street: new THREE.Vector3(span * 0.15, 1.7 + s.height_m * 0.15, s.depth_m / 2 + s.street_width_m + span * 0.55),
    }[view];
    this.camera.position.copy(pos.add(new THREE.Vector3(0, 0, target.z)));
    this.controls.target.copy(target);
    this.controls.update();
  }

  /* ---------- picking ---------- */
  pick(e, click) {
    const r = this.renderer.domElement.getBoundingClientRect();
    this.pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    this.raycaster.setFromCamera(this.pointer, this.camera);
    const hit = this.raycaster.intersectObjects(this.pickables, false)[0]?.object ?? null;
    const target = hit && (hit.userData.setback ? { setback: hit.userData.setback, mesh: hit } : componentOf(hit));
    this.setHover(target, e);
    if (click && target) {
      this.pinned = target.setback ? { setback: target.setback } : { component: target.component.id };
      this.renderInfo();
    }
  }

  setHover(target, e) {
    if (this.hovered?.mesh && this.hovered.mesh !== target?.mesh) this.hovered.mesh.material.opacity = ZONE_OPACITY;
    this.hovered = target;
    const tip = this.$(".s3-tip");
    this.renderer.domElement.style.cursor = target ? "pointer" : "";
    if (!target) return (tip.hidden = true);
    if (target.setback) {
      target.mesh.material.opacity = 0.8;
      tip.innerHTML = `<b>${esc(target.setback.label)}</b> · ${fmt(target.setback.m)} m`;
    } else {
      const c = target.component;
      tip.innerHTML = `<b>${esc(c.name)}</b> · ${c.ok ? "allowed" : "<span class=s3-no>not allowed here</span>"}`;
    }
    const r = this.$(".s3-stage").getBoundingClientRect();
    tip.style.left = `${e.clientX - r.left + 12}px`;
    tip.style.top = `${e.clientY - r.top + 12}px`;
    tip.hidden = false;
  }

  /* ---------- panels ---------- */
  renderPanels() {
    this.renderStatus();
    this.renderReadout();
    this.renderInfo();
    this.renderControls();
    this.renderPalette();
  }

  renderStatus() {
    const s = this.scene;
    this.$(".s3-status").innerHTML = `<span class="s3-badge ${s.permitted && s.covered ? "ok" : "no"}" data-testid="scene-status-badge">
      ${s.permitted && s.covered ? "Within these rules" : s.covered ? "Breaks a rule — see notes" : "Not computed"}</span>
      <span class="s3-env" data-testid="scene-envelope">${esc(s.envelope)}</span>`;
  }

  renderReadout() {
    const s = this.scene;
    const cover = s.plot_area_sqm ? (s.footprint_sqm / s.plot_area_sqm) * 100 : 0;
    const assumed = s.assumed.length
      ? `<p class="s3-assumed" data-testid="scene-assumed"><b>Assumed:</b> ${s.assumed.map((a) => esc(a.text)).join(" · ")}</p>`
      : "";
    const notes =
      s.notes.map((n, i) => `<p class="s3-warn" data-testid="scene-warning-${i + 1}">${esc(n)}</p>`).join("") +
      (s.info || []).map((n, i) => `<p class="s3-infonote" data-testid="scene-note-${i + 1}">${esc(n)}</p>`).join("");
    const kind = { detached: "Detached", "semi-detached": "Semi-detached", row: "Row-type" }[s.building_type];
    const far = s.far
      ? `<div class="s3-far ${s.far.ok ? "" : "over"}" data-testid="scene-far-meter" data-pin="far" title="Show rule text">
           <div class="s3-far-head"><span>Floor area vs FAR ${fmt(s.far.far)}</span>
             <b>${fmt(s.far.proposed_sqm)} / ${fmt(s.far.allowed_sqm)} sq m</b></div>
           <span class="s3-far-bar"><span style="width:${Math.min(100, (s.far.proposed_sqm / Math.max(s.far.allowed_sqm, 1)) * 100)}%"></span></span>
         </div>`
      : "";
    this.$(".s3-readout").innerHTML = s.covered
      ? `<div class="s3-big" data-testid="scene-buildable-area">${fmt(s.footprint_sqm)} <span>sq m buildable per floor</span></div>
         <div class="s3-small" data-testid="scene-footprint">${fmt(s.footprint_w_m)} m × ${fmt(s.footprint_d_m)} m · ${Math.round(cover)}% of a ${fmt(s.plot_area_sqm)} sq m plot${s.use_type === "residential" ? ` · ${kind}` : ""}${
           s.open_space_limit_sqm > s.footprint_sqm + 0.5 ? `<br>${fmt(s.open_space_limit_sqm)} sq m inside the open spaces; less is drawn because of ${s.far && s.far.proposed_sqm >= s.far.allowed_sqm - 0.5 ? "FAR" : "ground coverage"}.` : ""
         }</div>
         ${far}
         <ul class="s3-legend" data-testid="scene-open-spaces">
           ${legendRow("front", s.front)}${legendRow("rear", s.rear)}
           ${legendRow("side", s.side_left, s.side_right)}
         </ul>${assumed}${notes}`
      : `<div class="s3-big s3-big-warn" data-testid="scene-not-covered">Not covered <span>by these rules</span></div>${assumed}${notes}`;
    this.$(".s3-readout").querySelectorAll("[data-pin]").forEach((li) =>
      li.addEventListener("click", () => {
        this.pinned = li.dataset.pin === "far" ? { far: true } : { setback: this.scene[li.dataset.pin] };
        this.renderInfo();
      })
    );
  }

  renderInfo() {
    const p = this.pinned, s = this.scene;
    const id = window.testIds();
    const cite = (x) => {
      const t = id("scene-info-cite", x.label);
      return `<div class="cite-head" data-testid="${t}"><span class="rule" data-testid="${t}-rule">${esc(x.label)}</span><span class="page" data-testid="${t}-page">${esc(x.cite)}</span>${window.docButton ? window.docButton(x, `${t}-show-in-document`) : ""}</div>
      <pre class="quote" data-testid="${t}-quote">${esc(x.quote)}</pre>`;
    };
    let html = "";
    if (p?.setback) html = cite(p.setback);
    else if (p?.far && s.far) html = `${s.far.note ? `<p class="s3-small" data-testid="scene-info-far-note">${esc(s.far.note)}</p>` : ""}${cite(s.far.source)}`;
    else if (p?.component) {
      const c = s.components.find((x) => x.id === p.component);
      if (c)
        html = `<div class="s3-comp-head" data-testid="scene-info-component"><b data-testid="scene-info-component-name">${esc(c.name)}</b> <span class="s3-badge ${c.ok ? "ok" : "no"}" data-testid="scene-info-component-status">${c.ok ? "allowed" : "not allowed here"}</span></div>
          ${c.notes.map((n, i) => `<p class="s3-cnote" data-testid="scene-info-component-note-${i + 1}">${esc(n)}</p>`).join("")}
          ${c.illustrative.length ? `<p class="s3-illus" data-testid="scene-info-component-illustrative"><b>Drawn, not ruled:</b> ${c.illustrative.map(esc).join(" ")}</p>` : ""}
          ${c.sources.map(cite).join("")}`;
    }
    this.$(".s3-info").innerHTML = html;
  }

  renderControls() {
    const i = this.input, s = this.scene;
    const val = (k, shown) => (i[k] != null ? i[k] : shown);
    const slider = (k, lbl, min, max, step, shown, unit) => `
      <label class="s3-row" data-testid="scene-control-${slug(k)}-field">
        <span class="s3-lbl">${lbl}</span>
        <input type="range" data-testid="scene-control-${slug(k)}" name="${k}" min="${min}" max="${max}" step="${step}" value="${val(k, shown)}" />
        <output data-testid="scene-control-${slug(k)}-value">${fmt(val(k, shown))} ${unit}</output>
      </label>`;
    const kinds = [["detached", "Detached"], ["semi-detached", "Semi"], ["row", "Row"]];
    const form = this.$(".s3-controls");
    form.innerHTML = `
      <label class="s3-row" data-testid="scene-control-use-field"><span class="s3-lbl">Use</span>
        <select name="use_type" data-testid="scene-control-use">${USES.map(([v, t]) => `<option value="${v}" ${s.use_type === v ? "selected" : ""}>${t}</option>`).join("")}</select>
      </label>
      ${slider("plot_area", "Plot area", 30, 6000, 5, s.plot_area_sqm, "sq m")}
      ${slider("frontage", "Frontage", 4, 80, 0.5, s.frontage_m, "m")}
      ${slider("street_width", "Street width", 3, 80, 0.5, s.street_width_m, "m")}
      ${slider("height", "Height", 3, 100, 0.5, s.height_m, "m")}
      ${slider("plot_rise", "Plot vs road", -3, 3, 0.1, s.plot_rise_m || 0, "m")}
      <div class="s3-row" data-testid="scene-control-type-field" ${s.use_type === "residential" ? "" : "hidden"}><span class="s3-lbl">Type</span>
        <div class="s3-seg" data-testid="scene-control-type">${kinds
          .map(([k, t]) => `<button type="button" data-testid="scene-control-type-${slug(k)}" data-kind="${k}" class="${s.building_type === k ? "is-on" : ""}">${t}</button>`)
          .join("")}</div>
      </div>`;
    form.querySelectorAll("input[type=range]").forEach((r) =>
      r.addEventListener("input", () => {
        this.input[r.name] = Number(r.value);
        if (r.name === "height") delete this.input.floors;
        r.nextElementSibling.textContent = `${fmt(r.value)} ${r.name === "plot_area" ? "sq m" : "m"}`;
        this.refetch();
      })
    );
    form.querySelector("select").addEventListener("change", (e) => {
      this.input.use_type = e.target.value;
      this.refetch(true);
    });
    form.querySelectorAll("[data-kind]").forEach((b) =>
      b.addEventListener("click", () => {
        this.input.building_type = b.dataset.kind;
        form.querySelectorAll("[data-kind]").forEach((x) => x.classList.toggle("is-on", x === b));
        this.refetch();
      })
    );
  }

  async renderPalette() {
    const cat = await catalog();
    const s = this.scene;
    const on = new Set(this.input.components);
    const req = new Map(s.suggested.map((x) => [x.id, x.why]));
    const groups = {};
    for (const e of cat.components.filter((e) => !e.not_drawn)) (groups[e.group] ??= []).push(e);
    const chip = (e) => {
      const c = s.components.find((x) => x.id === e.id);
      const cls = ["s3-chip", on.has(e.id) ? "is-on" : "", c && !c.ok ? "is-bad" : ""].join(" ");
      const r = req.get(e.id);
      return `<button type="button" class="${cls}" data-testid="scene-component-${slug(e.id)}" data-comp="${e.id}" title="Rule ${esc(e.rules)}${r ? " — " + esc(r) : ""}">
        ${esc(e.name)}${r ? '<i class="s3-req" title="Required here">required</i>' : ""}</button>`;
    };
    const nid = window.testIds();
    const nd = s.not_drawn.length
      ? `<div class="s3-nd" data-testid="scene-not-drawn"><b>In your question but not drawn:</b>
          ${s.not_drawn.map((m) => `<p data-testid="${nid("scene-not-drawn", m.name)}"><span class="rule">${esc(m.name)}</span> <span class="page">Rule ${esc(m.rules)}</span> — ${esc(m.why)}</p>`).join("")}</div>`
      : "";
    // Folded by default: the answer leads, the extras are one click away. Open state survives re-renders.
    const need = s.suggested.filter((x) => !on.has(x.id)).length;
    this.$(".s3-palette").innerHTML = `
      <details class="s3-pal" data-testid="scene-components" ${this.palOpen ? "open" : ""}>
        <summary data-testid="scene-components-toggle">Components <span class="s3-sub" data-testid="scene-components-count">${on.size} shown${need ? ` · ${need} required` : ""}</span></summary>
        ${Object.entries(groups)
          .map(([g, es]) => `<div class="s3-pal-row" data-testid="scene-components-group-${slug(g)}"><span class="s3-lbl">${esc(g)}</span><div class="s3-chips">${es.map(chip).join("")}</div></div>`)
          .join("")}
      </details>
      ${nd}`;
    this.$(".s3-pal").addEventListener("toggle", (e) => (this.palOpen = e.target.open));
    this.$(".s3-palette").querySelectorAll("[data-comp]").forEach((b) =>
      b.addEventListener("click", () => {
        const id = b.dataset.comp;
        const list = new Set(this.input.components);
        if (list.has(id)) list.delete(id);
        else {
          list.add(id);
          if (INSIDE.has(id) && !this.xray) {
            this.xray = true;
            this.$("[data-xray]")?.classList.add("is-on");
          }
          this.pinned = { component: id };
        }
        this.input.components = [...list];
        this.refetch(true);
      })
    );
  }

  refetch(now = false) {
    clearTimeout(this.debounce);
    this.debounce = setTimeout(async () => {
      const q = { ...this.input, components: this.input.components.join(",") };
      const qs = new URLSearchParams(Object.entries(q).filter(([, v]) => v != null && v !== ""));
      try {
        const r = await fetch(`/api/scene?${qs}`);
        if (!r.ok) throw new Error(await r.text());
        this.scene = await r.json();
      } catch (err) {
        this.$(".s3-readout").innerHTML = `<p class="s3-warn" data-testid="scene-error">Could not recompute: ${esc(err.message)}</p>`;
        return;
      }
      if (this.pinned?.setback) {
        const k = ["front", "rear", "side_left", "side_right"].find((k) => this.scene[k]?.label === this.pinned.setback.label);
        this.pinned = k ? { setback: this.scene[k] } : null;
      }
      if (this.renderer) this.build(false);
      this.renderStatus();
      this.renderReadout();
      this.renderInfo();
      this.renderPalette();
      if (now) this.renderControls();
      // Frontage slider follows the server while frontage is still derived from area.
      if (this.input.frontage == null) {
        const f = this.$('input[name="frontage"]');
        f.value = this.scene.frontage_m;
        f.nextElementSibling.textContent = `${fmt(this.scene.frontage_m)} m`;
      }
    }, now ? 0 : 120);
  }
}

/* ---------- helpers ---------- */

/** The composed scene as a Draw-plot layout: same plot, building and components, ready to move. */
function sceneToLayout(s) {
  const W = s.frontage_m, D = s.depth_m;
  const front = s.front?.m ?? 0, rear = s.rear?.m ?? 0, left = s.side_left?.m ?? 0, right = s.side_right?.m ?? 0;
  const fw = s.footprint_w_m, fd = s.footprint_d_m;
  const bx = left + (W - left - right) / 2, by = rear + (D - front - rear) / 2;
  const r2 = (v) => Math.round(v * 100) / 100;
  const items = [];
  const add = (kind, name, o) => items.push({ id: `${kind}-${items.length + 1}`, kind, name, ...o });
  const has = (id) => s.components.find((c) => c.id === id);
  add("building", "Building 1", { x: r2(bx), y: r2(by), w: fw, d: fd, floors: s.floors, stilt: !!has("stilt_parking") });
  let wall = null;
  for (const c of s.components) {
    const p = c.params;
    switch (c.id) {
      case "balcony":
        if (p.depth > 0) add("balcony", "Balcony", { x: r2(bx), y: r2(by + fd / 2 + p.depth / 2), w: r2(fw * p.width_frac), d: p.depth });
        break;
      case "garage":
        add("garage", "Garage", { x: r2(p.side === "left" ? p.w / 2 + 0.05 : W - p.w / 2 - 0.05), y: r2(rear + p.l / 2), w: p.w, d: p.l });
        break;
      case "staircase":
        add("staircase", "Staircase", { x: r2(bx - fw / 2 + p.core_w / 2 + 0.15), y: r2(by - fd / 2 + p.core_d / 2 + 0.15), w: p.core_w, d: p.core_d, width: p.width });
        break;
      case "lift": {
        const sw = has("staircase")?.params.core_w ?? 0;
        add("lift", "Lift", { x: r2(bx - fw / 2 + 0.15 + sw + 0.2 + p.shaft_w / 2), y: r2(by - fd / 2 + 0.15 + p.shaft_d / 2), w: p.shaft_w, d: p.shaft_d });
        break;
      }
      case "ventilation_shaft":
        add("shaft", "Shaft", { x: r2(bx + fw / 2 - p.a / 2 - 0.4), y: r2(by), w: p.a, d: p.b });
        break;
      case "courtyard":
        add("courtyard", "Courtyard", { x: r2(bx), y: r2(by), w: p.side, d: p.side });
        break;
      case "septic_tank": {
        const z = { rear: [bx, by - fd / 2 - p.gap - p.w / 2, p.len, p.w], front: [bx, by + fd / 2 + p.gap + p.w / 2, p.len, p.w],
          left: [bx - fw / 2 - p.gap - p.w / 2, by, p.w, p.len], right: [bx + fw / 2 + p.gap + p.w / 2, by, p.w, p.len] }[p.zone];
        add("septic_tank", "Septic tank", { x: r2(z[0]), y: r2(z[1]), w: z[2], d: z[3] });
        break;
      }
      case "trees": {
        const spots = [];
        for (let x = 0.9; x <= W - 0.9; x += 2.6)
          for (let y = 0.9; y <= D - 0.9; y += 2.6)
            if (!(Math.abs(x - bx) < fw / 2 + 1.2 && Math.abs(y - by) < fd / 2 + 1.2)) spots.push([x, y]);
        const n = Math.min(40, p.count, spots.length);
        for (let i = 0; i < n; i++) {
          const [x, y] = spots[Math.floor((i * spots.length) / n)];
          add("tree", `Tree ${i + 1}`, { x: r2(x), y: r2(y), w: 1.4, d: 1.4 });
        }
        break;
      }
      case "rainwater_harvesting":
        add("rwh_pit", "Rainwater pit", { x: r2(Math.max(0.9, left / 2)), y: r2(D - Math.max(0.9, front / 2)), w: 1.2, d: 1.2 });
        break;
      case "electric_line":
        add("electric_line", "Power line", { a: [-5, r2(D + 0.5)], b: [r2(W + 5), r2(D + 0.5)], voltage: p.class });
        break;
      case "water_body":
        add("water_body", p.kind === "river" ? "River" : "Lake", { x: r2(W / 2), y: -12, w: r2(W * 2.5), d: 24, water: p.kind });
        break;
      case "boundary_wall":
        wall = { front_h: p.front_h, other_h: p.other_h };
        add("gate", "Gate", { x: r2(W / 2), y: r2(D - 0.15), w: p.gate_w, d: 0.3 });
        break;
    }
  }
  return {
    plot: [[0, 0], [W, 0], [W, D], [0, D]],
    road_edges: [2],
    street_width: s.street_width_m,
    road_level: 500,
    plot_levels: [0, 0, 0, 0].map(() => Math.round((500 + (s.plot_rise_m || 0)) * 100) / 100),
    use_type: s.use_type,
    building_type: s.building_type,
    wall,
    items,
  };
}

/** Inputs the question actually gave; assumed ones stay unset so the server keeps deriving them. */
function initialInput(s) {
  const assumed = new Set(s.assumed.map((a) => a.field));
  const input = { components: [...s.components.map((c) => c.id), ...s.not_drawn.map((m) => m.id)] };
  if (!assumed.has("plot_area")) input.plot_area = s.plot_area_sqm;
  if (!assumed.has("frontage")) input.frontage = s.frontage_m;
  if (!assumed.has("street_width")) input.street_width = s.street_width_m;
  if (!assumed.has("building_type")) input.building_type = s.building_type;
  if (!assumed.has("use_type")) input.use_type = s.use_type;
  if (s.plot_rise_m) input.plot_rise = s.plot_rise_m;
  // A floor count stays a floor count, so its height keeps showing as assumed.
  if (assumed.has("floors")) input.floors = Math.round(s.height_m / 3);
  else if (!assumed.has("height")) input.height = s.height_m;
  const line = s.components.find((c) => c.id === "electric_line");
  if (line && !line.illustrative.some((t) => t.includes("assumed"))) input.voltage = line.params.class;
  const water = s.components.find((c) => c.id === "water_body");
  if (water) input.water = water.params.kind;
  return input;
}

function componentOf(o) {
  for (let p = o; p; p = p.parent) if (p.userData.component) return { component: p.userData.component };
  return null;
}

function legendRow(kind, a, b) {
  if (!a && !b) return `<li class="s3-off" data-testid="scene-open-space-${kind}"><i class="sw sw-${kind}"></i>${kind} · none required</li>`;
  const v = b && b.m !== a.m ? `${fmt(a.m)} m + ${fmt(b.m)} m` : b ? `${fmt(a.m)} m both sides` : `${fmt(a.m)} m`;
  const pin = kind === "side" ? "side_left" : kind;
  return `<li data-testid="scene-open-space-${kind}" data-pin="${pin}" title="Show rule text"><i class="sw sw-${kind}"></i>${kind} · <b data-testid="scene-open-space-${kind}-value">${v}</b><span class="s3-src" data-testid="scene-open-space-${kind}-rule">${esc(a.label.split("· ")[1])}</span></li>`;
}

/** Footprint pieces: the whole box, or four wings around a courtyard of side `hole`. */
function ring(fw, fd, hole) {
  if (!hole) return [[fw, fd, 0, 0]];
  const wx = (fw - hole) / 2, wz = (fd - hole) / 2;
  return [
    [fw, wz, 0, -(hole / 2 + wz / 2)],
    [fw, wz, 0, hole / 2 + wz / 2],
    [wx, hole, -(hole / 2 + wx / 2), 0],
    [wx, hole, hole / 2 + wx / 2, 0],
  ];
}

function gable(fw, fd, x, y, z) {
  const rise = Math.min(fw, fd) * 0.3;
  const shape = new THREE.Shape();
  shape.moveTo(-fw / 2 - 0.4, 0);
  shape.lineTo(fw / 2 + 0.4, 0);
  shape.lineTo(0, rise);
  shape.closePath();
  const geo = new THREE.ExtrudeGeometry(shape, { depth: fd + 0.8, bevelEnabled: false });
  const m = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color: C.roof, roughness: 0.8 }));
  m.position.set(x, y, z - fd / 2 - 0.4);
  m.castShadow = true;
  return m;
}

function box(w, h, d, color, extra = {}) {
  return new THREE.Mesh(new THREE.BoxGeometry(w, h, d), new THREE.MeshStandardMaterial({ color, roughness: 0.9, ...extra }));
}

function outline(w, d, y, color) {
  const pts = [[-w / 2, -d / 2], [w / 2, -d / 2], [w / 2, d / 2], [-w / 2, d / 2], [-w / 2, -d / 2]].map(
    ([x, z]) => new THREE.Vector3(x, y, z)
  );
  return new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineBasicMaterial({ color }));
}

/** Camera-facing text label. `size` is the text height in metres. */
function label(text, { x, y, z, size = 1, ink = "#191b1c", bg = "rgba(255,255,255,0)" }) {
  const px = 64, pad = 18;
  const cv = document.createElement("canvas");
  const ctx = cv.getContext("2d");
  const font = `600 ${px}px system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`;
  ctx.font = font;
  cv.width = Math.ceil(ctx.measureText(text).width) + pad * 2;
  cv.height = px + pad * 1.4;
  ctx.font = font;
  ctx.fillStyle = bg;
  roundRect(ctx, 0, 0, cv.width, cv.height, 14);
  ctx.fill();
  ctx.fillStyle = ink;
  ctx.textBaseline = "middle";
  ctx.fillText(text, pad, cv.height / 2 + 2);
  const tex = new THREE.CanvasTexture(cv);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 4;
  const sp = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, depthTest: false, transparent: true }));
  const h = size * LABEL_SCALE * (cv.height / px);
  sp.scale.set(h * (cv.width / cv.height), h, 1);
  sp.position.set(x, y, z);
  sp.renderOrder = 10;
  return sp;
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function disposeTree(root) {
  root.traverse((o) => {
    if (o === root) return;
    o.geometry?.dispose();
    const mats = Array.isArray(o.material) ? o.material : o.material ? [o.material] : [];
    mats.forEach((m) => (m.map?.dispose(), m.dispose()));
  });
}

function webglOK() {
  try {
    const c = document.createElement("canvas");
    return !!(c.getContext("webgl2") || c.getContext("webgl"));
  } catch {
    return false;
  }
}
