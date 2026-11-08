import { invoke } from "@tauri-apps/api/core";

export type Period = "day" | "week" | "month" | "year" | "all" | "range";

export interface AppUsage {
  exe: string;
  name: string;
  /** Время в фокусе, мс. */
  activeMs: number;
  /** Время работы приложения, мс. */
  runningMs: number;
}

export interface Bucket {
  /** `YYYY-MM-DD` или `YYYY-MM` в зависимости от granularity. */
  key: string;
  activeMs: number;
}

export interface Report {
  period: Period;
  from: string;
  to: string;
  anchor: string;
  prev: string | null;
  next: string | null;
  isCurrent: boolean;
  granularity: "day" | "month";
  apps: AppUsage[];
  buckets: Bucket[];
  totalActiveMs: number;
}

export interface Status {
  kind: "starting" | "active" | "idle" | "locked";
  app: string | null;
}

/** `anchor` в формате YYYY-MM-DD; без него — текущий период. */
export function getReport(period: Period, anchor?: string, range?: { from: string; to: string }): Promise<Report> {
  return invoke<Report>("report", { period, anchor: anchor ?? null, from: range?.from ?? null, to: range?.to ?? null });
}

export function getStatus(): Promise<Status> {
  return invoke<Status>("current_status");
}

export function clearStatistics(): Promise<void> {
  return invoke<void>("clear_statistics");
}

export interface SettingsData {
  /** Через сколько минут без ввода пользователь считается ушедшим. */
  idleMinutes: number;
  /** Кнопка закрытия окна прячет приложение в трей. */
  closeToTray: boolean;
  /** Засчитывать время без ввода как активное (простой не определяется). */
  countIdle: boolean;
  /** Показывать кнопку «исключить» в списке приложений. */
  quickExclude: boolean;
  /** Показывать иконки приложений слева от названия. */
  showIcons: boolean;
  /** Плавные анимации интерфейса. */
  animations: boolean;
  /** Код языка интерфейса (en, ru, …). */
  language: string;
  /** Тема интерфейса: system | light | dark. */
  theme: string;
  /** Запуск вместе с Windows (свёрнутым в трей). */
  autostart: boolean;
}

export function getSettings(): Promise<SettingsData> {
  return invoke<SettingsData>("get_settings");
}

export function saveSettings(settings: SettingsData): Promise<SettingsData> {
  return invoke<SettingsData>("save_settings", { settings });
}

/** Однократное сообщение приложения; текст по `code` подбирает интерфейс на выбранном языке. */
export interface Notice {
  code: string;
  detail: string;
}

export function takeNotice(): Promise<Notice | null> {
  return invoke<Notice | null>("take_notice");
}

/** Приложение в списках настроек: `exe` — имя файла в нижнем регистре, `name` — читаемое имя. */
export interface AppRef {
  exe: string;
  name: string;
}

export function getIgnored(): Promise<AppRef[]> {
  return invoke<AppRef[]>("get_ignored");
}

/** Приложения, которые можно исключить (уже исключённые не возвращаются). */
export function listKnownApps(): Promise<AppRef[]> {
  return invoke<AppRef[]>("list_known_apps");
}

/** Исключает приложение и **безвозвратно удаляет** всю его накопленную статистику. */
export function ignoreApp(exe: string, name: string): Promise<AppRef[]> {
  return invoke<AppRef[]>("ignore_app", { exe, name });
}

export function unignoreApp(exe: string): Promise<AppRef[]> {
  return invoke<AppRef[]>("unignore_app", { exe });
}

/** Иконки приложений как `data:image/png;base64,…`; приложения без иконки в ответ не попадают. */
export function getIcons(exes: string[]): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("get_icons", { exes });
}
