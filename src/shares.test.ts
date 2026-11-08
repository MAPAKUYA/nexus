import { test } from "node:test";
import assert from "node:assert/strict";
import { buildShares, shareSlots } from "./shares.ts";

test("top apps plus 'others', fractions sum to 1", () => {
  const apps = Array.from({ length: 8 }, (_, i) => ({ name: `A${i}`, activeMs: (i + 1) * 1000 }));
  const s = buildShares(apps, 5);
  assert.equal(s.length, 6);
  assert.equal(s[0].name, "A7");
  assert.equal(s[5].isOther, true);
  assert.ok(s.slice(0, 5).every((x) => !x.isOther));
  assert.ok(Math.abs(s.reduce((x, y) => x + y.fraction, 0) - 1) < 1e-9);
});

test("apps without active time are ignored; empty gives no shares", () => {
  assert.deepEqual(buildShares([{ name: "x", activeMs: 0 }]), []);
  assert.deepEqual(buildShares([]), []);
  assert.equal(buildShares([{ name: "x", activeMs: 5 }, { name: "y", activeMs: 0 }]).length, 1);
});

test("no 'others' row when everything fits", () => {
  const s = buildShares([{ name: "a", activeMs: 1 }, { name: "b", activeMs: 3 }], 5);
  assert.deepEqual(s.map((x) => x.name), ["b", "a"]);
});

test("share slots keep a fixed order: five apps then others, empty slots are null", () => {
  const apps = Array.from({ length: 7 }, (_, i) => ({ name: `a${i}`, activeMs: (10 - i) * 1000 }));
  const full = shareSlots(buildShares(apps));
  assert.equal(full.length, 6);
  assert.deepEqual(full.slice(0, 5).map((s) => s?.name), ["a0", "a1", "a2", "a3", "a4"]);
  assert.equal(full[5]?.isOther, true);

  const few = shareSlots(buildShares(apps.slice(0, 2)));
  assert.equal(few.length, 6, "слотов всегда шесть — элементы не пересоздаются");
  assert.deepEqual(few.map((s) => s?.name ?? null), ["a0", "a1", null, null, null, null]);

  assert.deepEqual(shareSlots([]), [null, null, null, null, null, null]);
});

test("share slot colors depend only on the slot index", () => {
  const a = shareSlots(buildShares([{ name: "x", activeMs: 5 }, { name: "y", activeMs: 3 }]));
  const b = shareSlots(buildShares([{ name: "p", activeMs: 1 }, { name: "q", activeMs: 9 }]));
  assert.equal(a[0]?.color, b[0]?.color);
  assert.equal(a[1]?.color, b[1]?.color);
});
