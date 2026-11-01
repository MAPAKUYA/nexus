import { invoke } from "@tauri-apps/api/core";

export type Period = "day" | "week" | "month" | "year" | "all";

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
export function getReport(period: Period, anchor?: string): Promise<Report> {
  return invoke<Report>("report", { period, anchor: anchor ?? null });
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
  /** Код языка интерфейса (en, ru, …). */
  language: string;
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
