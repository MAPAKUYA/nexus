export interface Share {
  name: string;
  ms: number;
  /** Доля от общего активного времени, 0..1. */
  fraction: number;
  color: string;
  /** Сводная строка «остальные приложения» — её название подставляет интерфейс на своём языке. */
  isOther: boolean;
}

/** Цвета долей — переменные CSS, значения для светлой и тёмной темы заданы в styles.css. */
export const SHARE_COLORS = ["var(--share-1)", "var(--share-2)", "var(--share-3)", "var(--share-4)", "var(--share-5)"];
export const OTHER_COLOR = "var(--share-other)";

/** Распределение активного времени: до `max` крупнейших приложений и «Остальные». */
/**
 * Постоянные «слоты» полосы распределения: пять приложений по порядку и «остальные».
 * Слот всегда один и тот же элемент с тем же цветом — поэтому при смене периода его ширина
 * плавно меняется (в том числе до нуля), а не пересоздаётся.
 */
export function shareSlots(shares: Share[], max = 5): (Share | null)[] {
  const named = shares.filter((s) => !s.isOther);
  const other = shares.find((s) => s.isOther) ?? null;
  return [...Array.from({ length: max }, (_, i) => named[i] ?? null), other];
}

export function buildShares(apps: { name: string; activeMs: number }[], max = 5): Share[] {
  const used = apps.filter((a) => a.activeMs > 0).sort((a, b) => b.activeMs - a.activeMs);
  const total = used.reduce((s, a) => s + a.activeMs, 0);
  if (total <= 0) return [];
  const top = used.slice(0, max);
  const rest = used.slice(max).reduce((s, a) => s + a.activeMs, 0);
  const shares: Share[] = top.map((a, i) => ({
    name: a.name,
    ms: a.activeMs,
    fraction: a.activeMs / total,
    color: SHARE_COLORS[i % SHARE_COLORS.length],
    isOther: false,
  }));
  if (rest > 0) shares.push({ name: "", ms: rest, fraction: rest / total, color: OTHER_COLOR, isOther: true });
  return shares;
}
