/** Все тексты интерфейса. Каждый язык обязан реализовать этот тип целиком — компилятор не даст пропустить строку. */
export interface Dict {
  /** Единицы длительности; `sep` — разделитель между числом и единицей (у китайского и японского пустой). */
  units: { h: string; min: string; s: string; sep: string };
  periods: { day: string; week: string; month: string; year: string; all: string };
  common: {
    today: string;
    prev: string;
    next: string;
    pickDate: string;
    settings: string;
    backToStats: string;
    allTime: string;
    dismiss: string;
    cancel: string;
  };
  summary: {
    aria: string;
    inFocus: string;
    appsUsed: string;
    appsRunning: string;
    avgPerDay: string;
    shareAria: string;
    others: string;
  };
  chart: {
    byDay: string;
    byMonth: string;
    aria: string;
    max: (duration: string) => string;
    none: string;
    details: string;
    openPeriod: string;
  };
  apps: {
    title: string;
    running: string;
    focus: string;
    emptyToday: string;
    emptyPeriod: string;
    bar: (name: string, active: string, running: string) => string;
  };
  status: {
    starting: string;
    active: (app: string) => string;
    activeNoApp: string;
    idle: string;
    locked: string;
  };
  errors: { load: string };
  notices: { dbRecovered: (file: string) => string };
  settings: {
    title: string;
    language: string;
    languageHint: string;
    countIdle: string;
    countIdleHint: string;
    idleThreshold: string;
    idleThresholdHint: string;
    minutes: (n: number) => string;
    autostart: string;
    autostartHint: string;
    closeToTray: string;
    closeToTrayHint: string;
    dataTitle: string;
    dataNote: (path: string) => string;
    deleteAll: string;
    confirmDelete: string;
    confirmYes: string;
    deleted: string;
  };
}
