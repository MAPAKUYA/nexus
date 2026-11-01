import { test } from "node:test";
import assert from "node:assert/strict";
import { capitalize, formatBucket, formatDuration, formatRange, parseDay, ratio } from "./format.ts";

const EN = { h: "h", min: "min", s: "s", sep: " " };
const RU = { h: "ч", min: "мин", s: "с", sep: " " };
const ZH = { h: "小时", min: "分钟", s: "秒", sep: "" };
const labels = { today: "Today", allTime: "All time" };
/** ICU в разных версиях ставит то обычный, то узкий пробел — для сравнения приводим всё к обычному. */
const norm = (s: string) => s.replace(/[\s\u202f\u2009]/g, " ");

test("formatDuration uses units and separator from the dictionary", () => {
  assert.equal(formatDuration(0, EN), "0 s");
  assert.equal(formatDuration(20_000, EN), "20 s");
  assert.equal(formatDuration(45 * 60_000, EN), "45 min");
  assert.equal(formatDuration((3 * 3600 + 12 * 60) * 1000, EN), "3 h 12 min");
  assert.equal(formatDuration(2 * 3600 * 1000, EN), "2 h");
  assert.equal(formatDuration(-5, EN), "0 s");
  assert.equal(formatDuration((3 * 3600 + 12 * 60) * 1000, RU), "3 ч 12 мин");
  assert.equal(formatDuration((3 * 3600 + 12 * 60) * 1000, ZH), "3小时 12分钟");
});

test("ratio is clamped and safe", () => {
  assert.equal(ratio(5, 0), 0);
  assert.equal(ratio(0, 10), 0);
  assert.equal(ratio(20, 10), 1);
  assert.equal(ratio(5, 10), 0.5);
  assert.equal(ratio(NaN, 10), 0);
});

test("parseDay rejects non-existent dates", () => {
  assert.ok(parseDay("2024-02-29"));
  for (const bad of ["2025-02-29", "2026-13-01", "2026-04-31", "garbage", "2026-1-1", ""]) assert.equal(parseDay(bad), null, bad);
});

test("capitalize", () => {
  assert.equal(capitalize("октябрь"), "Октябрь");
  assert.equal(capitalize(""), "");
});

test("formatRange in English", () => {
  assert.equal(formatRange("day", "2026-10-30", "2026-10-30", true, "en", labels), "Today, Oct 30, 2026");
  assert.equal(formatRange("day", "2026-10-29", "2026-10-29", false, "en", labels), "Oct 29, 2026");
  assert.equal(norm(formatRange("week", "2026-10-26", "2026-11-01", true, "en", labels)), "Oct 26 – Nov 1, 2026");
  assert.equal(formatRange("month", "2026-10-01", "2026-10-31", true, "en", labels), "October 2026");
  assert.equal(formatRange("year", "2026-01-01", "2026-12-31", true, "en", labels), "2026");
  assert.equal(formatRange("all", "0000-01-01", "9999-12-31", true, "en", labels), "All time");
});

test("formatRange follows the language rules", () => {
  assert.match(formatRange("month", "2026-10-01", "2026-10-31", true, "ru", labels), /^Октябрь 2026/);
  assert.equal(formatRange("month", "2026-10-01", "2026-10-31", true, "ja", labels), "2026年10月");
  assert.match(formatRange("day", "2026-10-30", "2026-10-30", false, "de", labels), /30\. Okt/);
  assert.match(norm(formatRange("week", "2026-12-28", "2027-01-03", false, "en", labels)), /Dec 28, 2026 – Jan 3, 2027/);
});

test("formatRange survives garbage", () => {
  assert.equal(formatRange("day", "oops", "oops", false, "en", labels), "oops");
  assert.equal(formatRange("month", "oops", "oops", false, "en", labels), "oops");
});

test("formatBucket", () => {
  assert.equal(formatBucket("2026-10-30", "day", "en"), "Fri, October 30");
  assert.equal(formatBucket("2026-10-30", "day", "ru"), "пт, 30 октября");
  assert.equal(formatBucket("2026-10", "month", "en"), "October 2026");
  assert.equal(formatBucket("oops", "day", "en"), "oops");
  assert.equal(formatBucket("2026-99", "month", "en"), "2026-99");
});
