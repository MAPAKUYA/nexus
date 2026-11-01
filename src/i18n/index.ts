import { de } from "./de.ts";
import { en } from "./en.ts";
import { es } from "./es.ts";
import { fr } from "./fr.ts";
import { ja } from "./ja.ts";
import { pt } from "./pt.ts";
import { ru } from "./ru.ts";
import type { Dict } from "./types";
import { zh } from "./zh.ts";

export type { Dict } from "./types";

/** Коды совпадают с `SUPPORTED_LANGUAGES` в `crates/nexus-core/src/settings.rs` (проверяется тестом). */
export type Lang = "en" | "ru" | "es" | "de" | "fr" | "pt" | "zh" | "ja";

export interface LanguageInfo {
  code: Lang;
  /** Название на самом языке — не переводится, чтобы его можно было найти в чужом интерфейсе. */
  name: string;
  /** Региональный код для `Intl` (даты, месяцы, дни недели). */
  locale: string;
}

export const DEFAULT_LANG: Lang = "en";

export const LANGUAGES: LanguageInfo[] = [
  { code: "en", name: "English", locale: "en" },
  { code: "ru", name: "Русский", locale: "ru" },
  { code: "es", name: "Español", locale: "es" },
  { code: "de", name: "Deutsch", locale: "de" },
  { code: "fr", name: "Français", locale: "fr" },
  { code: "pt", name: "Português (Brasil)", locale: "pt-BR" },
  { code: "zh", name: "简体中文", locale: "zh-CN" },
  { code: "ja", name: "日本語", locale: "ja" },
];

export const DICTS: Record<Lang, Dict> = { en, ru, es, de, fr, pt, zh, ja };

export function isLang(code: string): code is Lang {
  return LANGUAGES.some((l) => l.code === code);
}

/** Словарь по коду; неизвестный код даёт английский. */
export function dictFor(code: string): Dict {
  return isLang(code) ? DICTS[code] : en;
}

export function localeFor(code: string): string {
  return LANGUAGES.find((l) => l.code === code)?.locale ?? "en";
}
