#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod platform;

use nexus_core::{
    base64_encode, build_range_report, build_report, display_name, is_supported_language, normalize_exe, tray_text, AppRef, Date, Db, Period, Report,
    parse_range, Settings, Status, Tracker, TrackerConfig,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, RunEvent, WindowEvent,
};

const POLL_EVERY: Duration = Duration::from_secs(1);
/// Максимум данных, который можно потерять при внезапном отключении питания.
const FLUSH_EVERY: Duration = Duration::from_secs(30);

/// Порядок блокировок везде один: сначала `tracker`, затем `db` (исключает взаимоблокировки и
/// гарантирует, что отчёт не увидит одни и те же данные дважды или не увидит их совсем).
struct AppState {
    tracker: Mutex<Tracker>,
    db: Mutex<Db>,
    status: Mutex<Status>,
    settings: Mutex<Settings>,
    /// Однократное сообщение для пользователя (например, о восстановлении базы после повреждения).
    notice: Mutex<Option<Notice>>,
    /// Приложение работает только из трея: окно закрыто (память WebView освобождена), выход не нужен.
    tray_mode: AtomicBool,
}

/// Однократное сообщение для интерфейса; текст по коду подбирает веб-часть на выбранном языке.
#[derive(Serialize)]
struct Notice {
    code: &'static str,
    detail: String,
}

/// Пункты меню трея, чьи подписи меняются вместе с языком.
struct TrayItems {
    open: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn apply_tray_language(app: &AppHandle, lang: &str) {
    let t = tray_text(lang);
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.open.set_text(t.open);
        let _ = items.quit.set_text(t.quit);
    }
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(t.tooltip));
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Переносит накопленное в памяти время в БД. При ошибке данные возвращаются в накопитель.
fn flush(state: &AppState) {
    let mut tracker = lock(&state.tracker);
    let rows = tracker.take_pending();
    if rows.is_empty() {
        return;
    }
    if let Err(e) = lock(&state.db).flush(&rows) {
        eprintln!("NEXUS: не удалось сохранить статистику: {e}");
        tracker.restore(rows);
    }
}

/// Окончательное сохранение: засчитывает отложенное время и пишет всё в БД.
fn finalize_and_flush(state: &AppState) {
    lock(&state.tracker).finalize();
    flush(state);
}

