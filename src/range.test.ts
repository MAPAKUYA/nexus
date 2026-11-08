import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MAX_RANGE_DAYS, checkRange, lastDays, thisYear, todayLocal } from "./range.ts";

test("presets include today and count days inclusively", () => {
  assert.deepEqual(lastDays(7, "2026-11-04"), { from: "2026-10-29", to: "2026-11-04" });
  assert.deepEqual(lastDays(1, "2026-11-04"), { from: "2026-11-04", to: "2026-11-04" });
  assert.deepEqual(lastDays(30, "2026-03-05"), { from: "2026-02-04", to: "2026-03-05" });
  assert.deepEqual(lastDays(3, "2024-03-01"), { from: "2024-02-28", to: "2024-03-01" }, "високосный год");
  assert.deepEqual(thisYear("2026-11-04"), { from: "2026-01-01", to: "2026-11-04" });
  assert.equal(lastDays(7, "garbage"), null);
});

test("checkRange swaps reversed dates and rejects garbage", () => {
  assert.deepEqual(checkRange("2025-03-04", "2026-11-04"), { status: "ok", range: { from: "2025-03-04", to: "2026-11-04" } });
  assert.deepEqual(checkRange("2026-11-04", "2025-03-04").range, { from: "2025-03-04", to: "2026-11-04" });
  assert.equal(checkRange("2026-02-30", "2026-03-01").status, "invalid");
  assert.equal(checkRange("", "2026-03-01").status, "invalid");
  assert.equal(checkRange("2026-3-1", "2026-03-02").status, "invalid");
  assert.equal(checkRange("2024-02-29", "2024-02-29").status, "ok");
  assert.equal(checkRange("2023-02-29", "2024-02-29").status, "invalid");
});

test("range length limit", () => {
  assert.equal(checkRange("2026-01-01", "2075-12-31").status, "ok");
  assert.equal(checkRange("2026-01-01", "2077-01-01").status, "tooLong");
});

test("limit matches the Rust core", () => {
  const rust = readFileSync(new URL("../crates/nexus-core/src/report.rs", import.meta.url), "utf8");
  const m = /pub const MAX_RANGE_DAYS: i64 = ([\d_]+);/.exec(rust);
  assert.ok(m, "константа найдена в report.rs");
  assert.equal(Number(m[1].replaceAll("_", "")), MAX_RANGE_DAYS);
});

test("todayLocal uses local calendar fields", () => {
  assert.equal(todayLocal(new Date(2026, 10, 4, 23, 59)), "2026-11-04");
  assert.equal(todayLocal(new Date(2026, 0, 1, 0, 0)), "2026-01-01");
});
