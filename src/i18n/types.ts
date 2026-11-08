/** Все тексты интерфейса. Каждый язык обязан реализовать этот тип целиком — компилятор не даст пропустить строку. */
export interface Dict {
  /** Единицы длительности; `sep` — разделитель между числом и единицей (у китайского и японского пустой). */
  units: { h: string; min: string; s: string; sep: string };
  periods: { day: string; week: string; month: string; year: string; all: string; range: string };
  /** Выбор произвольного диапазона дат. */
  rangePicker: {
    from: string;
    to: string;
    last7: string;
    last30: string;
    last90: string;
    /** Подпись группы кнопок 7 / 30 / 90 (для экранных дикторов). */
    presets: string;
    thisYear: string;
    tooLong: string;
  };
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
    exclude: string;
    excludeAria: (name: string) => string;
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
    theme: string;
    themeHint: string;
    themeNames: { system: string; light: string; dark: string };
    showIcons: string;
    showIconsHint: string;
    animations: string;
    animationsHint: string;
    quickExclude: string;
    quickExcludeHint: string;
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
  /** Чёрный список приложений: время в них не учитывается, накопленное удаляется. */
  ignored: {
    title: string;
    hint: string;
    empty: string;
    add: string;
    search: string;
    noMatches: string;
    remove: string;
    removeAria: (name: string) => string;
    confirmTitle: (name: string) => string;
    confirmBody: string;
    confirmYes: string;
  };
}
