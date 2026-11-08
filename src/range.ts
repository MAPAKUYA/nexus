/** Произвольный диапазон дат: чистые функции без обращений к DOM (проверяются тестами). */

/** Должен совпадать с `MAX_RANGE_DAYS` в crates/nexus-core/src/report.rs (это проверяет тест). */
export const MAX_RANGE_DAYS = 18_300;

export interface DateRange {
  from: string;
  to: string;
}

function parse(s: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return null;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(Date.UTC(y, mo - 1, d));
  // Date.UTC «чинит» несуществующие даты (30 февраля → 2 марта) — такие отвергаем.
  const ok = date.getUTCFullYear() === y && date.getUTCMonth() === mo - 1 && date.getUTCDate() === d;
  return ok && y >= 1900 ? date : null;
}

function iso(d: Date): string {
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${p(d.getUTCFullYear(), 4)}-${p(d.getUTCMonth() + 1)}-${p(d.getUTCDate())}`;
}

/** Сегодняшняя дата по местному времени компьютера в виде YYYY-MM-DD. */
export function todayLocal(now: Date = new Date()): string {
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${p(now.getFullYear(), 4)}-${p(now.getMonth() + 1)}-${p(now.getDate())}`;
}

/** Последние `n` дней, включая сегодняшний. */
export function lastDays(n: number, today: string): DateRange | null {
  const t = parse(today);
  if (!t || n < 1) return null;
  return { from: iso(new Date(t.getTime() - (n - 1) * 86_400_000)), to: today };
}

/** С 1 января текущего года по сегодня. */
export function thisYear(today: string): DateRange | null {
  const t = parse(today);
  return t ? { from: `${String(t.getUTCFullYear()).padStart(4, "0")}-01-01`, to: today } : null;
}

export type RangeCheck = "ok" | "invalid" | "tooLong";

/** Проверка и упорядочивание: перепутанные даты меняются местами. */
export function checkRange(from: string, to: string): { status: RangeCheck; range: DateRange | null } {
  const a = parse(from);
  const b = parse(to);
  if (!a || !b) return { status: "invalid", range: null };
  const [lo, hi] = a <= b ? [a, b] : [b, a];
  const days = Math.round((hi.getTime() - lo.getTime()) / 86_400_000);
  if (days >= MAX_RANGE_DAYS) return { status: "tooLong", range: null };
  return { status: "ok", range: { from: iso(lo), to: iso(hi) } };
}
