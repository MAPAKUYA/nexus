import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DICTS, LANGUAGES, dictFor, isLang, localeFor } from "./index.ts";
import { en } from "./en.ts";

/** Плоский список путей ключей словаря (функции считаются значениями). */
function paths(obj: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === "object" ? paths(v as Record<string, unknown>, `${prefix}${k}.`) : [`${prefix}${k}:${typeof v}`],
  );
}

const sample = (v: unknown, key: string): string => {
  if (typeof v === "string") return v;
  assert.equal(typeof v, "function", key);
  return (v as (...a: unknown[]) => string)("ARG1", "ARG2", "ARG3");
};

function leaves(obj: Record<string, unknown>, prefix = ""): [string, unknown][] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === "object" ? leaves(v as Record<string, unknown>, `${prefix}${k}.`) : [[`${prefix}${k}`, v] as [string, unknown]],
  );
}

test("every language has exactly the same keys and value types as English", () => {
  const reference = paths(en as unknown as Record<string, unknown>);
  for (const { code } of LANGUAGES) {
    assert.deepEqual(paths(DICTS[code] as unknown as Record<string, unknown>), reference, `язык ${code}`);
  }
});

test("no translation is empty and functions embed their arguments", () => {
  for (const { code } of LANGUAGES) {
    for (const [key, value] of leaves(DICTS[code] as unknown as Record<string, unknown>)) {
      if (key === "units.sep") continue; // разделитель может быть пустым (китайский, японский)
      const text = sample(value, `${code}.${key}`);
      assert.ok(text.trim().length > 0, `${code}.${key} пусто`);
      if (typeof value === "function") {
        const needed = key === "settings.minutes" ? ["ARG1"] : key === "apps.bar" ? ["ARG1", "ARG2", "ARG3"] : ["ARG1"];
        for (const n of needed) assert.ok(text.includes(n), `${code}.${key} теряет аргумент ${n}`);
      }
    }
  }
});

test("non-English languages are actually translated", () => {
  for (const { code } of LANGUAGES.filter((l) => l.code !== "en")) {
    const d = DICTS[code];
    assert.notEqual(d.periods.day, en.periods.day, code);
    assert.notEqual(d.settings.title, en.settings.title, code);
    assert.notEqual(d.apps.emptyToday, en.apps.emptyToday, code);
  }
});

test("language list: English first and default, unique codes, matches the Rust list", () => {
  assert.equal(LANGUAGES[0].code, "en");
  assert.equal(new Set(LANGUAGES.map((l) => l.code)).size, LANGUAGES.length);
  const rust = readFileSync(new URL("../../crates/nexus-core/src/settings.rs", import.meta.url), "utf8");
  const m = /SUPPORTED_LANGUAGES: \[&str; \d+\] = \[([^\]]+)\]/.exec(rust);
  assert.ok(m, "не найден SUPPORTED_LANGUAGES в settings.rs");
  const rustCodes = m[1].split(",").map((s) => s.trim().replace(/"/g, ""));
  assert.deepEqual(LANGUAGES.map((l) => l.code), rustCodes);
});

test("dictFor / localeFor / isLang fall back to English", () => {
  assert.equal(dictFor("xx"), en);
  assert.equal(dictFor(""), en);
  assert.equal(dictFor("ru"), DICTS.ru);
  assert.equal(localeFor("xx"), "en");
  assert.equal(localeFor("pt"), "pt-BR");
  assert.ok(isLang("ja") && !isLang("EN") && !isLang("en-US"));
});

test("tray strings in Rust cover the same languages", () => {
  const rust = readFileSync(new URL("../../crates/nexus-core/src/i18n.rs", import.meta.url), "utf8");
  for (const { code } of LANGUAGES.filter((l) => l.code !== "en")) assert.ok(rust.includes(`"${code}" =>`), `нет «${code}» в tray_text`);
});
