//! Ядро NEXUS: учёт времени, периоды, отчёты и локальное хранилище. Не зависит ни от Tauri, ни
//! от Windows, поэтому полностью покрывается обычными unit-тестами.

mod dates;
mod db;
mod i18n;
mod model;
mod report;
mod settings;
mod tracker;

pub use dates::{days_in_month, Date, Period};
pub use db::{Db, Error};
pub use i18n::{tray_text, TrayText};
pub use model::{display_name, merge_pending, AppUsage, Delta, PendingRow, Sample, Status};
pub use report::{build_report, Bucket, Report};
pub use settings::{
    is_supported_language, Settings, DEFAULT_LANGUAGE, MAX_IDLE_MINUTES, MIN_IDLE_MINUTES, SUPPORTED_LANGUAGES,
};
pub use tracker::{Tracker, TrackerConfig, DEFAULT_IDLE_THRESHOLD_MS, DEFAULT_MAX_GAP_MS};
