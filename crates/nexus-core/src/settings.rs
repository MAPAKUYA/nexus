use crate::db::{Db, Result};
use crate::tracker::DEFAULT_IDLE_THRESHOLD_MS;
use serde::{Deserialize, Serialize};

pub const MIN_IDLE_MINUTES: u32 = 1;
pub const MAX_IDLE_MINUTES: u32 = 120;

/// Языки интерфейса. Коды должны совпадать со списком в `src/i18n/index.ts` (это проверяется тестом).
pub const SUPPORTED_LANGUAGES: [&str; 8] = ["en", "ru", "es", "de", "fr", "pt", "zh", "ja"];
pub const DEFAULT_LANGUAGE: &str = "en";

pub fn is_supported_language(code: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&code)
}

/// Настройки, хранящиеся в локальной БД. Автозапуск сюда не входит: его источник правды — реестр.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub idle_minutes: u32,
    /// Кнопка закрытия окна прячет приложение в трей вместо выхода.
    pub close_to_tray: bool,
    /// Засчитывать время без ввода как активное (определение простоя выключено).
    /// Заблокированный экран при этом всё равно не засчитывается.
    pub count_idle: bool,
    /// Язык интерфейса (код из `SUPPORTED_LANGUAGES`).
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { idle_minutes: (DEFAULT_IDLE_THRESHOLD_MS / 60_000) as u32, close_to_tray: false, count_idle: false, language: DEFAULT_LANGUAGE.to_string() }
    }
}

impl Settings {
    /// Приводит значения к допустимым (данные из интерфейса и БД не считаются доверенными).
    pub fn sanitized(mut self) -> Self {
        self.idle_minutes = self.idle_minutes.clamp(MIN_IDLE_MINUTES, MAX_IDLE_MINUTES);
        if !is_supported_language(&self.language) {
            self.language = DEFAULT_LANGUAGE.to_string();
        }
        self
    }

    pub fn idle_threshold_ms(&self) -> u64 {
        self.idle_minutes as u64 * 60_000
    }

    pub fn load(db: &Db) -> Result<Self> {
        let d = Settings::default();
        let flag = |key: &str, default: bool| -> Result<bool> { Ok(db.get_setting(key)?.map(|v| v == "1").unwrap_or(default)) };
        let idle = db.get_setting("idle_minutes")?.and_then(|v| v.parse().ok()).unwrap_or(d.idle_minutes);
        let language = db.get_setting("language")?.unwrap_or(d.language);
        Ok(Settings {
            idle_minutes: idle,
            close_to_tray: flag("close_to_tray", d.close_to_tray)?,
            count_idle: flag("count_idle", d.count_idle)?,
            language,
        }
        .sanitized())
    }

    pub fn save(&self, db: &mut Db) -> Result<()> {
        let s = self.clone().sanitized();
        let flag = |b: bool| if b { "1" } else { "0" };
        db.set_setting("idle_minutes", &s.idle_minutes.to_string())?;
        db.set_setting("close_to_tray", flag(s.close_to_tray))?;
        db.set_setting("count_idle", flag(s.count_idle))?;
        db.set_setting("language", &s.language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(idle_minutes: u32, close_to_tray: bool, count_idle: bool) -> Settings {
        Settings { idle_minutes, close_to_tray, count_idle, language: "en".into() }
    }

    #[test]
    fn defaults_are_five_minutes_exit_on_close_and_idle_not_counted() {
        let db = Db::open_in_memory().unwrap();
        let s = Settings::load(&db).unwrap();
        assert_eq!(s, with(5, false, false));
        assert_eq!(s.idle_threshold_ms(), 300_000);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let mut db = Db::open_in_memory().unwrap();
        with(15, true, true).save(&mut db).unwrap();
        assert_eq!(Settings::load(&db).unwrap(), with(15, true, true));
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let mut db = Db::open_in_memory().unwrap();
        with(0, false, false).save(&mut db).unwrap();
        assert_eq!(Settings::load(&db).unwrap().idle_minutes, 1);
        with(100_000, false, false).save(&mut db).unwrap();
        assert_eq!(Settings::load(&db).unwrap().idle_minutes, 120);
    }

    #[test]
    fn garbage_in_db_falls_back_to_defaults() {
        let mut db = Db::open_in_memory().unwrap();
        db.set_setting("idle_minutes", "abc").unwrap();
        db.set_setting("close_to_tray", "maybe").unwrap();
        db.set_setting("count_idle", "yes").unwrap();
        // нераспознанное значение флага читается как «выключено», а не как ошибка
        assert_eq!(Settings::load(&db).unwrap(), with(5, false, false));
    }

    #[test]
    fn language_roundtrip_and_validation() {
        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(Settings::load(&db).unwrap().language, "en", "по умолчанию английский");
        Settings { language: "ru".into(), ..with(5, false, false) }.save(&mut db).unwrap();
        assert_eq!(Settings::load(&db).unwrap().language, "ru");
        // неизвестный язык (из БД или интерфейса) заменяется английским, а не ломает приложение
        db.set_setting("language", "xx").unwrap();
        assert_eq!(Settings::load(&db).unwrap().language, "en");
        Settings { language: "../../etc".into(), ..with(5, false, false) }.save(&mut db).unwrap();
        assert_eq!(Settings::load(&db).unwrap().language, "en");
    }

    #[test]
    fn supported_languages_list() {
        assert!(SUPPORTED_LANGUAGES.iter().all(|c| is_supported_language(c)));
        assert!(is_supported_language("en") && is_supported_language("ja"));
        assert!(!is_supported_language("EN") && !is_supported_language("") && !is_supported_language("en-US"));
        assert_eq!(SUPPORTED_LANGUAGES[0], DEFAULT_LANGUAGE);
    }
}
