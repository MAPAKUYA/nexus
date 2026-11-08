import { useEffect, useState } from "react";
import type { AppRef } from "./api";
import { useI18n } from "./i18n/I18nContext";
import { LEAVE_MS, motionOn } from "./motion";

interface Props {
  /** Приложение, которое предлагается исключить; `null` — окно закрыто. */
  app: AppRef | null;
  busy: boolean;
  error: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Окно подтверждения исключения приложения: предупреждает, что накопленная статистика будет удалена.
 * Всегда присутствует в разметке родителя: при `app = null` окно плавно исчезает и только потом убирается.
 */
export default function ExcludeConfirm({ app, busy, error, onConfirm, onCancel }: Props) {
  const { t } = useI18n();
  const [shown, setShown] = useState<AppRef | null>(app);
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    if (app) {
      setShown(app);
      setLeaving(false);
      return;
    }
    if (!shown) return;
    if (!motionOn()) {
      setShown(null);
      return;
    }
    setLeaving(true);
    const timer = window.setTimeout(() => {
      setShown(null);
      setLeaving(false);
    }, LEAVE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app]);

  useEffect(() => {
    if (!app) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [app, busy, onCancel]);

  if (!shown) return null;
  return (
    <div className={leaving ? "modal-backdrop modal-leaving" : "modal-backdrop"} onClick={() => !busy && onCancel()}>
      <div
        className="modal"
        role="alertdialog"
        aria-modal="true"
        aria-label={t.ignored.confirmTitle(shown.name)}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="modal-title">{t.ignored.confirmTitle(shown.name)}</h2>
        <p className="modal-exe">{shown.exe}</p>
        <p className="modal-body">{t.ignored.confirmBody}</p>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="modal-actions">
          <button className="btn" onClick={onCancel} disabled={busy} autoFocus>
            {t.common.cancel}
          </button>
          <button className="btn btn-danger" onClick={onConfirm} disabled={busy}>
            {t.ignored.confirmYes}
          </button>
        </div>
      </div>
    </div>
  );
}
