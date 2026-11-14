// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//
// Nebula vocabulary for the emotion halo around the one dot.
// The one-dot page is not wired here. The halo imports this module.
//
//   import { drawNebula } from "./vocabulary.mjs";
//   // `emotion` is the object on GET /dot, passed through unchanged.
//   // Returns false and paints nothing when the feed has no real state.
//   const drew = drawNebula(ctx, emotion, seconds, {
//     cx, cy, inner: dotRadius, outer: dotRadius * 2.6,
//   });
//
// Neutral is never a cloud. Sense off, a missing reading, and confidence
// below the galion-emotion floor (0.2) paint nothing. Colour stays on the
// locked amber → violet background gradient. The disc (#F5F5F7) is not painted.
// Pulse and drift use 432 Hz lowered by ten octaves (2:1), then 3:2 or 5:4.

export const TABLE = {
  "schema": "galion-nebula-vocabulary/1",
  "brand": {
    "name": "Galion",
    "site": "galion.app",
    "ground": "#0A0A0C",
    "dot": "#F5F5F7",
    "amber": "#F5A524",
    "violet": "#7C3AED",
    "amberRgb": [245, 165, 36],
    "violetRgb": [124, 58, 237]
  },
  "clock": {
    "referenceHz": 432,
    "octavesDown": 10,
    "ratios": {
      "2:1": { "num": 2, "den": 1 },
      "3:2": { "num": 3, "den": 2 },
      "5:4": { "num": 5, "den": 4 }
    }
  },
  "confidenceFloor": 0.2,
  "states": [
    {
      "id": "calm",
      "present": true,
      "sources": ["affect", "dot", "mood"],
      "aliases": [],
      "amber": 0.46,
      "amberEdge": 0.28,
      "alpha": 0.07,
      "density": 0.045,
      "ratio": "2:1",
      "depth": 0.06,
      "driftTurns": 0.12
    },
    {
      "id": "quiet",
      "present": true,
      "sources": ["mood"],
      "aliases": [],
      "amber": 0.28,
      "amberEdge": 0.16,
      "alpha": 0.045,
      "density": 0.03,
      "ratio": "2:1",
      "depth": 0.04,
      "driftTurns": 0.08
    },
    {
      "id": "warm",
      "present": true,
      "sources": ["dot"],
      "aliases": [],
      "amber": 0.72,
      "amberEdge": 0.5,
      "alpha": 0.085,
      "density": 0.055,
      "ratio": "3:2",
      "depth": 0.08,
      "driftTurns": 0.16
    },
    {
      "id": "joy",
      "present": true,
      "sources": ["affect", "mood"],
      "aliases": ["happy"],
      "amber": 0.84,
      "amberEdge": 0.58,
      "alpha": 0.1,
      "density": 0.07,
      "ratio": "3:2",
      "depth": 0.1,
      "driftTurns": 0.2
    },
    {
      "id": "bright",
      "present": true,
      "sources": ["dot"],
      "aliases": [],
      "amber": 0.76,
      "amberEdge": 0.44,
      "alpha": 0.09,
      "density": 0.062,
      "ratio": "5:4",
      "depth": 0.1,
      "driftTurns": 0.24
    },
    {
      "id": "excited",
      "present": true,
      "sources": ["affect"],
      "aliases": [],
      "amber": 0.68,
      "amberEdge": 0.4,
      "alpha": 0.09,
      "density": 0.064,
      "ratio": "5:4",
      "depth": 0.12,
      "driftTurns": 0.28
    },
    {
      "id": "energised",
      "present": true,
      "sources": ["dot"],
      "aliases": ["energized"],
      "amber": 0.54,
      "amberEdge": 0.32,
      "alpha": 0.075,
      "density": 0.055,
      "ratio": "5:4",
      "depth": 0.11,
      "driftTurns": 0.26
    },
    {
      "id": "low",
      "present": true,
      "sources": ["dot"],
      "aliases": [],
      "amber": 0.2,
      "amberEdge": 0.1,
      "alpha": 0.06,
      "density": 0.04,
      "ratio": "2:1",
      "depth": 0.05,
      "driftTurns": 0.1
    },
    {
      "id": "sad",
      "present": true,
      "sources": ["affect"],
      "aliases": [],
      "amber": 0.14,
      "amberEdge": 0.08,
      "alpha": 0.065,
      "density": 0.042,
      "ratio": "2:1",
      "depth": 0.05,
      "driftTurns": 0.09
    },
    {
      "id": "grief",
      "present": true,
      "sources": ["mood"],
      "aliases": [],
      "amber": 0.08,
      "amberEdge": 0.2,
      "alpha": 0.075,
      "density": 0.048,
      "ratio": "3:2",
      "depth": 0.05,
      "driftTurns": 0.1
    },
    {
      "id": "tired",
      "present": true,
      "sources": ["affect"],
      "aliases": [],
      "amber": 0.24,
      "amberEdge": 0.12,
      "alpha": 0.05,
      "density": 0.032,
      "ratio": "2:1",
      "depth": 0.03,
      "driftTurns": 0.07
    },
    {
      "id": "vulnerability",
      "present": true,
      "sources": ["mood"],
      "aliases": [],
      "amber": 0.22,
      "amberEdge": 0.12,
      "alpha": 0.065,
      "density": 0.04,
      "ratio": "3:2",
      "depth": 0.06,
      "driftTurns": 0.11
    },
    {
      "id": "tense",
      "present": true,
      "sources": ["dot"],
      "aliases": [],
      "amber": 0.16,
      "amberEdge": 0.06,
      "alpha": 0.055,
      "density": 0.034,
      "ratio": "5:4",
      "depth": 0.04,
      "driftTurns": 0.14
    },
    {
      "id": "anxious",
      "present": true,
      "sources": ["affect"],
      "aliases": ["anxiety"],
      "amber": 0.18,
      "amberEdge": 0.07,
      "alpha": 0.05,
      "density": 0.032,
      "ratio": "5:4",
      "depth": 0.05,
      "driftTurns": 0.15
    },
    {
      "id": "angry",
      "present": true,
      "sources": ["affect"],
      "aliases": [],
      "amber": 0.3,
      "amberEdge": 0.14,
      "alpha": 0.045,
      "density": 0.026,
      "ratio": "2:1",
      "depth": 0.03,
      "driftTurns": 0.06
    },
    {
      "id": "neutral",
      "present": false,
      "sources": ["affect", "dot"],
      "aliases": ["unknown", "none"],
      "amber": 0,
      "amberEdge": 0,
      "alpha": 0,
      "density": 0,
      "ratio": "2:1",
      "depth": 0,
      "driftTurns": 0
    }
  ]
};

