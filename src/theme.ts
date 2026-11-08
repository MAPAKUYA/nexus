/** Тема интерфейса. `system` — следовать теме Windows (это делает сам CSS через prefers-color-scheme). */
export type ThemeSetting = "system" | "light" | "dark";

export const THEMES: readonly ThemeSetting[] = ["system", "light", "dark"];
export const DEFAULT_THEME: ThemeSetting = "system";

/** Ключ копии выбранной темы в localStorage — нужен только чтобы применить её до первой отрисовки. */
export const THEME_STORAGE_KEY = "nexus-theme";

export function isThemeSetting(value: unknown): value is ThemeSetting {
  return typeof value === "string" && (THEMES as readonly string[]).includes(value);
}

/** Какая тема фактически показывается: `system` зависит от темы Windows. */
export function resolveTheme(setting: ThemeSetting, systemPrefersLight: boolean): "light" | "dark" {
  if (setting === "system") return systemPrefersLight ? "light" : "dark";
  return setting;
}

/** Значение атрибута `data-theme` на `<html>`; `null` — атрибута нет, тему выбирает CSS по системной. */
export function themeAttribute(setting: ThemeSetting): "light" | "dark" | null {
  return setting === "system" ? null : setting;
}
