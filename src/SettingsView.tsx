import { useEffect, useState } from "react";
import { clearStatistics, getSettings, saveSettings, type SettingsData } from "./api";
import { useI18n } from "./i18n/I18nContext";
import { LANGUAGES, isLang, type Lang } from "./i18n";

const IDLE_OPTIONS = [1, 2, 3, 5, 10, 15, 30, 60];
const DB_PATH = "%APPDATA%\\app.nexus.tracker\\nexus.sqlite3";

interface Props {
  onBack: () => void;
  /** Вызывается сразу при выборе языка — интерфейс переключается без перезапуска. */
  onLanguage: (lang: Lang) => void;
}

export default function SettingsView({ onBack, onLanguage }: Props) {
  const { t, lang } = useI18n();
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [cleared, setCleared] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

  const update = async (patch: Partial<SettingsData>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next); // сразу показываем выбор, при ошибке откатим
    if (patch.language && isLang(patch.language)) onLanguage(patch.language);
    try {
      setSettings(await saveSettings(next));
      setError(null);
    } catch (e) {
      setSettings(settings);
      if (patch.language && isLang(settings.language)) onLanguage(settings.language);
      setError(String(e));
    }
  };

  const doClear = async () => {
    try {
      await clearStatistics();
      setConfirmClear(false);
      setCleared(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const idleOptions =
    settings && !IDLE_OPTIONS.includes(settings.idleMinutes)
      ? [...IDLE_OPTIONS, settings.idleMinutes].sort((a, b) => a - b)
      : IDLE_OPTIONS;

  return (
    <section className="settings" aria-label={t.settings.title}>
      <div className="settings-head">
        <h2 className="settings-title">{t.settings.title}</h2>
        <button className="link-btn" onClick={onBack}>
          {t.common.backToStats}
        </button>
      </div>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {settings && (
        <div className="panel">
          <label className="setting">
            <span className="setting-text">
              <span className="setting-name">{t.settings.language}</span>
              <span className="setting-hint">{t.settings.languageHint}</span>
            </span>
            <select className="select" value={lang} onChange={(e) => void update({ language: e.target.value })}>
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>
                  {l.name}
                </option>
              ))}
            </select>
          </label>

          <label className="setting">
            <span className="setting-text">
              <span className="setting-name">{t.settings.countIdle}</span>
              <span className="setting-hint">{t.settings.countIdleHint}</span>
            </span>
            <input
              className="check"
              type="checkbox"
              checked={settings.countIdle}
              onChange={(e) => void update({ countIdle: e.target.checked })}
            />
          </label>

          <label className={settings.countIdle ? "setting setting-off" : "setting"}>
            <span className="setting-text">
              <span className="setting-name">{t.settings.idleThreshold}</span>
              <span className="setting-hint">{t.settings.idleThresholdHint}</span>
            </span>
            <select
              className="select"
              disabled={settings.countIdle}
              value={settings.idleMinutes}
              onChange={(e) => void update({ idleMinutes: Number(e.target.value) })}
            >
              {idleOptions.map((m) => (
                <option key={m} value={m}>
                  {t.settings.minutes(m)}
                </option>
              ))}
            </select>
          </label>

          <label className="setting">
            <span className="setting-text">
              <span className="setting-name">{t.settings.autostart}</span>
              <span className="setting-hint">{t.settings.autostartHint}</span>
            </span>
            <input
              className="check"
              type="checkbox"
              checked={settings.autostart}
              onChange={(e) => void update({ autostart: e.target.checked })}
            />
          </label>

          <label className="setting">
            <span className="setting-text">
              <span className="setting-name">{t.settings.closeToTray}</span>
              <span className="setting-hint">{t.settings.closeToTrayHint}</span>
            </span>
            <input
              className="check"
              type="checkbox"
              checked={settings.closeToTray}
              onChange={(e) => void update({ closeToTray: e.target.checked })}
            />
          </label>
        </div>
      )}

      <h3 className="section-title">{t.settings.dataTitle}</h3>
      <div className="panel">
        <p className="note">{t.settings.dataNote(DB_PATH)}</p>
        {confirmClear ? (
          <div className="confirm" role="alertdialog" aria-label={t.settings.confirmDelete}>
            <span>{t.settings.confirmDelete}</span>
            <button className="btn btn-danger" onClick={() => void doClear()}>
              {t.settings.confirmYes}
            </button>
            <button className="btn" onClick={() => setConfirmClear(false)}>
              {t.common.cancel}
            </button>
          </div>
        ) : (
          <div className="confirm">
            <button
              className="btn"
              onClick={() => {
                setConfirmClear(true);
                setCleared(false);
              }}
            >
              {t.settings.deleteAll}
            </button>
            {cleared && <span className="note">{t.settings.deleted}</span>}
          </div>
        )}
      </div>

      <p className="about">NEXUS {__APP_VERSION__}</p>
    </section>
  );
}
