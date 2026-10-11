/* One drawing function per catalog component (src/catalog.rs).
 * Each takes the layout context and the component from /api/scene and returns a THREE.Group.
 * All sizes come from `comp.params` (computed from the rule text in Rust); this file only
 * places them. Anything marked illustrative in `comp.illustrative` is a placement choice.
 *
 * Context `k`: { s (scene), W, D (plot), fw, fd (footprint), bx, bz (footprint centre),
 *   y0 (top of plinth, or of stilt), floorH, floors, H (building height), roofY, front, rear, left, right } */
import * as THREE from "three";

export const COLORS = {
  ok: 0x2f6f4f,
  bad: 0xb23b3b,
  concrete: 0xc9ccc4,
  railing: 0x5b6166,
  glass: 0x6f8fb3,
  wall: 0xe8e2d6,
  gate: 0x3a3f44,
  lift: 0x1f3a5f,
  shaft: 0x2a3440,
  court: 0x8fd0e8,
  stair: 0xb8925c,
  car: [0xb23b3b, 0x3d6fa8, 0xe8e2d6, 0x4a4f55, 0xc58a1e],
  trunk: 0x7a5a3a,
  crown: 0x4c8a5a,
  septic: 0x8a7a64,
  rwh: 0x5aa0c8,
  pole: 0x6b6f73,
  wire: 0x222222,
  water: 0x4f8fc0,
  buffer: 0xb23b3b,
};

const mat = (color, extra = {}) => new THREE.MeshStandardMaterial({ color, roughness: 0.85, ...extra });
function box(w, h, d, color, extra) {
  const m = new THREE.Mesh(new THREE.BoxGeometry(Math.max(w, 0.01), Math.max(h, 0.01), Math.max(d, 0.01)), mat(color, extra));
  m.castShadow = true;
  m.receiveShadow = true;
  return m;
}
const at = (o, x, y, z) => (o.position.set(x, y, z), o);

/** Group tagged so clicks open the component inspector. */
function group(comp) {
  const g = new THREE.Group();
  g.userData.component = comp;
  return g;
}

