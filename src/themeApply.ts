import { motionOn } from "./motion";
import { DEFAULT_THEME, THEME_STORAGE_KEY, isThemeSetting, themeAttribute, type ThemeSetting } from "./theme";

/**
 * Применяет тему к документу и запоминает её для следующего запуска. Безопасно вызывать повторно.
 * `animate` — плавно перекрасить интерфейс (только при смене темы пользователем, не при запуске).
 */
export function applyTheme(setting: ThemeSetting, animate = false): void {
  const attr = themeAttribute(setting);
  const root = document.documentElement;
  if (animate && motionOn()) {
    root.classList.add("theme-fade");
    window.setTimeout(() => root.classList.remove("theme-fade"), 350);
  }
  if (attr) root.dataset.theme = attr;
  else delete root.dataset.theme;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, setting);
  } catch {
    // хранилище недоступно — не страшно, тема применится после загрузки настроек
  }
}

/** Тема, сохранённая при прошлом запуске (до того как настройки загрузятся из базы). */
export function readStoredTheme(): ThemeSetting {
  try {
    const v = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeSetting(v) ? v : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}
