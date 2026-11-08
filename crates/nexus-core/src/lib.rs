//! Ядро NEXUS: учёт времени, периоды, отчёты и локальное хранилище. Не зависит ни от Tauri, ни
//! от Windows, поэтому полностью покрывается обычными unit-тестами.

mod dates;
mod db;
mod i18n;
mod icon;
mod model;
mod report;
mod settings;
mod tracker;

pub use dates::{days_in_month, Date, Period};
pub use db::{Db, Error};
pub use icon::{base64_encode, bgra_to_rgba, encode_png_rgba, looks_like_png, MAX_ICON_BYTES, MAX_ICON_SIDE};
pub use i18n::{tray_text, TrayText};
pub use model::{display_name, merge_pending, normalize_exe, AppRef, AppUsage, Delta, PendingRow, Sample, Status};
pub use report::{build_range_report, build_report, parse_range, Bucket, Report, DAILY_MAX_RANGE_DAYS, MAX_RANGE_DAYS};
pub use settings::{
    is_supported_language, is_supported_theme, Settings, DEFAULT_LANGUAGE, DEFAULT_THEME, MAX_IDLE_MINUTES,
    MIN_IDLE_MINUTES, SUPPORTED_LANGUAGES, SUPPORTED_THEMES,
};
pub use tracker::{Tracker, TrackerConfig, DEFAULT_IDLE_THRESHOLD_MS, DEFAULT_MAX_GAP_MS};