export const SCHEMA = TABLE.schema;
export const BRAND = TABLE.brand;
export const CLOCK = TABLE.clock;
export const CONFIDENCE_FLOOR = TABLE.confidenceFloor;
export const VOCABULARY = TABLE.states;

const BY_LABEL = new Map();
for (const row of VOCABULARY) {
  BY_LABEL.set(row.id, row);
  for (const alias of row.aliases) BY_LABEL.set(alias, row);
}

function clamp01(x) {
  if (typeof x !== "number" || !Number.isFinite(x)) return 0;
  return Math.min(1, Math.max(0, x));
}

export function basePeriodS() {
  return 2 ** CLOCK.octavesDown / CLOCK.referenceHz;
}

export function pulsePeriodS(ratio) {
  const r = CLOCK.ratios[ratio];
  if (!r) return 0;
  return basePeriodS() * (r.num / r.den);
}

export function mixRgb(amberShare) {
  const t = clamp01(amberShare);
  const a = BRAND.amberRgb;
  const v = BRAND.violetRgb;
  return [
    Math.round(v[0] + (a[0] - v[0]) * t),
    Math.round(v[1] + (a[1] - v[1]) * t),
    Math.round(v[2] + (a[2] - v[2]) * t),
  ];
}

function absent(reason) {
  return {
    present: false,
    id: null,
    reason,
    amber: 0,
    amberEdge: 0,
    alpha: 0,
    density: 0,
    ratio: null,
    depth: 0,
    driftTurns: 0,
    periodS: 0,
    driftHz: 0,
  };
}

function normaliseLabel(label) {
  if (typeof label !== "string") return "";
  return label.trim().toLowerCase().replace(/[^a-z-]/g, "");
}

function gainOf(input) {
  if (typeof input.arousal !== "number" || !Number.isFinite(input.arousal)) return 1;
  return 0.82 + 0.18 * clamp01(input.arousal);
}

