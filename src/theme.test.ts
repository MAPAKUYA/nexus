import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_THEME, THEMES, isThemeSetting, resolveTheme, themeAttribute } from "./theme.ts";

test("system follows Windows, explicit choices ignore it", () => {
  assert.equal(resolveTheme("system", true), "light");
  assert.equal(resolveTheme("system", false), "dark");
  assert.equal(resolveTheme("light", false), "light");
  assert.equal(resolveTheme("dark", true), "dark");
});

test("html attribute is absent for system so CSS media query decides", () => {
  assert.equal(themeAttribute("system"), null);
  assert.equal(themeAttribute("light"), "light");
  assert.equal(themeAttribute("dark"), "dark");
});

test("theme validation and default", () => {
  assert.equal(DEFAULT_THEME, "system");
  for (const t of THEMES) assert.ok(isThemeSetting(t));
  for (const bad of ["", "Dark", "auto", null, undefined, 1]) assert.ok(!isThemeSetting(bad), String(bad));
});
