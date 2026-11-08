import { useEffect, useMemo, useState } from "react";
import { getIgnored, ignoreApp, listKnownApps, unignoreApp, type AppRef } from "./api";
import ExcludeConfirm from "./ExcludeConfirm";
import { useI18n } from "./i18n/I18nContext";

/** Раздел настроек «Исключённые приложения»: список, удаление из списка и выбор из известных приложений. */
export default function IgnoredApps() {
  const { t } = useI18n();
  const [ignored, setIgnored] = useState<AppRef[] | null>(null);
  const [picking, setPicking] = useState(false);
  const [known, setKnown] = useState<AppRef[]>([]);
  const [query, setQuery] = useState("");
  const [target, setTarget] = useState<AppRef | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getIgnored()
      .then(setIgnored)
      .catch((e) => setError(String(e)));
  }, []);

  const openPicker = async () => {
    setPicking(true);
    setQuery("");
    try {
      setKnown(await listKnownApps());
    } catch (e) {
      setError(String(e));
    }
  };

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    return known.filter((a) => !q || a.name.toLowerCase().includes(q) || a.exe.includes(q));
  }, [known, query]);

  const confirm = async () => {
    if (!target) return;
    setBusy(true);
    try {
      setIgnored(await ignoreApp(target.exe, target.name));
      setTarget(null);
      setPicking(false);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (app: AppRef) => {
    try {
      setIgnored(await unignoreApp(app.exe));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <>
      <h3 className="section-title">{t.ignored.title}</h3>
      <div className="panel">
        <p className="note">{t.ignored.hint}</p>
        {error && !target && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        {ignored && ignored.length === 0 && <p className="note">{t.ignored.empty}</p>}
        {ignored && ignored.length > 0 && (
          <ul className="ignored-list">
            {ignored.map((a) => (
              <li key={a.exe} className="ignored-item">
                <span className="ignored-text">
                  <span className="ignored-name">{a.name}</span>
                  <span className="ignored-exe">{a.exe}</span>
                </span>
                <button className="link-btn" aria-label={t.ignored.removeAria(a.name)} onClick={() => void remove(a)}>
                  {t.ignored.remove}
                </button>
              </li>
            ))}
          </ul>
        )}

        {picking ? (
          <div className="picker">
            <input
              className="input"
              type="search"
              autoFocus
              placeholder={t.ignored.search}
              aria-label={t.ignored.search}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {matches.length === 0 ? (
              <p className="note">{t.ignored.noMatches}</p>
            ) : (
              <ul className="picker-list">
                {matches.map((a) => (
                  <li key={a.exe}>
                    <button className="picker-item" onClick={() => setTarget(a)}>
                      <span className="ignored-name">{a.name}</span>
                      <span className="ignored-exe">{a.exe}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <button className="link-btn" onClick={() => setPicking(false)}>
              {t.common.cancel}
            </button>
          </div>
        ) : (
          <div className="confirm panel-actions">
            <button className="btn" onClick={() => void openPicker()}>
              {t.ignored.add}
            </button>
          </div>
        )}
      </div>

      <ExcludeConfirm app={target} busy={busy} error={error} onConfirm={() => void confirm()} onCancel={() => setTarget(null)} />
    </>
  );
}