#[tauri::command]
fn report(
    state: tauri::State<'_, Arc<AppState>>,
    period: String,
    anchor: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Report, String> {
    let today = Date::parse(&platform::local_day()).ok_or("не удалось определить текущую дату")?;
    if period == "range" {
        // Произвольный диапазон: обе даты разбираются строго, перепутанные меняются местами.
        let (from, to) = parse_range(from.as_deref().unwrap_or(""), to.as_deref().unwrap_or(""))
            .ok_or("некорректный диапазон дат")?;
        let tracker = lock(&state.tracker);
        let pending = tracker.pending_all();
        let db = lock(&state.db);
        return build_range_report(&db, &pending, from, to, today).map_err(|e| e.to_string());
    }
    let period = Period::parse(&period).ok_or("неизвестный период")?;
    let anchor = match anchor {
        Some(a) => Date::parse(&a).ok_or("некорректная дата")?,
        None => today,
    };
    let tracker = lock(&state.tracker);
    let pending = tracker.pending_all();
    let db = lock(&state.db);
    build_report(&db, &pending, period, anchor, today).map_err(|e| e.to_string())
}

#[tauri::command]
fn current_status(state: tauri::State<'_, Arc<AppState>>) -> Status {
    lock(&state.status).clone()
}

/// Полностью удаляет статистику (и сохранённую, и накопленную в памяти).
#[tauri::command]
fn clear_statistics(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut tracker = lock(&state.tracker);
    tracker.clear();
    lock(&state.db).clear_all().map_err(|e| e.to_string())
}

/// Чёрный список → трекеру. Вызывается под блокировкой трекера и базы (порядок: трекер, затем база).
fn sync_ignored(tracker: &mut Tracker, db: &Db) -> Result<Vec<AppRef>, String> {
    let list = db.ignored_apps().map_err(|e| e.to_string())?;
    tracker.set_ignored(list.iter().map(|a| a.exe.clone()).collect::<HashSet<_>>());
    Ok(list)
}

#[tauri::command]
fn get_ignored(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<AppRef>, String> {
    lock(&state.db).ignored_apps().map_err(|e| e.to_string())
}

/// Иконки приложений как `data:`-адреса PNG. Возвращает только найденные; при выключенной настройке — ничего.
#[tauri::command]
fn get_icons(state: tauri::State<'_, Arc<AppState>>, exes: Vec<String>) -> Result<HashMap<String, String>, String> {
    if !lock(&state.settings).show_icons {
        return Ok(HashMap::new());
    }
    // Данные от интерфейса не доверенные: ограничиваем число запросов и проверяем имена.
    let wanted: Vec<String> = exes.iter().take(300).filter_map(|e| normalize_exe(e)).collect();
    let found = lock(&state.db).icons_for(&wanted).map_err(|e| e.to_string())?;
    Ok(found
        .into_iter()
        .map(|(exe, png)| (exe, format!("data:image/png;base64,{}", base64_encode(&png))))
        .collect())
}

/// Приложения, которые можно исключить: известные по статистике плюс те, что ещё не сохранены в БД.
#[tauri::command]
fn list_known_apps(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<AppRef>, String> {
    let tracker = lock(&state.tracker);
    let db = lock(&state.db);
    let mut apps = db.known_apps().map_err(|e| e.to_string())?;
    for exe in tracker.pending_exes() {
        if !apps.iter().any(|a| a.exe == exe) {
            apps.push(AppRef { name: display_name(&exe), exe });
        }
    }
    apps.retain(|a| !tracker.is_ignored(&a.exe));
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.exe.cmp(&b.exe)));
    Ok(apps)
}

/// Добавляет приложение в чёрный список и безвозвратно удаляет всю его статистику.
/// Сначала меняется база: если запись не удалась, учёт остаётся прежним.
#[tauri::command]
fn ignore_app(
    state: tauri::State<'_, Arc<AppState>>,
    exe: String,
    name: Option<String>,
) -> Result<Vec<AppRef>, String> {
    let exe = normalize_exe(&exe).ok_or("некорректное имя приложения")?;
    let name: String = name.unwrap_or_default().chars().take(200).collect();
    let mut tracker = lock(&state.tracker);
    let mut db = lock(&state.db);
    db.add_ignored(&exe, &name).map_err(|e| e.to_string())?;
    sync_ignored(&mut tracker, &db)
}

/// Убирает приложение из чёрного списка (удалённая статистика не восстанавливается).
#[tauri::command]
fn unignore_app(state: tauri::State<'_, Arc<AppState>>, exe: String) -> Result<Vec<AppRef>, String> {
    let exe = normalize_exe(&exe).ok_or("некорректное имя приложения")?;
    let mut tracker = lock(&state.tracker);
    let mut db = lock(&state.db);
    db.remove_ignored(&exe).map_err(|e| e.to_string())?;
    sync_ignored(&mut tracker, &db)
}

/// Настройки для интерфейса: к сохранённым в БД добавлен автозапуск (его источник правды — реестр).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    idle_minutes: u32,
    close_to_tray: bool,
    count_idle: bool,
    quick_exclude: bool,
    show_icons: bool,
    animations: bool,
    language: String,
    theme: String,
    autostart: bool,
}

