import { createContext, useContext } from "react";
import { DEFAULT_LANG, DICTS, localeFor, type Dict, type Lang } from "./index";

export interface I18n {
  lang: Lang;
  t: Dict;
  locale: string;
}

export const I18nContext = createContext<I18n>({ lang: DEFAULT_LANG, t: DICTS[DEFAULT_LANG], locale: localeFor(DEFAULT_LANG) });

export const useI18n = (): I18n => useContext(I18nContext);
