/** Единицы длительности из словаря языка. */
export interface DurationUnits {
  h: string;
  min: string;
  s: string;
  sep: string;
}

/** Длительность для людей: «3 h 12 min», «45 min», «20 s» (единицы и разделитель — из словаря). */
export function formatDuration(ms: number, u: DurationUnits): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const part = (n: number, unit: string) => `${n}${u.sep}${unit}`;
  if (h > 0) return m > 0 ? `${part(h, u.h)} ${part(m, u.min)}` : part(h, u.h);
  if (m > 0) return part(m, u.min);
  return part(s, u.s);
}

/** Доля 0..1 с защитой от деления на ноль и выхода за границы. */
export function ratio(part: number, whole: number): number {
  if (!(whole > 0) || !(part > 0)) return 0;
  return Math.min(1, part / whole);
}

/** Первая буква заглавная (названия месяцев в части языков приходят строчными). */
export function capitalize(s: string): string {
  return s.length > 0 ? s[0].toLocaleUpperCase() + s.slice(1) : s;
}

/** Дата `YYYY-MM-DD` как момент в UTC; несуществующие даты (например, 2026-02-31) дают null. */
export function parseDay(day: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!m) return null;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(Date.UTC(y, mo - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === mo - 1 && date.getUTCDate() === d ? date : null;
}

/** Месяц `YYYY-MM` как первое число месяца в UTC. */
export function parseMonth(key: string): Date | null {
  const m = /^(\d{4})-(\d{2})$/.exec(key);
  return m ? parseDay(`${m[1]}-${m[2]}-01`) : null;
}

/** Все форматы работают в UTC, чтобы часовой пояс компьютера не сдвигал даты. */
function dtf(locale: string, options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  return new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" });
}

export function weekdayShort(date: Date, locale: string): string {
  return dtf(locale, { weekday: "short" }).format(date);
}

export function monthShort(date: Date, locale: string): string {
  return dtf(locale, { month: "short" }).format(date);
}

export interface RangeLabels {
  today: string;
  allTime: string;
}

/** Заголовок периода для панели навигации. */
export function formatRange(
  period: string,
  from: string,
  to: string,
  isCurrent: boolean,
  locale: string,
  labels: RangeLabels,
): string {
  const a = parseDay(from);
  const b = parseDay(to);
  switch (period) {
    case "day":
      return a ? `${isCurrent ? `${labels.today}, ` : ""}${dtf(locale, { day: "numeric", month: "short", year: "numeric" }).format(a)}` : from;
    case "week":
      return a && b ? dtf(locale, { day: "numeric", month: "short", year: "numeric" }).formatRange(a, b) : `${from} – ${to}`;
    case "month":
      return a ? capitalize(dtf(locale, { month: "long", year: "numeric" }).format(a)) : from;
    case "year":
      return a ? dtf(locale, { year: "numeric" }).format(a) : from;
    default:
      return labels.allTime;
  }
}

/** Подпись строки истории: «Fri, October 30» или «October 2026». */
export function formatBucket(key: string, granularity: "day" | "month", locale: string): string {
  if (granularity === "month") {
    const d = parseMonth(key);
    return d ? capitalize(dtf(locale, { month: "long", year: "numeric" }).format(d)) : key;
  }
  const d = parseDay(key);
  return d ? dtf(locale, { weekday: "short", day: "numeric", month: "long" }).format(d) : key;
}
