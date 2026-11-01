import { test } from "node:test";
import assert from "node:assert/strict";
import { buildShares } from "./shares.ts";

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
