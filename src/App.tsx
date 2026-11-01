import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getReport,
  getSettings,
  getStatus,
  takeNotice,
  type Notice,
  type Period,
  type Report,
  type Status,
} from "./api";
import Chart from "./Chart";
import { formatBucket, formatDuration, formatRange, ratio } from "./format";
import { I18nContext, useI18n } from "./i18n/I18nContext";
import { DEFAULT_LANG, DICTS, isLang, localeFor, type Lang } from "./i18n";
import { buildSeries } from "./series";
import SettingsView from "./SettingsView";
import { buildShares } from "./shares";

const REFRESH_MS = 2000;
/** Недели, месяцы, годы и «всё время» меняются медленно, а считаются дороже — обновляем реже. */
const REFRESH_SLOW_MS = 10_000;

const PERIOD_IDS: Period[] = ["day", "week", "month", "year", "all"];

/** Загружает язык из настроек и только после этого рисует интерфейс — чтобы не мелькал английский. */
export default function App() {
  const [lang, setLang] = useState<Lang | null>(null);

  useEffect(() => {
    getSettings()
      .then((s) => setLang(isLang(s.language) ? s.language : DEFAULT_LANG))
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
  const [report, setReport] = useState<Report | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const requestId = useRef(0);
  const reportRef = useRef<Report | null>(null);

  const load = useCallback(async () => {
    const id = ++requestId.current;
    try {
      const [r, s] = await Promise.all([getReport(period, anchor), getStatus()]);
      if (id !== requestId.current) return; // пришёл устаревший ответ
      reportRef.current = r;
      setReport(r);
      setStatus(s);
      setError(null);
    } catch (e) {
      if (id === requestId.current) setError(String(e));
    }
  }, [period, anchor]);

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
    setAnchor(report && !report.isCurrent ? report.anchor : undefined);
  };

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

  const apps = report?.apps ?? [];
  const usedCount = useMemo(() => apps.filter((a) => a.activeMs > 0).length, [apps]);
  const maxRunning = useMemo(() => apps.reduce((m, a) => Math.max(m, a.runningMs), 0), [apps]);
  const shares = useMemo(() => buildShares(apps), [apps]);
  const series = useMemo(
    () => (report ? buildSeries(report.period, report.from, report.to, report.buckets, locale) : []),
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
          <button className="link-btn" onClick={() => setView("settings")}>
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
        <SettingsView
          onLanguage={onLanguage}
          onBack={() => {
            setView("stats");
            void load();
          }}
        />
      </main>
    );
  }

  return (
    <main className="shell">
      {header}

      <nav className="toolbar">
        <div className="tabs" role="tablist">
          {PERIOD_IDS.map((id) => (
            <button
              key={id}
              role="tab"
              aria-selected={period === id}
              className={period === id ? "tab tab-on" : "tab"}
              onClick={() => switchPeriod(id)}
            >
              {t.periods[id]}
            </button>
          ))}
        </div>

        {report && period !== "all" && (
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

        {shares.length > 0 && (
          <div className="share">
            <div className="share-bar" role="img" aria-label={t.summary.shareAria}>
              {shares.map((s) => (
                <span
                  key={s.isOther ? "others" : s.name}
                  style={{ width: `${s.fraction * 100}%`, background: s.color }}
                  title={`${s.isOther ? t.summary.others : s.name}: ${fmt(s.ms)}`}
                />
              ))}
            </div>
            <ul className="share-legend">
              {shares.map((s) => (
                <li key={s.name}>
                  <span className="share-dot" style={{ background: s.color }} aria-hidden="true" />
                  <span className="share-name">{s.isOther ? t.summary.others : s.name}</span>
                  <span className="share-pct">{s.fraction < 0.01 ? "<1" : Math.round(s.fraction * 100)}%</span>
                </li>
              ))}
            </ul>
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
              <li key={a.exe} className="row">
                <span className="name" title={a.exe}>
                  {a.name}
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
    </main>
  );
}