fn settings_dto(state: &AppState) -> SettingsDto {
    let s = lock(&state.settings).clone();
    SettingsDto {
        idle_minutes: s.idle_minutes,
        close_to_tray: s.close_to_tray,
        count_idle: s.count_idle,
        quick_exclude: s.quick_exclude,
        show_icons: s.show_icons,
        animations: s.animations,
        language: s.language,
        theme: s.theme,
        autostart: platform::autostart_enabled(),
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> SettingsDto {
    settings_dto(&state)
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    settings: SettingsDto,
) -> Result<SettingsDto, String> {
    if settings.autostart != platform::autostart_enabled() {
        platform::set_autostart(settings.autostart)?;
    }
    let new = Settings {
        idle_minutes: settings.idle_minutes,
        close_to_tray: settings.close_to_tray,
        count_idle: settings.count_idle,
        quick_exclude: settings.quick_exclude,
        show_icons: settings.show_icons,
        animations: settings.animations,
        language: settings.language,
        theme: settings.theme,
    }
    .sanitized();
    {
        let mut tracker = lock(&state.tracker);
        new.save(&mut lock(&state.db)).map_err(|e| e.to_string())?;
        tracker.set_idle_threshold_ms(new.idle_threshold_ms());
        tracker.set_count_idle(new.count_idle);
    }
    let (language_changed, theme_changed) = {
        let old = lock(&state.settings);
        (old.language != new.language, old.theme != new.theme)
    };
    let language = new.language.clone();
    let theme = new.theme.clone();
    *lock(&state.settings) = new;
    if language_changed {
        apply_tray_language(&app, &language);
    }
    if theme_changed {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_theme(window_theme(&theme));
            let _ = w.set_background_color(Some(window_background(&theme)));
        }
    }
    Ok(settings_dto(&state))
}

/// Сообщение, накопленное при запуске. Показывается один раз.
#[tauri::command]
fn take_notice(state: tauri::State<'_, Arc<AppState>>) -> Option<Notice> {
    lock(&state.notice).take()
}

#[cfg(windows)]
fn spawn_tracker(state: Arc<AppState>) {
    thread::Builder::new()
        .name("nexus-tracker".into())
        .spawn(move || {
            let mut sampler = platform::Sampler::new();
            let epoch = Instant::now();
            let mut last_flush = Instant::now();
            loop {
                thread::sleep(POLL_EVERY);
                // Непредвиденная ошибка в одном замере не должна навсегда остановить учёт.
                let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let sample = sampler.sample();
                    let now_ms = epoch.elapsed().as_millis() as u64;
                    // Статус строится из замера без исключённых приложений: «фокус на исключённом» = нет приложения.
                    let shown = {
                        let mut tracker = lock(&state.tracker);
                        tracker.tick(now_ms, &sample);
                        tracker.visible_sample(&sample)
                    };
                    let (threshold, count_idle, show_icons) = {
                        let s = lock(&state.settings);
                        (s.idle_threshold_ms(), s.count_idle, s.show_icons)
                    };
                    sampler.set_want_icons(show_icons);
                    *lock(&state.status) = Status::from_sample(&shown, threshold, count_idle);
                    for (exe, png) in sampler.take_new_icons() {
                        if lock(&state.tracker).is_ignored(&exe) {
                            continue;
                        }
                        if let Err(e) = lock(&state.db).set_icon(&exe, &png) {
                            eprintln!("NEXUS: не удалось сохранить иконку: {e}");
                        }
                    }
                    for (exe, name) in sampler.take_new_names() {
                        if lock(&state.tracker).is_ignored(&exe) {
                            continue; // об исключённых приложениях в базе не остаётся даже имени
                        }
                        if let Err(e) = lock(&state.db).set_app_name(&exe, &name) {
                            eprintln!("NEXUS: не удалось сохранить имя приложения: {e}");
                        }
                    }
                    if last_flush.elapsed() >= FLUSH_EVERY {
                        flush(&state);
                        last_flush = Instant::now();
                    }
                }));
                if step.is_err() {
                    eprintln!("NEXUS: сбой в потоке учёта, замер пропущен");
                    lock(&state.tracker).reset_clock();
                }
            }
        })
        .expect("не удалось запустить поток учёта");
}

/// Показывает главное окно. Если оно было закрыто ради экономии памяти — создаёт его заново.
/// Тема рамки окна: при «системной» окно следует Windows само.
fn window_theme(theme: &str) -> Option<tauri::Theme> {
    match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    }
}

/// Цвет фона окна до отрисовки страницы: совпадает с фоном выбранной темы, чтобы не было вспышки.
/// Значения — `--bg` из styles.css.
fn window_background(theme: &str) -> tauri::window::Color {
    let light = match theme {
        "light" => true,
        "dark" => false,
        _ => platform::system_prefers_light(),
    };
    if light {
        tauri::window::Color(244, 246, 251, 255)
    } else {
        tauri::window::Color(16, 19, 26, 255)
    }
}

fn show_main(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.tray_mode.store(false, Ordering::SeqCst);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app.config().app.windows.first().cloned() else { return };
    let theme = app.try_state::<Arc<AppState>>().map(|s| lock(&s.settings).theme.clone()).unwrap_or_default();
    let built = tauri::WebviewWindowBuilder::from_config(app, &cfg)
        .map(|b| b.theme(window_theme(&theme)).background_color(window_background(&theme)))
        .and_then(|b| b.build());
    match built {
        Ok(w) => {
            let _ = w.show();
            let _ = w.set_focus();
        }
        Err(e) => eprintln!("NEXUS: не удалось открыть окно: {e}"),
    }
}