/** Resolve one feed reading to a halo look. No reading paints nothing. */
export function lookup(input) {
  if (input == null || typeof input !== "object" || Array.isArray(input)) {
    return absent("no-reading");
  }
  if (input.sense === false) return absent("sense-off");
  if (typeof input.confidence === "number") {
    if (!Number.isFinite(input.confidence) || input.confidence < CONFIDENCE_FLOOR) {
      return absent("low-confidence");
    }
  }
  const label = normaliseLabel(input.label);
  if (!label || label === "neutral" || label === "unknown" || label === "none") {
    return absent("neutral");
  }
  const row = BY_LABEL.get(label);
  if (!row || row.present === false) return absent(row ? "neutral" : "unknown-label");
  const gain = gainOf(input);
  const periodS = pulsePeriodS(row.ratio);
  return {
    present: true,
    id: row.id,
    reason: null,
    amber: row.amber,
    amberEdge: row.amberEdge,
    alpha: row.alpha * gain,
    density: row.density * gain,
    ratio: row.ratio,
    depth: row.depth * gain,
    driftTurns: row.driftTurns,
    periodS,
    driftHz: periodS > 0 ? row.driftTurns / periodS : 0,
  };
}

/** Wisps for one moment. Empty when the look is absent. */
export function sample(input, timeS) {
  const look = lookup(input);
  if (!look.present) return { ...look, wisps: [], gradient: "none" };
  const t = typeof timeS === "number" && Number.isFinite(timeS) ? timeS : 0;
  const pulse = 1 + look.depth * Math.sin((t / look.periodS) * Math.PI * 2);
  const spin = (t / look.periodS) * look.driftTurns * Math.PI * 2;
  const n = Math.max(1, Math.min(3, Math.round(look.density * 40)));
  const wisps = [];
  for (let i = 0; i < n; i++) {
    wisps.push({
      angle: spin + (i * 2 * Math.PI) / n,
      offset: 0.08 + 0.14 * look.density,
      alpha: Math.min(0.14, look.alpha * pulse * (i === 0 ? 1 : 0.55)),
      amber: i === 0 ? look.amber : look.amberEdge,
      radius: 0.62 + 0.18 * (1 - i / 3),
    });
  }
  const [r, g, b] = mixRgb(look.amber);
  const [r2, g2, b2] = mixRgb(look.amberEdge);
  const a = Math.min(0.14, look.alpha * pulse);
  return {
    ...look,
    wisps,
    gradient: `radial-gradient(circle, rgba(${r},${g},${b},${a.toFixed(3)}) 0%, rgba(${r2},${g2},${b2},0) 72%)`,
  };
}

/**
 * Paint the halo on an annulus outside the dot. Does not clear the canvas
 * and does not paint the disc. Returns false when nothing was drawn.
 * The caller clears the halo layer each frame, then draws the locked dot.
 */
export function drawNebula(ctx, input, timeS, geom) {
  const field = sample(input, timeS);
  if (!field.present || !ctx || !geom) return false;
  const cx = geom.cx;
  const cy = geom.cy;
  const inner = Math.max(0, Number(geom.inner) || 0);
  const outer = Number(geom.outer);
  if (!(outer > inner)) return false;
  ctx.save();
  ctx.globalCompositeOperation = "source-over";
  ctx.beginPath();
  ctx.arc(cx, cy, outer, 0, Math.PI * 2);
  if (inner > 0) ctx.arc(cx, cy, inner, 0, Math.PI * 2, true);
  ctx.clip();
  const span = outer - inner;
  for (const w of field.wisps) {
    const dist = inner + span * (0.22 + w.offset);
    const x = cx + Math.cos(w.angle) * dist;
    const y = cy + Math.sin(w.angle) * dist * 0.92;
    const rad = span * w.radius;
    const [r, g, b] = mixRgb(w.amber);
    const gdt = ctx.createRadialGradient(x, y, rad * 0.08, x, y, rad);
    gdt.addColorStop(0, `rgba(${r},${g},${b},${w.alpha.toFixed(3)})`);
    gdt.addColorStop(1, `rgba(${r},${g},${b},0)`);
    ctx.fillStyle = gdt;
    ctx.beginPath();
    ctx.arc(x, y, rad, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
  return true;
}
