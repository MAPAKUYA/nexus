import type { Bucket } from "./api";
import { monthShort, parseDay, weekdayShort } from "./format.ts";

export interface SeriesPoint {
  /** `YYYY-MM-DD` или `YYYY-MM`. */
  key: string;
  /** Короткая подпись под столбцом. */
  label: string;
  /** Показывать ли подпись (чтобы не было каши на длинных рядах). */
  showLabel: boolean;
  activeMs: number;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Все даты от `from` до `to` включительно (не больше 400 — защита от ошибочных границ). */
function eachDay(from: string, to: string): string[] {
  const a = parseDay(from);
  const b = parseDay(to);
  if (!a || !b) return [];
  const out: string[] = [];
  for (let t = a.getTime(); t <= b.getTime() && out.length < 400; t += 86_400_000) {
    out.push(new Date(t).toISOString().slice(0, 10));
  }
  return out;
}

/**
 * Ряд для графика: пропущенные дни/месяцы заполняются нулями, чтобы столбцы стояли на своих местах.
 * Подписи (дни недели, месяцы) строятся правилами языка `locale`. Для периода «день» график не строится.
 */
export function buildSeries(
  period: string,
  from: string,
  to: string,
  buckets: Bucket[],
  locale: string,
  granularity?: "day" | "month",
): SeriesPoint[] {
  const byKey = new Map(buckets.map((b) => [b.key, b.activeMs]));
  const value = (key: string) => byKey.get(key) ?? 0;

  if (period === "week" || period === "month") {
    return eachDay(from, to).map((key) => {
      const date = parseDay(key)!;
      const d = date.getUTCDate();
      return {
        key,
        label: period === "week" ? weekdayShort(date, locale) : String(d),
        showLabel: period === "week" || d === 1 || d % 5 === 0,
        activeMs: value(key),
      };
    });
  }

  if (period === "range") {
    const a = parseDay(from);
    const b = parseDay(to);
    if (!a || !b) return [];
    // Месяцы или дни выбирает ядро (по длине диапазона); без подсказки — по тому же правилу: больше 92 дней.
    const monthly = granularity ? granularity === "month" : Math.round((b.getTime() - a.getTime()) / 86_400_000) + 1 > 92;
    if (!monthly) {
      const days = eachDay(from, to);
      return days.map((key) => {
        const date = parseDay(key)!;
        const d = date.getUTCDate();
        const dense = days.length <= 14;
        return {
          key,
          label: d === 1 && !dense ? monthShort(date, locale) : String(d),
          showLabel: dense || d === 1 || d % 5 === 0,
          activeMs: value(key),
        };
      });
    }
    const y0 = a.getUTCFullYear();
    const m0 = a.getUTCMonth();
    const total = (b.getUTCFullYear() - y0) * 12 + (b.getUTCMonth() - m0) + 1;
    const step = total > 240 ? 10 : total > 120 ? 5 : total > 72 ? 2 : 1; // подписи годов для длинных рядов
    const points: SeriesPoint[] = [];
    for (let i = 0; i < Math.min(total, 700); i++) {
      const idx = y0 * 12 + m0 + i;
      const y = Math.floor(idx / 12);
      const m = (idx % 12) + 1;
      const key = `${y}-${pad(m)}`;
      const short = monthShort(new Date(Date.UTC(y, m - 1, 1)), locale);
      points.push({
        key,
        label: m === 1 || i === 0 ? `${short} ${String(y).slice(2)}` : short,
        showLabel: total <= 12 || i === 0 || (total <= 48 ? (m - 1) % 3 === 0 : m === 1 && y % step === 0),
        activeMs: value(key),
      });
    }
    return points;
  }

  if (period === "year") {
    const y = parseDay(from)?.getUTCFullYear();
    if (y === undefined) return [];
    return Array.from({ length: 12 }, (_, i) => {
      const key = `${y}-${pad(i + 1)}`;
      return { key, label: monthShort(new Date(Date.UTC(y, i, 1)), locale), showLabel: true, activeMs: value(key) };
    });
  }

  if (period === "all") {
    const keys = buckets.map((b) => b.key).filter((k) => /^\d{4}-\d{2}$/.test(k)).sort();
    if (keys.length === 0) return [];
    const [y0, m0] = keys[0].split("-").map(Number);
    const [y1, m1] = keys[keys.length - 1].split("-").map(Number);
    const total = (y1 - y0) * 12 + (m1 - m0) + 1;
    const points: SeriesPoint[] = [];
    for (let i = 0; i < Math.min(total, 600); i++) {
      const idx = y0 * 12 + (m0 - 1) + i;
      const y = Math.floor(idx / 12);
      const m = (idx % 12) + 1;
      const key = `${y}-${pad(m)}`;
      const short = monthShort(new Date(Date.UTC(y, m - 1, 1)), locale);
      points.push({
        key,
        label: m === 1 || i === 0 ? `${short} ${String(y).slice(2)}` : short,
        showLabel: total <= 12 || i === 0 || (m - 1) % 3 === 0, // длинный ряд — подпись раз в квартал
        activeMs: value(key),
      });
    }
    return points;
  }

  return [];
}
