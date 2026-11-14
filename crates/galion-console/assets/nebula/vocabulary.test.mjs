import test from "node:test";
import assert from "node:assert/strict";
import {
  BRAND,
  CLOCK,
  CONFIDENCE_FLOOR,
  VOCABULARY,
  basePeriodS,
  drawNebula,
  lookup,
  mixRgb,
  pulsePeriodS,
  sample,
} from "./vocabulary.mjs";

const AFFECT = ["neutral", "calm", "joy", "excited", "sad", "tired", "angry", "anxious"];
const DOT = ["bright", "warm", "tense", "low", "energised", "calm"];
const MOOD = ["grief", "joy", "vulnerability", "quiet", "calm"];

function onSegment(rgb) {
  return rgb.every((c, i) => {
    const lo = Math.min(BRAND.amberRgb[i], BRAND.violetRgb[i]);
    const hi = Math.max(BRAND.amberRgb[i], BRAND.violetRgb[i]);
    return c >= lo && c <= hi;
  });
}

function fakeCtx() {
  const calls = [];
  return {
    calls,
    save() { calls.push("save"); },
    restore() { calls.push("restore"); },
    beginPath() { calls.push("begin"); },
    arc() { calls.push("arc"); },
    clip() { calls.push("clip"); },
    fill() { calls.push("fill"); },
    createRadialGradient() {
      calls.push("grad");
      return { addColorStop() { calls.push("stop"); } };
    },
    set fillStyle(_v) {},
    set globalCompositeOperation(_v) {},
  };
}

test("432 Hz clock is the octave, fifth, and third", () => {
  assert.equal(CLOCK.referenceHz, 432);
  assert.equal(CLOCK.octavesDown, 10);
  assert.equal(basePeriodS(), 2 ** 10 / 432);
  assert.equal(pulsePeriodS("2:1"), (2 ** 10 / 432) * (2 / 1));
  assert.equal(pulsePeriodS("3:2"), (2 ** 10 / 432) * (3 / 2));
  assert.equal(pulsePeriodS("5:4"), (2 ** 10 / 432) * (5 / 4));
  assert.equal(pulsePeriodS("7:4"), 0);
});

test("brand stops are the locked galion.app pair", () => {
  assert.equal(BRAND.site, "galion.app");
  assert.equal(BRAND.ground, "#0A0A0C");
  assert.equal(BRAND.dot, "#F5F5F7");
  assert.equal(BRAND.amber, "#F5A524");
  assert.equal(BRAND.violet, "#7C3AED");
  assert.deepEqual(mixRgb(0), [124, 58, 237]);
  assert.deepEqual(mixRgb(1), [245, 165, 36]);
  assert.deepEqual(mixRgb(0.5), [185, 112, 137]);
  assert.equal(CONFIDENCE_FLOOR, 0.2);
});

test("every feed state is listed once and stays faint", () => {
  const ids = VOCABULARY.map((row) => row.id);
  assert.deepEqual(new Set(ids).size, ids.length);
  for (const label of [...AFFECT, ...DOT, ...MOOD]) {
    assert.ok(ids.includes(label) || VOCABULARY.some((row) => row.aliases.includes(label)), label);
  }
  for (const row of VOCABULARY) {
    assert.ok(row.alpha <= 0.1, row.id);
    assert.ok(row.density <= 0.09, row.id);
    assert.ok(row.depth <= 0.14, row.id);
    assert.ok(row.driftTurns <= 0.4, row.id);
    assert.ok(row.amber >= 0 && row.amber <= 1, row.id);
    assert.ok(row.amberEdge >= 0 && row.amberEdge <= 1, row.id);
    assert.ok(["2:1", "3:2", "5:4"].includes(row.ratio), row.id);
    if (row.present) {
      const n = Math.round(row.density * 40);
      assert.ok(n >= 1 && n <= 3, row.id);
      assert.ok(onSegment(mixRgb(row.amber)), row.id);
      assert.ok(onSegment(mixRgb(row.amberEdge)), row.id);
    }
  }
  assert.equal(VOCABULARY.find((row) => row.id === "neutral").present, false);
});

test("anger is quieter and slower than joy", () => {
  const joy = lookup({ sense: true, label: "joy", confidence: 1 });
  const calm = lookup({ sense: true, label: "calm", confidence: 1 });
  const angry = lookup({ sense: true, label: "angry", confidence: 1 });
  assert.ok(angry.driftHz < calm.driftHz && calm.driftHz < joy.driftHz);
  assert.ok(angry.density < calm.density && calm.density < joy.density);
  assert.ok(angry.alpha < joy.alpha);
  assert.equal(angry.ratio, "2:1");
  assert.equal(angry.amber, 0.3);
});

