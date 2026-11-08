import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getReport,
  getIcons,
  getSettings,
  getStatus,
  ignoreApp,
  takeNotice,
  type AppRef,
  type Notice,
  type Period,
  type Report,
  type Status,
} from "./api";
import Chart from "./Chart";
import ExcludeConfirm from "./ExcludeConfirm";
import { setMotion } from "./motion";
import Segmented from "./Segmented";
import { formatBucket, formatDuration, formatRange, ratio } from "./format";
import { I18nContext, useI18n } from "./i18n/I18nContext";
import { DEFAULT_LANG, DICTS, isLang, localeFor, type Lang } from "./i18n";
import { checkRange, lastDays, todayLocal, type DateRange } from "./range";
import { buildSeries } from "./series";
import SettingsView from "./SettingsView";
import { buildShares, OTHER_COLOR, SHARE_COLORS, shareSlots } from "./shares";
import { isThemeSetting } from "./theme";
import { applyTheme } from "./themeApply";

const REFRESH_MS = 2000;
/** Недели, месяцы, годы и «всё время» меняются медленно, а считаются дороже — обновляем реже. */
const REFRESH_SLOW_MS = 10_000;

const PERIOD_IDS: Period[] = ["day", "week", "month", "year", "all", "range"];
const PRESET_DAYS = [7, 30, 90] as const;

/** Загружает язык из настроек и только после этого рисует интерфейс — чтобы не мелькал английский. */
export default function App() {
  const [lang, setLang] = useState<Lang | null>(null);

  useEffect(() => {
    getSettings()
      .then((s) => {
        applyTheme(isThemeSetting(s.theme) ? s.theme : "system");
        setMotion(s.animations !== false);
        setLang(isLang(s.language) ? s.language : DEFAULT_LANG);
      })
      .catch(() => setLang(DEFAULT_LANG));
  }, []);

  useEffect(() => {
    if (lang) document.documentElement.lang = localeFor(lang);
  }, [lang]);

  if (!lang) return null;
  return (
    <I18nContext.Provider value={{ lang, t: DICTS[lang], locale: localeFor(lang) }}>
      <Main onLanguage={setLang} />
    </I18nContext.Provider>
  );
}