fn setup_tray(app: &tauri::App, lang: &str) -> tauri::Result<()> {
    let text = tray_text(lang);
    let open = MenuItem::with_id(app, "open", text.open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", text.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    app.manage(TrayItems { open: open.clone(), quit: quit.clone() });
    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .tooltip(text.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            // exit() запускает RunEvent::Exit, где статистика сохраняется.
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    // Автозапуск стартует с ключом --minimized: окно не показывается, приложение сразу в трее.
    let start_minimized = std::env::args().any(|a| a == "--minimized");

    #[cfg(windows)]
    if !platform::acquire_single_instance() {
        if !start_minimized {
            platform::request_show_existing();
        }
        return;
    }

    let app = tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let Some(state) = window.try_state::<Arc<AppState>>() else { return };
            match event {
                // Сворачивание уводит приложение в трей. Окно закрывается целиком, чтобы освободить
                // память WebView2; учёт продолжается, при открытии окно создаётся заново.
                WindowEvent::Resized(_) => {
                    if window.is_minimized().unwrap_or(false) {
                        state.tray_mode.store(true, Ordering::SeqCst);
                        let _ = window.destroy();
                    }
                }
                // По настройке закрытие окна тоже уводит приложение в трей, а не завершает его.
                WindowEvent::CloseRequested { .. } => {
                    if lock(&state.settings).close_to_tray {
                        state.tray_mode.store(true, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        })
        .setup(move |app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let (mut db, recovered) = Db::open_or_recover(&dir.join("nexus.sqlite3"))?;
            let notice = recovered.map(|p| Notice {
                code: "db_recovered",
                detail: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            });
            let mut settings = Settings::load(&db).unwrap_or_default();
            if db.get_setting("language").ok().flatten().is_none() {
                // Первый запуск: язык, выбранный в установщике. У портативной версии его нет — английский.
                #[cfg(windows)]
                if let Some(code) = platform::installer_language().filter(|c| is_supported_language(c)) {
                    settings.language = code;
                }
                let _ = settings.save(&mut db);
            }
            let tracker_cfg = TrackerConfig {
                idle_threshold_ms: settings.idle_threshold_ms(),
                count_idle: settings.count_idle,
                ..TrackerConfig::default()
            };
            let mut tracker = Tracker::new(tracker_cfg);
            if let Ok(list) = db.ignored_apps() {
                tracker.set_ignored(list.into_iter().map(|a| a.exe).collect());
            }
            let state = Arc::new(AppState {
                tracker: Mutex::new(tracker),
                db: Mutex::new(db),
                status: Mutex::new(Status::starting()),
                settings: Mutex::new(settings),
                notice: Mutex::new(notice),
                tray_mode: AtomicBool::new(false),
            });
            #[cfg(windows)]
            {
                // Если exe перенесли в другую папку, запись автозапуска обновится при следующем старте.
                if platform::autostart_enabled() {
                    let _ = platform::set_autostart(true);
                }
                spawn_tracker(state.clone());
                let for_session = state.clone();
                let handle = app.handle().clone();
                platform::spawn_session_watcher(
                    move || finalize_and_flush(&for_session),
                    move || {
                        let h = handle.clone();
                        let _ = handle.run_on_main_thread(move || show_main(&h));
                    },
                );
            }
            app.manage(state.clone());
            let language = lock(&state.settings).language.clone();
            setup_tray(app, &language)?;
            if start_minimized {
                // Окно не создаётся вообще: до первого открытия приложение занимает минимум памяти.
                state.tray_mode.store(true, Ordering::SeqCst);
            } else {
                show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            report,
            current_status,
            clear_statistics,
            get_settings,
            save_settings,
            take_notice,
            get_ignored,
            list_known_apps,
            ignore_app,
            unignore_app,
            get_icons
        ])
        .build(tauri::generate_context!())
        .expect("не удалось создать приложение");

    app.run(|handle, event| match event {
        // Закрытие последнего окна не завершает приложение, пока оно работает из трея.
        // Явный выход (меню трея, exit(0)) приходит с кодом и не блокируется.
        RunEvent::ExitRequested { code, api, .. } => {
            if code.is_none() {
                if let Some(state) = handle.try_state::<Arc<AppState>>() {
                    if state.tray_mode.load(Ordering::SeqCst) {
                        api.prevent_exit();
                    }
                }
            }
        }
        RunEvent::Exit => {
            if let Some(state) = handle.try_state::<Arc<AppState>>() {
                finalize_and_flush(&state);
            }
        }
        _ => {}
    });
}
