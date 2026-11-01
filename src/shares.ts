export interface Share {
  name: string;
  ms: number;
  /** Доля от общего активного времени, 0..1. */
  fraction: number;
  color: string;
  /** Сводная строка «остальные приложения» — её название подставляет интерфейс на своём языке. */
  isOther: boolean;
}

export const SHARE_COLORS = ["#8ea8ff", "#5fd0c4", "#e5b567", "#d68fb6", "#a6c97a"];
export const OTHER_COLOR = "#4a5268";

/** Распределение активного времени: до `max` крупнейших приложений и «Остальные». */
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
