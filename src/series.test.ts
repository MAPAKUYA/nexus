import { test } from "node:test";
import assert from "node:assert/strict";
import { buildSeries } from "./series.ts";

test("week: seven days, gaps filled with zeros, weekday labels from the language", () => {
  const s = buildSeries("week", "2026-10-26", "2026-11-01", [{ key: "2026-10-30", activeMs: 5000 }], "en");
  assert.equal(s.length, 7);
  assert.deepEqual(s.map((p) => p.label), ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]);
  assert.equal(s[4].activeMs, 5000);
  assert.equal(s.filter((p) => p.activeMs > 0).length, 1);
  assert.deepEqual(buildSeries("week", "2026-10-26", "2026-11-01", [], "ru").map((p) => p.label), ["пн", "вт", "ср", "чт", "пт", "сб", "вс"]);
});

test("month: correct number of days including leap February", () => {
  assert.equal(buildSeries("month", "2026-10-01", "2026-10-31", [], "en").length, 31);
  assert.equal(buildSeries("month", "2024-02-01", "2024-02-29", [], "en").length, 29);
  assert.equal(buildSeries("month", "2025-02-01", "2025-02-28", [], "en").length, 28);
  const labels = buildSeries("month", "2026-10-01", "2026-10-31", [], "en").filter((p) => p.showLabel).map((p) => p.label);
  assert.deepEqual(labels, ["1", "5", "10", "15", "20", "25", "30"]);
});

test("year: twelve months with localized short names", () => {
  const s = buildSeries("year", "2026-01-01", "2026-12-31", [{ key: "2026-03", activeMs: 1 }], "en");
  assert.equal(s.length, 12);
  assert.equal(s[2].activeMs, 1);
  assert.equal(s[0].key, "2026-01");
  assert.deepEqual([s[0].label, s[4].label, s[11].label], ["Jan", "May", "Dec"]);
});

test("all: continuous months between first and last, gaps are zero", () => {
  const s = buildSeries("all", "", "", [{ key: "2026-02", activeMs: 3 }, { key: "2025-11", activeMs: 2 }], "en");
  assert.deepEqual(s.map((p) => p.key), ["2025-11", "2025-12", "2026-01", "2026-02"]);
  assert.deepEqual(s.map((p) => p.activeMs), [2, 0, 0, 3]);
});

test("all: long series is labelled once per quarter, first label carries the year", () => {
  const s = buildSeries("all", "", "", [{ key: "2025-02", activeMs: 1 }, { key: "2026-10", activeMs: 1 }], "en");
  assert.equal(s.length, 21);
  assert.equal(s[0].label, "Feb 25");
  assert.deepEqual(s.filter((p) => p.showLabel).map((p) => p.key), ["2025-02", "2025-04", "2025-07", "2025-10", "2026-01", "2026-04", "2026-07", "2026-10"]);
});

test("empty / day / garbage give no series", () => {
  assert.deepEqual(buildSeries("all", "", "", [], "en"), []);
  assert.deepEqual(buildSeries("day", "2026-10-30", "2026-10-30", [], "en"), []);
  assert.deepEqual(buildSeries("week", "x", "y", [], "en"), []);
  assert.deepEqual(buildSeries("year", "bad", "bad", [], "en"), []);
});