function Main({ onLanguage }: { onLanguage: (lang: Lang) => void }) {
  const { t, locale } = useI18n();
  const [view, setView] = useState<"stats" | "settings">("stats");
  const [period, setPeriod] = useState<Period>("day");
  const [anchor, setAnchor] = useState<string | undefined>(undefined);
  const [range, setRange] = useState<DateRange>(() => lastDays(30, todayLocal()) ?? { from: todayLocal(), to: todayLocal() });
  const [draft, setDraft] = useState<DateRange>(range); // что показано в полях ввода (может быть неполным)
  const [rangeError, setRangeError] = useState<"invalid" | "tooLong" | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [excluding, setExcluding] = useState<AppRef | null>(null);
  const [excludeBusy, setExcludeBusy] = useState(false);
  const [excludeError, setExcludeError] = useState<string | null>(null);
  const [quickExclude, setQuickExclude] = useState(false);
  const [showIcons, setShowIcons] = useState(false);
  const [viewAnim, setViewAnim] = useState(false); // экран меняли действием пользователя — можно плавно показать новый
  const [freshIcons, setFreshIcons] = useState<Set<string>>(new Set());
  const [icons, setIcons] = useState<Record<string, string>>({});
  const iconsRef = useRef<Record<string, string>>({});

  // Кнопка «исключить» в списке включается в настройках; значение перечитывается при возврате из настроек.
  useEffect(() => {
    if (view !== "stats") return;
    getSettings()
      .then((s) => {
        setQuickExclude(Boolean(s.quickExclude));
        setShowIcons(Boolean(s.showIcons));
      })
      .catch(() => {});
  }, [view]);
  const requestId = useRef(0);
  const reportRef = useRef<Report | null>(null);

  const load = useCallback(async () => {
    const id = ++requestId.current;
    try {
      const [r, s] = await Promise.all([getReport(period, anchor, range), getStatus()]);
      if (id !== requestId.current) return; // пришёл устаревший ответ
      reportRef.current = r;
      setReport(r);
      setStatus(s);
      setError(null);
    } catch (e) {
      if (id === requestId.current) setError(String(e));
    }
  }, [period, anchor, range]);

  useEffect(() => {
    takeNotice().then(setNotice).catch(() => undefined);
  }, []);

  useEffect(() => {
    void load();
    // Обновляем только пока окно видно — в трее приложение ничего не опрашивает. Завершённый период
    // уже не меняется, поэтому для него обновляется только индикатор состояния.
    let tick = 0;
    const timer = window.setInterval(() => {
      if (document.hidden) return;
      tick += 1;
      const current = reportRef.current;
      if (current && !current.isCurrent) {
        void getStatus().then(setStatus).catch(() => undefined);
      } else if (period === "day" || tick % (REFRESH_SLOW_MS / REFRESH_MS) === 0) {
        void load();
      } else {
        void getStatus().then(setStatus).catch(() => undefined);
      }
    }, REFRESH_MS);
    const onVisible = () => !document.hidden && void load();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [load, period]);

  const switchPeriod = (p: Period) => {
    setPeriod(p);
    setAnchor(p !== "range" && report && !report.isCurrent && report.period !== "range" ? report.anchor : undefined);
  };

  /** Задаёт диапазон из полей ввода или пресета; неверный ввод не отправляется в ядро. */
  const applyRange = (from: string, to: string) => {
    const { status, range: ok } = checkRange(from, to);
    if (ok) {
      setRangeError(null);
      setDraft(ok); // перепутанные даты в полях тоже меняются местами
      setRange(ok);
    } else {
      // Поля показывают введённое, но в ядро неверный диапазон не отправляется.
      setDraft({ from, to });
      setRangeError(status === "tooLong" ? "tooLong" : "invalid");
    }
  };
  const applyPreset = (r: DateRange | null) => r && applyRange(r.from, r.to);
  // Какой из пресетов 7 / 30 / 90 совпадает с выбранным диапазоном (если вручную — ни один).
  const activePreset =
    PRESET_DAYS.find((n) => {
      const p = lastDays(n, todayLocal());
      return !!p && p.from === range.from && p.to === range.to;
    }) ?? null;

  const openBucket = (key: string) => {
    if (!report) return;
    if (report.granularity === "month") {
      setPeriod("month");
      setAnchor(`${key}-01`);
    } else {
      setPeriod("day");
      setAnchor(key);
    }
  };

  const confirmExclude = async () => {
    if (!excluding) return;
    setExcludeBusy(true);
    try {
      await ignoreApp(excluding.exe, excluding.name);
      setExcluding(null);
      setExcludeError(null);
      void load(); // список обновляется сразу, не дожидаясь опроса
    } catch (e) {
      setExcludeError(String(e));
    } finally {
      setExcludeBusy(false);
    }
  };

  const apps = report?.apps ?? [];

  // Иконки запрашиваются только при включённой настройке и только для тех приложений, что в списке.
  // Новая иконка появляется в базе через несколько секунд после первого замера, поэтому недостающие
  // запрашиваются ещё раз (через 6, 15 и 30 секунд).
  const appKey = apps.map((a) => a.exe).join("|");
  useEffect(() => {
    if (!showIcons || !appKey) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;
    const missing = () => appKey.split("|").filter((e) => !iconsRef.current[e]);
    const run = async () => {
      const want = missing().slice(0, 200);
      if (want.length === 0) return;
      try {
        const got = await getIcons(want);
        if (cancelled) return;
        if (Object.keys(got).length > 0) {
          const arrived = Object.keys(got).filter((k) => !iconsRef.current[k]);
          iconsRef.current = { ...iconsRef.current, ...got };
          setIcons(iconsRef.current);
          setFreshIcons(new Set(arrived)); // только что полученные иконки проявляются плавно
          window.setTimeout(() => setFreshIcons(new Set()), 400);
        }
      } catch {
        // иконки — украшение: ошибка не должна мешать статистике
      }
      attempt += 1;
      if (!cancelled && attempt <= 3 && missing().length > 0) {
        timer = setTimeout(() => void run(), [6000, 15000, 30000][attempt - 1]);
      }
    };
    void run();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [showIcons, appKey]);
  const usedCount = useMemo(() => apps.filter((a) => a.activeMs > 0).length, [apps]);
  const maxRunning = useMemo(() => apps.reduce((m, a) => Math.max(m, a.runningMs), 0), [apps]);
  const shares = useMemo(() => buildShares(apps), [apps]);
  const series = useMemo(
    () => (report ? buildSeries(report.period, report.from, report.to, report.buckets, locale, report.granularity) : []),
    [report, locale],
  );
  const dayCount = report && report.granularity === "day" ? report.buckets.length : 0;
  const maxBucket = (report?.buckets ?? []).reduce((m, b) => Math.max(m, b.activeMs), 0);

  const statusText = (st: Status): string => {
    switch (st.kind) {
      case "active":
        return st.app ? t.status.active(st.app) : t.status.activeNoApp;
      case "idle":
        return t.status.idle;
      case "locked":
        return t.status.locked;
      default:
        return t.status.starting;
    }
  };
  const fmt = (ms: number) => formatDuration(ms, t.units);

  const header = (
    <header className="top">
      <h1 className="brand">NEXUS</h1>
      <div className="top-right">
        {status && (
          <p className={`status status-${status.kind}`} aria-live="polite">
            <span className="dot" aria-hidden="true" />
            {statusText(status)}
          </p>
        )}
        {view === "stats" && (
          <button
            className="link-btn"
            onClick={() => {
              setViewAnim(true);
              setView("settings");
            }}
          >
            {t.common.settings}
          </button>
        )}
      </div>
    </header>
  );

  if (view === "settings") {
    return (
      <main className="shell">
        {header}
        <div className={viewAnim ? "view-fade" : undefined}>
          <SettingsView
            onLanguage={onLanguage}
            onBack={() => {
              setViewAnim(true);
              setView("stats");
              void load();
            }}
          />
        </div>
      </main>
    );
  }

  return (
    <main className="shell">
      {header}

      <div className={viewAnim ? "view-fade" : undefined}>
        <nav className="toolbar">
          <Segmented
            kind="tabs"
            items={PERIOD_IDS.map((id) => ({ id, label: t.periods[id] }))}
            value={period}
            onSelect={switchPeriod}
          />

          {report && period !== "all" && period !== "range" && (
            <div className="nav">
              <button className="icon-btn" aria-label={t.common.prev} disabled={!report.prev} onClick={() => setAnchor(report.prev ?? undefined)}>
                ‹
              </button>
              <span className="range">{formatRange(report.period, report.from, report.to, report.isCurrent, locale, {
                  today: t.common.today,
                  allTime: t.common.allTime,
                })}</span>
              <button className="icon-btn" aria-label={t.common.next} disabled={!report.next} onClick={() => setAnchor(report.next ?? undefined)}>
                ›
              </button>
              <input
                className="date-input"
                type="date"
                aria-label={t.common.pickDate}
                value={report.anchor}
                min="1900-01-01"
                max="9999-12-31"
                onChange={(e) => e.target.value && setAnchor(e.target.value)}
              />
              <button className="link-btn" disabled={report.isCurrent} onClick={() => setAnchor(undefined)}>
                {t.common.today}
              </button>
            </div>
          )}
          {report && period === "all" && <span className="range">{t.common.allTime}</span>}
          {period === "range" && (
            <div className="nav range-controls">
              <label className="range-field">
                <span>{t.rangePicker.from}</span>
                <input
                  className="date-input"
                  type="date"
                  value={draft.from}
                  min="1900-01-01"
                  max="9999-12-31"
                  onChange={(e) => applyRange(e.target.value, draft.to)}
                />
              </label>
              <label className="range-field">
                <span>{t.rangePicker.to}</span>
                <input
                  className="date-input"
                  type="date"
                  value={draft.to}
                  min="1900-01-01"
                  max="9999-12-31"
                  onChange={(e) => applyRange(draft.from, e.target.value)}
                />
              </label>
              {/* Быстрый выбор: последние 7 / 30 / 90 дней (пресет «Этот год» есть в range.ts, но пока скрыт). */}
              <Segmented
                kind="toggle"
                label={t.rangePicker.presets}
                items={PRESET_DAYS.map((n) => ({
                  id: n,
                  label: String(n),
                  hint: n === 7 ? t.rangePicker.last7 : n === 30 ? t.rangePicker.last30 : t.rangePicker.last90,
                }))}
                value={activePreset}
                onSelect={(n) => applyPreset(lastDays(n, todayLocal()))}
              />
              {rangeError === "tooLong" && (
                <span className="error" role="alert">
                  {t.rangePicker.tooLong}
                </span>
              )}
            </div>
          )}
        </nav>

        <section className="summary" aria-label={t.summary.aria}>
          <dl>
            <div>
              <dt>{t.summary.inFocus}</dt>
              <dd>{fmt(report?.totalActiveMs ?? 0)}</dd>
            </div>
            <div>
              <dt>{t.summary.appsUsed}</dt>
              <dd>{usedCount}</dd>
            </div>
            <div>
              <dt>{t.summary.appsRunning}</dt>
              <dd>{apps.length}</dd>
            </div>
            {dayCount > 1 && report && (
              <div>
                <dt>{t.summary.avgPerDay}</dt>
                <dd>{fmt(report.totalActiveMs / dayCount)}</dd>
              </div>
            )}
          </dl>

          {report && (
            <div className="share">
              <div className="share-bar" role="img" aria-label={t.summary.shareAria}>
                {shareSlots(shares).map((s, i) => (
                  <span
                    key={i}
                    style={{
                      width: s ? `${s.fraction * 100}%` : "0%",
                      minWidth: s ? 3 : 0,
                      background: i < 5 ? SHARE_COLORS[i] : OTHER_COLOR,
                    }}
                    title={s ? `${s.isOther ? t.summary.others : s.name}: ${fmt(s.ms)}` : undefined}
                  />
                ))}
              </div>
              {shares.length > 0 && (
                <ul className="share-legend">
                  {shares.map((s) => (
                    <li key={s.name}>
                      <span className="share-dot" style={{ background: s.color }} aria-hidden="true" />
                      <span className="share-name">{s.isOther ? t.summary.others : s.name}</span>
                      <span className="share-pct">{s.fraction < 0.01 ? "<1" : Math.round(s.fraction * 100)}%</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </section>

        {notice && (
          <p className="notice" role="status">
            {notice.code === "db_recovered" ? t.notices.dbRecovered(notice.detail) : notice.detail}{" "}
            <button className="link-btn" onClick={() => setNotice(null)}>
              {t.common.dismiss}
            </button>
          </p>
        )}

        {error && (
          <p className="error" role="alert">
            {t.errors.load} {error}
          </p>
        )}

        {report && series.length > 0 && (
          <section className="chart-section" aria-label={t.chart.aria}>
            <h2 className="section-title">{report.granularity === "month" ? t.chart.byMonth : t.chart.byDay}</h2>
            <Chart points={series} granularity={report.granularity} onOpen={openBucket} />
            {report.buckets.length > 0 && (
              <details className="history">
                <summary>{t.chart.details}</summary>
                <ul className="list">
                  {report.buckets.map((b) => (
                    <li key={b.key}>
                      <button className="row row-btn" onClick={() => openBucket(b.key)} title={t.chart.openPeriod}>
                        <span className="name">{formatBucket(b.key, report.granularity, locale)}</span>
                        <span className="bar" aria-hidden="true">
                          <span className="bar-running">
                            <span className="bar-active" style={{ width: `${ratio(b.activeMs, maxBucket) * 100}%` }} />
                          </span>
                        </span>
                        <span className="time time-active">{fmt(b.activeMs)}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </details>
            )}
          </section>
        )}

        <section className="apps" aria-label={t.apps.title}>
          <h2 className="section-title">{t.apps.title}</h2>
          <div className="legend">
            <span className="key key-running" /> {t.apps.running}
            <span className="key key-active" /> {t.apps.focus}
          </div>

          {apps.length === 0 && !error ? (
            <p className="empty">
              {report?.period === "day" && report.isCurrent ? t.apps.emptyToday : t.apps.emptyPeriod}
            </p>
          ) : (
            <ul className="list">
              {apps.map((a) => (
                <li key={a.exe} className={showIcons ? "row row-icons" : "row"}>
                  <span className="name-cell">
                    {showIcons &&
                      (icons[a.exe] ? (
                        <img className={freshIcons.has(a.exe) ? "app-icon app-icon-in" : "app-icon"} src={icons[a.exe]} alt="" width={20} height={20} draggable={false} />
                      ) : (
                        <span className="app-icon app-icon-empty" aria-hidden="true" />
                      ))}
                    <span className="name" title={a.exe}>
                      {a.name}
                    </span>
                    {quickExclude && (
                      <button
                        className="row-exclude"
                        title={t.apps.exclude}
                        aria-label={t.apps.excludeAria(a.name)}
                        onClick={() => {
                          setExcludeError(null);
                          setExcluding({ exe: a.exe, name: a.name });
                        }}
                      >
                        ⊘
                      </button>
                    )}
                  </span>
                  <span
                    className="bar"
                    role="img"
                    aria-label={t.apps.bar(a.name, fmt(a.activeMs), fmt(a.runningMs))}
                  >
                    <span className="bar-running" style={{ width: `${ratio(a.runningMs, maxRunning) * 100}%` }}>
                      <span className="bar-active" style={{ width: `${ratio(a.activeMs, a.runningMs) * 100}%` }} />
                    </span>
                  </span>
                  <span className="time time-active">{fmt(a.activeMs)}</span>
                  <span className="time time-running">{fmt(a.runningMs)}</span>
                </li>
              ))}
            </ul>
          )}
        </section>

        <ExcludeConfirm
          app={excluding}
          busy={excludeBusy}
          error={excludeError}
          onConfirm={() => void confirmExclude()}
          onCancel={() => setExcluding(null)}
        />
      </div>
    </main>
  );
}