export const BUILDERS = {
  balcony(k, c) {
    const g = group(c);
    const d = c.params.depth;
    if (d <= 0) return g;
    const w = k.fw * c.params.width_frac;
    const tint = c.ok ? COLORS.concrete : COLORS.bad;
    for (let i = c.params.from_floor; i < k.floors; i++) {
      const y = k.y0 + i * k.floorH;
      const z = k.bz + k.fd / 2 + d / 2;
      g.add(at(box(w, 0.15, d, tint), k.bx, y, z));
      // railing: front + two returns
      const rail = { transparent: true, opacity: 0.55 };
      g.add(at(box(w, 1.0, 0.05, COLORS.railing, rail), k.bx, y + 0.57, k.bz + k.fd / 2 + d));
      g.add(at(box(0.05, 1.0, d, COLORS.railing, rail), k.bx - w / 2, y + 0.57, z));
      g.add(at(box(0.05, 1.0, d, COLORS.railing, rail), k.bx + w / 2, y + 0.57, z));
      // door into the balcony
      g.add(at(box(1.0, 2.1, 0.04, COLORS.glass), k.bx, y + 1.1, k.bz + k.fd / 2 + 0.02));
    }
    return g;
  },

  chhajja(k, c) {
    const g = group(c);
    const d = c.params.depth;
    const n = Math.max(1, Math.floor(k.fw / 3.2));
    const step = k.fw / n;
    for (let i = 0; i < k.floors; i++) {
      const y = k.y0 + i * k.floorH;
      for (let j = 0; j < n; j++) {
        const x = k.bx - k.fw / 2 + step * (j + 0.5);
        for (const side of [1, -1]) {
          const zf = k.bz + side * (k.fd / 2);
          g.add(at(box(1.2, 1.2, 0.04, COLORS.glass), x, y + 1.5, zf + side * 0.02));
          g.add(at(box(1.6, 0.08, d, COLORS.concrete), x, y + 2.25, zf + side * (d / 2)));
        }
      }
    }
    return g;
  },

  parapet(k, c) {
    const g = group(c);
    const h = c.params.height, t = 0.15, y = k.roofY + h / 2;
    const col = COLORS.concrete;
    g.add(at(box(k.fw, h, t, col), k.bx, y, k.bz + k.fd / 2 - t / 2));
    g.add(at(box(k.fw, h, t, col), k.bx, y, k.bz - k.fd / 2 + t / 2));
    g.add(at(box(t, h, k.fd, col), k.bx - k.fw / 2 + t / 2, y, k.bz));
    g.add(at(box(t, h, k.fd, col), k.bx + k.fw / 2 - t / 2, y, k.bz));
    return g;
  },

  boundary_wall(k, c) {
    const g = group(c);
    const { front_h, other_h, gate_w } = c.params;
    const t = 0.2, W = k.W, D = k.D;
    const seg = (W - gate_w) / 2;
    // front, split by the gate
    g.add(at(box(seg, front_h, t, COLORS.wall), -W / 2 + seg / 2, front_h / 2, D / 2 - t / 2));
    g.add(at(box(seg, front_h, t, COLORS.wall), W / 2 - seg / 2, front_h / 2, D / 2 - t / 2));
    // gate leaves folded back against the wall (Rule 41(c))
    g.add(at(box(gate_w / 2, front_h * 0.9, 0.06, COLORS.gate), -gate_w / 2 - gate_w / 4, front_h * 0.45, D / 2 - t - 0.08));
    g.add(at(box(gate_w / 2, front_h * 0.9, 0.06, COLORS.gate), gate_w / 2 + gate_w / 4, front_h * 0.45, D / 2 - t - 0.08));
    g.add(at(box(W, other_h, t, COLORS.wall), 0, other_h / 2, -D / 2 + t / 2));
    g.add(at(box(t, other_h, D, COLORS.wall), -W / 2 + t / 2, other_h / 2, 0));
    g.add(at(box(t, other_h, D, COLORS.wall), W / 2 - t / 2, other_h / 2, 0));
    return g;
  },

  staircase(k, c) {
    const g = group(c);
    const p = c.params;
    const tint = c.ok ? COLORS.stair : COLORS.bad;
    // Core at the rear-left corner of the footprint.
    const x0 = k.bx - k.fw / 2 + 0.15, z0 = k.bz - k.fd / 2 + 0.15;
    const per = p.per_flight, rise = (k.floorH) / (2 * per);
    const steps = [];
    for (let f = 0; f < k.floors; f++) {
      const base = k.y0 + f * k.floorH;
      for (let s = 0; s < per; s++) {
        // flight A: left half, climbs from the front landing towards the rear
        steps.push([x0 + p.width / 2, base + (s + 1) * rise, z0 + p.core_d - p.landing - (s + 0.5) * p.tread]);
        // flight B: right half, climbs back from the mid landing
        steps.push([x0 + p.width * 1.5 + 0.1, base + k.floorH / 2 + (s + 1) * rise, z0 + p.landing + (s + 0.5) * p.tread]);
      }
      // mid landing (rear) and floor landing (front)
      g.add(at(box(p.core_w, 0.12, p.landing, tint), x0 + p.core_w / 2, base + k.floorH / 2, z0 + p.landing / 2));
      g.add(at(box(p.core_w, 0.12, p.landing, tint), x0 + p.core_w / 2, base, z0 + p.core_d - p.landing / 2));
    }
    const geo = new THREE.BoxGeometry(p.width, 0.06, p.tread);
    const inst = new THREE.InstancedMesh(geo, mat(tint), steps.length);
    const m4 = new THREE.Matrix4();
    steps.forEach(([x, y, z], i) => inst.setMatrixAt(i, m4.makeTranslation(x, y, z)));
    inst.castShadow = true;
    g.add(inst);
    // stair cabin (mumty) on the roof, with the required headroom
    const cab = box(p.core_w, p.headroom + 0.3, p.core_d, COLORS.concrete);
    g.add(at(cab, x0 + p.core_w / 2, k.roofY + (p.headroom + 0.3) / 2, z0 + p.core_d / 2));
    return g;
  },

  lift(k, c) {
    const g = group(c);
    const p = c.params;
    const x = k.bx - k.fw / 2 + 0.15 + (k.stairW ?? 0) + p.shaft_w / 2 + 0.2;
    const z = k.bz - k.fd / 2 + 0.15 + p.shaft_d / 2;
    const h = k.roofY - k.y0;
    g.add(at(box(p.shaft_w, h, p.shaft_d, COLORS.lift, { transparent: true, opacity: 0.35 }), x, k.y0 + h / 2, z));
    g.add(at(box(p.shaft_w + 0.3, p.machine_h, p.shaft_d + 0.3, COLORS.concrete), x, k.roofY + p.machine_h / 2, z));
    const cab = at(box(p.shaft_w - 0.3, 2.2, p.shaft_d - 0.3, c.ok ? 0xf2c14e : COLORS.bad), x, k.y0 + 1.1, z);
    cab.userData.animate = (t) => {
      const span = Math.max(0, h - 2.4);
      cab.position.y = k.y0 + 1.1 + span * (0.5 - 0.5 * Math.cos(t / 2600));
    };
    g.add(cab);
    return g;
  },

  ventilation_shaft(k, c) {
    const g = group(c);
    const { a, b } = c.params;
    const x = k.bx + k.fw / 2 - a / 2 - 0.4, z = k.bz;
    const h = k.roofY - k.y0 + 0.6;
    g.add(at(box(a, h, b, COLORS.shaft, { transparent: true, opacity: 0.8 }), x, k.y0 + h / 2, z));
    return g;
  },

  courtyard(k, c) {
    const g = group(c);
    const s = c.params.side;
    const h = k.roofY - k.y0;
    g.add(at(box(s, h, s, COLORS.court, { transparent: true, opacity: 0.25, depthWrite: false }), k.bx, k.y0 + h / 2, k.bz));
    g.add(at(box(s, 0.05, s, c.ok ? 0xbfe3c8 : COLORS.bad), k.bx, k.y0 + 0.03, k.bz));
    return g;
  },

  stilt_parking(k, c) {
    const g = group(c);
    const p = c.params;
    const yb = k.plinth;
    // columns on a grid
    const nx = Math.max(2, Math.round(k.fw / 4) + 1), nz = Math.max(2, Math.round(k.fd / 4) + 1);
    for (let i = 0; i < nx; i++)
      for (let j = 0; j < nz; j++) {
        const x = k.bx - k.fw / 2 + 0.2 + (i * (k.fw - 0.4)) / (nx - 1);
        const z = k.bz - k.fd / 2 + 0.2 + (j * (k.fd - 0.4)) / (nz - 1);
        g.add(at(box(0.3, p.clear_h, 0.3, COLORS.concrete), x, yb + p.clear_h / 2, z));
      }
    // cars in bays along the front, then further rows
    const cols = Math.floor(k.fw / p.bay_w), rows = Math.max(1, Math.floor(k.fd / p.bay_l));
    let n = 0;
    for (let r = 0; r < rows && n < p.cars; r++)
      for (let i = 0; i < cols && n < p.cars; i++, n++) {
        const x = k.bx - k.fw / 2 + p.bay_w * (i + 0.5);
        const z = k.bz + k.fd / 2 - p.bay_l * (r + 0.5);
        const body = box(p.bay_w - 0.6, 0.8, p.bay_l - 0.9, COLORS.car[n % COLORS.car.length]);
        g.add(at(body, x, yb + 0.55, z));
        g.add(at(box(p.bay_w - 0.8, 0.55, (p.bay_l - 0.9) * 0.5, 0x2a3440), x, yb + 1.2, z - 0.15));
        g.add(at(box(p.bay_w - 0.05, 0.01, 0.06, 0xffffff), x, yb + 0.02, z + p.bay_l / 2));
      }
    return g;
  },

  basement(k, c) {
    const g = group(c);
    const h = c.params.clear_h + 0.3;
    g.add(at(box(k.fw, h, k.fd, 0x9aa3ad, { transparent: true, opacity: 0.45, depthWrite: false }), k.bx, -h / 2, k.bz));
    g.add(at(box(k.fw, 0.1, k.fd, 0x6b7280), k.bx, -h, k.bz));
    // ramp down from the front open space
    const ramp = box(3, 0.15, Math.min(6, k.front + k.fd * 0.3), 0x6b7280);
    ramp.rotation.x = -Math.atan2(h, Math.min(6, k.front + k.fd * 0.3));
    g.add(at(ramp, k.bx + k.fw / 2 - 1.8, -h / 2, k.bz + k.fd / 2 - 1));
    return g;
  },

  garage(k, c) {
    const g = group(c);
    const { w, l, side } = c.params;
    const x = side === "left" ? -k.W / 2 + w / 2 + 0.05 : k.W / 2 - w / 2 - 0.05;
    const z = -k.D / 2 + k.rear + l / 2;
    const tint = c.ok ? 0xd8d2c4 : COLORS.bad;
    g.add(at(box(w, 2.6, l, tint), x, 1.3, z));
    g.add(at(box(w * 0.85, 2.1, 0.05, COLORS.gate), x, 1.05, z + l / 2 + 0.03));
    return g;
  },

  trees(k, c) {
    const g = group(c);
    const n = Math.min(60, c.params.count);
    if (!n) return g;
    const pts = openSpots(k, 2.6).slice(0);
    const pick = [];
    for (let i = 0; i < n && pts.length; i++) pick.push(pts[Math.floor((i * pts.length) / n)]);
    const trunkGeo = new THREE.CylinderGeometry(0.12, 0.16, 1.6, 6);
    const crownGeo = new THREE.SphereGeometry(1.0, 10, 8);
    const trunks = new THREE.InstancedMesh(trunkGeo, mat(COLORS.trunk), pick.length);
    const crowns = new THREE.InstancedMesh(crownGeo, mat(COLORS.crown), pick.length);
    const m4 = new THREE.Matrix4();
    pick.forEach(([x, z], i) => {
      const s = 0.8 + ((i * 37) % 10) / 25;
      trunks.setMatrixAt(i, m4.makeTranslation(x, 0.8, z));
      crowns.setMatrixAt(i, m4.makeScale(s, s * 1.15, s).setPosition(x, 2.1 * s + 0.4, z));
    });
    trunks.castShadow = crowns.castShadow = true;
    g.add(trunks, crowns);
    return g;
  },

  septic_tank(k, c) {
    const g = group(c);
    const { w, len, depth, gap, zone } = c.params;
    let x = k.bx, z = k.bz, rotate = false;
    if (zone === "rear") z = k.bz - k.fd / 2 - gap - w / 2;
    else if (zone === "front") z = k.bz + k.fd / 2 + gap + w / 2;
    else {
      rotate = true;
      x = zone === "left" ? k.bx - k.fw / 2 - gap - w / 2 : k.bx + k.fw / 2 + gap + w / 2;
    }
    const tank = box(rotate ? w : len, depth, rotate ? len : w, COLORS.septic, { transparent: true, opacity: 0.8 });
    g.add(at(tank, x, -depth / 2 + 0.02, z));
    g.add(at(box(rotate ? w + 0.1 : len + 0.1, 0.08, rotate ? len + 0.1 : w + 0.1, c.ok ? 0x9a8f80 : COLORS.bad), x, 0.06, z));
    g.add(at(new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.03, 2, 6), mat(COLORS.railing)), x + (rotate ? 0 : len / 2), 1.0, z + (rotate ? len / 2 : 0)));
    return g;
  },

  rainwater_harvesting(k, c) {
    const g = group(c);
    const x = -k.W / 2 + Math.max(0.9, k.left / 2), z = k.D / 2 - Math.max(0.9, k.front / 2);
    g.add(at(new THREE.Mesh(new THREE.CylinderGeometry(0.6, 0.6, 0.12, 20), mat(COLORS.rwh)), x, 0.08, z));
    // downpipe from the roof
    g.add(at(new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, k.roofY, 8), mat(COLORS.railing)), k.bx - k.fw / 2 - 0.08, k.roofY / 2, k.bz + k.fd / 2 - 0.3));
    return g;
  },

  electric_line(k, c) {
    const g = group(c);
    const p = c.params;
    const z = k.D / 2; // along the front plot boundary
    const span = Math.max(k.W * 3, 60);
    for (let x = -span / 2; x <= span / 2; x += 15) {
      g.add(at(new THREE.Mesh(new THREE.CylinderGeometry(0.12, 0.18, p.wire_h + 0.6, 8), mat(COLORS.pole)), x, (p.wire_h + 0.6) / 2, z));
      g.add(at(box(1.6, 0.1, 0.1, COLORS.pole), x, p.wire_h, z));
    }
    for (const dz of [-0.6, 0.6]) g.add(at(box(span, 0.03, 0.03, COLORS.wire), 0, p.wire_h - 0.02, z + dz));
    // clearance envelope: horizontal both sides of the line, vertical above/below
    // Rule 52 gives a vertical clearance for LV/HV (around the wire) and a full-height
    // horizontal corridor for EHV.
    const [lo, hi] = p.v != null ? [Math.max(0, p.wire_h - p.v), p.wire_h + p.v] : [0, p.wire_h + 2];
    const env = box(span, hi - lo, 2 * p.h, c.ok ? 0xf2c14e : COLORS.bad, { transparent: true, opacity: 0.16, depthWrite: false });
    env.castShadow = false;
    g.add(at(env, 0, (lo + hi) / 2, z));
    return g;
  },

  water_body(k, c) {
    const g = group(c);
    const D = k.D;
    const water = box(Math.max(k.W * 4, 80), 0.05, 40, COLORS.water, { transparent: true, opacity: 0.85, roughness: 0.2, metalness: 0.1 });
    g.add(at(water, 0, 0.03, -D / 2 - 20));
    const buf = box(k.W, 0.03, Math.min(c.params.buffer, D), COLORS.buffer, { transparent: true, opacity: 0.18, depthWrite: false });
    g.add(at(buf, 0, 0.16, -D / 2 + Math.min(c.params.buffer, D) / 2));
    return g;
  },
};

/** Grid points in the plot's open spaces (outside the footprint), for trees. */
export function openSpots(k, step) {
  const out = [];
  for (let x = -k.W / 2 + 0.9; x <= k.W / 2 - 0.9; x += step)
    for (let z = -k.D / 2 + 0.9; z <= k.D / 2 - 0.9; z += step) {
      const inside = Math.abs(x - k.bx) < k.fw / 2 + 1.2 && Math.abs(z - k.bz) < k.fd / 2 + 1.2;
      if (!inside) out.push([x, z]);
    }
  return out;
}
