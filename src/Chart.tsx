import { formatBucket, formatDuration, ratio } from "./format";
import { useI18n } from "./i18n/I18nContext";
import type { SeriesPoint } from "./series";

interface Props {
  points: SeriesPoint[];
  granularity: "day" | "month";
  onOpen: (key: string) => void;
}

/** Столбчатая диаграмма активности. Каждый столбец — кнопка, открывающая этот день/месяц. */
export default function Chart({ points, granularity, onOpen }: Props) {
  const { t, locale } = useI18n();
  const max = points.reduce((m, p) => Math.max(m, p.activeMs), 0);
  const cols = { gridTemplateColumns: `repeat(${points.length}, minmax(0, 1fr))` };

  return (
    <div className="chart">
      <p className="chart-max">{max > 0 ? t.chart.max(formatDuration(max, t.units)) : t.chart.none}</p>
      <div className="chart-plot" style={cols}>
        {points.map((p) => {
          const text = `${formatBucket(p.key, granularity, locale)}: ${formatDuration(p.activeMs, t.units)}`;
          return (
            <button key={p.key} className="chart-col" title={text} aria-label={text} onClick={() => onOpen(p.key)}>
              <span
                className={p.activeMs > 0 ? "chart-bar" : "chart-bar chart-bar-empty"}
                style={{ height: p.activeMs > 0 ? `${Math.max(3, ratio(p.activeMs, max) * 100)}%` : undefined }}
              />
            </button>
          );
        })}
      </div>
      <div className="chart-axis" style={cols} aria-hidden="true">
        {points.map((p) => (
          <span key={p.key}>{p.showLabel ? p.label : ""}</span>
        ))}
      </div>
    </div>
  );
}
