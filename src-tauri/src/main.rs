#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod platform;

use nexus_core::{
    build_report, is_supported_language, tray_text, Date, Db, Period, Report, Settings, Status, Tracker, TrackerConfig,
};
use serde::{Deserialize, Serialize};
use std::{
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
fn report(state: tauri::State<'_, Arc<AppState>>, period: String, anchor: Option<String>) -> Result<Report, String> {
    let period = Period::parse(&period).ok_or("неизвестный период")?;
    let today = Date::parse(&platform::local_day()).ok_or("не удалось определить текущую дату")?;
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

/// Настройки для интерфейса: к сохранённым в БД добавлен автозапуск (его источник правды — реестр).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    idle_minutes: u32,
    close_to_tray: bool,
    count_idle: bool,
    language: String,
    autostart: bool,
}

fn settings_dto(state: &AppState) -> SettingsDto {
    let s = lock(&state.settings).clone();
    SettingsDto {
        idle_minutes: s.idle_minutes,
        close_to_tray: s.close_to_tray,
        count_idle: s.count_idle,
        language: s.language,
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
        language: settings.language,
    }
    .sanitized();
    {
        let mut tracker = lock(&state.tracker);
        new.save(&mut lock(&state.db)).map_err(|e| e.to_string())?;
        tracker.set_idle_threshold_ms(new.idle_threshold_ms());
        tracker.set_count_idle(new.count_idle);
    }
    let language_changed = lock(&state.settings).language != new.language;
    let language = new.language.clone();
    *lock(&state.settings) = new;
    if language_changed {
        apply_tray_language(&app, &language);
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
                    lock(&state.tracker).tick(now_ms, &sample);
                    let (threshold, count_idle) = {
                        let s = lock(&state.settings);
                        (s.idle_threshold_ms(), s.count_idle)
                    };
                    *lock(&state.status) = Status::from_sample(&sample, threshold, count_idle);
                    for (exe, name) in sampler.take_new_names() {
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
    match tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
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
            let state = Arc::new(AppState {
                tracker: Mutex::new(Tracker::new(tracker_cfg)),
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
        .invoke_handler(tauri::generate_handler![report, current_status, clear_statistics, get_settings, save_settings, take_notice])
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
