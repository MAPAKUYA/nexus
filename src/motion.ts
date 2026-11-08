/**
 * Управление анимациями интерфейса. Все анимации в styles.css включаются только при
 * `data-motion="on"` на `<html>`; единственный переключатель — настройка «Анимации»
 * (системная настройка Windows «Показывать анимацию» намеренно не учитывается — как и в других
 * приложениях). Пока настройки не загрузились, атрибута нет — значит, при запуске ничего не анимируется.
 */

/** Длительность исчезновения окон; должна совпадать с `--leave-ms` в styles.css. */
export const LEAVE_MS = 160;

export function setMotion(enabled: boolean): void {
  document.documentElement.dataset.motion = enabled ? "on" : "off";
}

/** Анимации сейчас включены (настройка «Анимации» в приложении). */
export function motionOn(): boolean {
  return document.documentElement.dataset.motion === "on";
}
