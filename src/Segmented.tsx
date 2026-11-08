import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

export interface SegmentItem<T extends string | number> {
  id: T;
  label: string;
  /** Подсказка при наведении и подпись для экранных дикторов (по умолчанию — label). */
  hint?: string;
}

interface Props<T extends string | number> {
  items: SegmentItem<T>[];
  /** Выбранный элемент; `null` — ни один (подсветка плавно гаснет на месте). */
  value: T | null;
  onSelect: (id: T) => void;
  /** `tabs` — вкладки (role=tablist), `toggle` — группа кнопок. */
  kind: "tabs" | "toggle";
  label?: string;
}

/**
 * Блок кнопок с подсветкой выбранной: подсветка — отдельный элемент, который плавно переезжает
 * от одной кнопки к другой (и гаснет, если выбранной нет). Без анимаций (см. motion.ts) переезд мгновенный.
 */
export default function Segmented<T extends string | number>({ items, value, onSelect, kind, label }: Props<T>) {
  const box = useRef<HTMLDivElement>(null);
  const buttons = useRef(new Map<T, HTMLButtonElement>());
  const placed = useRef(false);
  const [pos, setPos] = useState({ x: 0, w: 0 });
  const [ready, setReady] = useState(false);
  const firstId = items[0]?.id;
  const labelsKey = items.map((i) => i.label).join("|");

  const measure = useCallback(() => {
    // Без выбранной кнопки подсветка остаётся на прежнем месте (и гаснет); при самом первом
    // показе встаёт под первую кнопку, чтобы потом не «выезжать» из угла.
    const id = value ?? (placed.current ? undefined : firstId);
    const el = id === undefined ? undefined : buttons.current.get(id);
    if (!el || el.offsetWidth === 0) return;
    placed.current = true;
    setPos((p) => (p.x === el.offsetLeft && p.w === el.offsetWidth ? p : { x: el.offsetLeft, w: el.offsetWidth }));
  }, [value, firstId]);

  // До первой отрисовки: подсветка сразу встаёт на место, без «выезда».
  useLayoutEffect(measure, [measure, labelsKey]);

  // Размеры меняются при смене языка, шрифтов и размера окна.
  useEffect(() => {
    const el = box.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [measure]);

  // Переходы включаются чуть позже первой отрисовки, чтобы начальная расстановка не анимировалась.
  useEffect(() => {
    const t = window.setTimeout(() => setReady(true), 60);
    return () => window.clearTimeout(t);
  }, []);

  return (
    <div className="tabs" role={kind === "tabs" ? "tablist" : "group"} aria-label={label} ref={box}>
      <span
        className={`tab-pill${value !== null ? " on" : ""}${ready ? " ready" : ""}`}
        style={{ transform: `translateX(${pos.x}px)`, width: pos.w }}
        aria-hidden="true"
      />
      {items.map((it) => {
        const on = it.id === value;
        return (
          <button
            key={it.id}
            ref={(el) => {
              if (el) buttons.current.set(it.id, el);
              else buttons.current.delete(it.id);
            }}
            role={kind === "tabs" ? "tab" : undefined}
            aria-selected={kind === "tabs" ? on : undefined}
            aria-pressed={kind === "toggle" ? on : undefined}
            aria-label={kind === "toggle" ? (it.hint ?? it.label) : undefined}
            title={kind === "toggle" ? (it.hint ?? it.label) : undefined}
            className={on ? "tab tab-on" : "tab"}
            onClick={() => onSelect(it.id)}
          >
            {it.label}
          </button>
        );
      })}
    </div>
  );
}