test("no reading, sense off, neutral, and low confidence draw nothing", () => {
  const empty = [
    [null, "no-reading"],
    [undefined, "no-reading"],
    [[], "no-reading"],
    [{ sense: false, label: "neutral", valence: 0, arousal: 0, lean: 0 }, "sense-off"],
    [{ sense: false, label: "joy", valence: 0.8, arousal: 0.8, confidence: 1 }, "sense-off"],
    [{ sense: true, label: "neutral", valence: 0.4, arousal: 0.5, confidence: 1 }, "neutral"],
    [{ sense: true, label: "unknown", confidence: 1 }, "neutral"],
    [{ sense: true, label: "none", confidence: 1 }, "neutral"],
    [{ sense: true, label: "", confidence: 1 }, "neutral"],
    [{ label: "joy", confidence: 0.199 }, "low-confidence"],
    [{ label: "joy", confidence: Number.NaN }, "low-confidence"],
    [{ sense: true, label: "curious", confidence: 1 }, "unknown-label"],
    [{ sense: true, label: "lean-in", confidence: 1 }, "unknown-label"],
    [{ sense: true, label: "seductive", confidence: 1 }, "unknown-label"],
  ];
  for (const [input, reason] of empty) {
    const look = lookup(input);
    assert.equal(look.present, false, reason);
    assert.equal(look.reason, reason);
    assert.equal(look.alpha, 0);
    const field = sample(input, 1.25);
    assert.equal(field.present, false);
    assert.deepEqual(field.wisps, []);
    assert.equal(field.gradient, "none");
    const ctx = fakeCtx();
    assert.equal(drawNebula(ctx, input, 1.25, { cx: 40, cy: 40, inner: 10, outer: 36 }), false);
    assert.deepEqual(ctx.calls, []);
  }
});

test("a real label resolves, including aliases, and stays on the gradient", () => {
  const joy = lookup({ sense: true, label: " Joy ", confidence: 0.2 });
  assert.equal(joy.present, true);
  assert.equal(joy.id, "joy");
  assert.equal(joy.ratio, "3:2");
  assert.equal(joy.amber, 0.84);
  assert.equal(lookup({ label: "happy", confidence: 1 }).id, "joy");
  assert.equal(lookup({ label: "energized", confidence: 1 }).id, "energised");
  assert.equal(lookup({ label: "anxiety", confidence: 1 }).id, "anxious");
  for (const label of ["calm", "quiet", "warm", "bright", "excited", "low", "sad", "grief", "tired", "vulnerability", "tense"]) {
    const look = lookup({ sense: true, label, confidence: 1 });
    assert.equal(look.present, true, label);
    assert.equal(look.id, label);
    assert.ok(onSegment(mixRgb(look.amber)));
  }
  const held = lookup({ sense: true, label: "joy", confidence: 1, valence: -1 });
  assert.equal(held.amber, 0.84);
  const quiet = lookup({ sense: true, label: "joy", confidence: 1, arousal: 0 });
  assert.ok(Math.abs(quiet.density - 0.07 * 0.82) < 1e-12);
  const full = lookup({ sense: true, label: "joy", confidence: 1 });
  assert.equal(full.density, 0.07);
});

test("sample is a short deterministic wisp list", () => {
  const a = sample({ sense: true, label: "joy", confidence: 1 }, 0);
  const b = sample({ sense: true, label: "joy", confidence: 1 }, 0);
  assert.deepEqual(a, b);
  assert.equal(a.wisps.length, 3);
  assert.equal(sample({ sense: true, label: "angry", confidence: 1 }, 0).wisps.length, 1);
  assert.ok(a.wisps.every((w) => w.alpha <= 0.14));
  assert.match(a.gradient, /^radial-gradient\(circle, rgba\((\d+),(\d+),(\d+),/);
  const ctx = fakeCtx();
  assert.equal(drawNebula(ctx, { sense: true, label: "calm", confidence: 1 }, 0, {
    cx: 50, cy: 50, inner: 12, outer: 40,
  }), true);
  assert.equal(ctx.calls.filter((c) => c === "fill").length, sample({ sense: true, label: "calm", confidence: 1 }, 0).wisps.length);
  const skipped = fakeCtx();
  assert.equal(drawNebula(skipped, { sense: true, label: "calm", confidence: 1 }, 0, {
    cx: 50, cy: 50, inner: 40, outer: 12,
  }), false);
  assert.deepEqual(skipped.calls, []);
});
