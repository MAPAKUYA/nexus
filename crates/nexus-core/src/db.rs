use crate::icon::looks_like_png;
use crate::model::{display_name, AppRef, AppUsage, PendingRow};
use rusqlite::{params, Connection};
use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: i32 = 3;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    /// База создана более новой версией NEXUS.
    UnsupportedSchema(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "ошибка базы данных: {e}"),
            Error::UnsupportedSchema(v) => {
                write!(f, "база данных создана более новой версией NEXUS (схема {v})")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Локальное хранилище статистики. Единственный файл SQLite, никаких сетевых функций.
pub struct Db {
    conn: Connection,
    app_ids: HashMap<String, i64>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    /// Открывает базу; если файл повреждён (не является базой SQLite), переносит его в резервную копию
    /// рядом и создаёт новую. Возвращает путь к резервной копии, если восстановление потребовалось.
    /// Базы более новой версии и любые другие ошибки (права, диск) не трогаются.
    pub fn open_or_recover(path: &Path) -> Result<(Self, Option<PathBuf>)> {
        use rusqlite::ErrorCode;
        match Self::open(path) {
            Ok(db) => Ok((db, None)),
            Err(Error::Sqlite(rusqlite::Error::SqliteFailure(e, _)))
                if matches!(e.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) =>
            {
                let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let suffixed = |suffix: &str| -> PathBuf {
                    let mut name = path.file_name().unwrap_or_default().to_os_string();
                    name.push(format!("{suffix}.broken-{secs}"));
                    path.with_file_name(name)
                };
                let backup = suffixed("");
                std::fs::rename(path, &backup).map_err(|_| Error::Sqlite(rusqlite::Error::SqliteFailure(e, None)))?;
                for ext in ["-wal", "-shm"] {
                    let mut side = path.as_os_str().to_os_string();
                    side.push(ext);
                    let side = PathBuf::from(side);
                    if side.exists() {
                        let _ = std::fs::rename(&side, suffixed(ext));
                    }
                }
                Self::open(path).map(|db| (db, Some(backup)))
            }
            Err(other) => Err(other),
        }
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 3000;",
        )?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema(version));
        }
        if version < 1 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE apps (
                     id   INTEGER PRIMARY KEY,
                     exe  TEXT NOT NULL UNIQUE,
                     name TEXT NOT NULL
                 );
                 CREATE TABLE usage (
                     day        TEXT    NOT NULL,
                     app_id     INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
                     active_ms  INTEGER NOT NULL DEFAULT 0,
                     running_ms INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY (day, app_id)
                 ) WITHOUT ROWID;
                 CREATE INDEX usage_app ON usage(app_id);
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE settings (
                     key   TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 ) WITHOUT ROWID;
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        }
        if version < 3 {
            // Чёрный список приложений и (на будущее) их иконки. Обе таблицы создаются одной миграцией.
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE ignored_apps (
                     exe  TEXT PRIMARY KEY,
                     name TEXT NOT NULL
                 ) WITHOUT ROWID;
                 CREATE TABLE app_icons (
                     exe TEXT PRIMARY KEY,
                     png BLOB NOT NULL
                 ) WITHOUT ROWID;
                 PRAGMA user_version = 3;
                 COMMIT;",
            )?;
        }
        Ok(Self { conn, app_ids: HashMap::new() })
    }

    /// Атомарно прибавляет накопленное время. Запись (день, приложение) всегда одна: дубликаты
    /// невозможны благодаря первичному ключу и UPSERT.
    pub fn flush(&mut self, rows: &[PendingRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        let mut new_ids: HashMap<String, i64> = HashMap::new();
        {
            let mut insert_app =
                tx.prepare_cached("INSERT INTO apps (exe, name) VALUES (?1, ?2) ON CONFLICT(exe) DO NOTHING")?;
            let mut select_app = tx.prepare_cached("SELECT id FROM apps WHERE exe = ?1")?;
            let mut upsert = tx.prepare_cached(
                "INSERT INTO usage (day, app_id, active_ms, running_ms) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(day, app_id) DO UPDATE SET
                     active_ms  = active_ms  + excluded.active_ms,
                     running_ms = running_ms + excluded.running_ms",
            )?;
            for r in rows {
                let id = match self.app_ids.get(&r.exe).or_else(|| new_ids.get(&r.exe)) {
                    Some(id) => *id,
                    None => {
                        insert_app.execute(params![r.exe, display_name(&r.exe)])?;
                        let id: i64 = select_app.query_row([&r.exe], |x| x.get(0))?;
                        new_ids.insert(r.exe.clone(), id);
                        id
                    }
                };
                upsert.execute(params![
                    r.day,
                    id,
                    r.delta.active_ms.min(i64::MAX as u64) as i64,
                    r.delta.running_ms.min(i64::MAX as u64) as i64
                ])?;
            }
        }
        tx.commit()?;
        self.app_ids.extend(new_ids); // кэш обновляем только после успешного коммита
        Ok(())
    }

    pub fn usage_for_day(&self, day: &str) -> Result<Vec<AppUsage>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT a.exe, a.name, u.active_ms, u.running_ms
             FROM usage u JOIN apps a ON a.id = u.app_id
             WHERE u.day = ?1 AND (u.active_ms > 0 OR u.running_ms > 0)
             ORDER BY u.active_ms DESC, u.running_ms DESC, a.exe",
        )?;
        let rows = stmt
            .query_map([day], |r| {
                Ok(AppUsage {
                    exe: r.get(0)?,
                    name: r.get(1)?,
                    active_ms: r.get::<_, i64>(2)?.max(0) as u64,
                    running_ms: r.get::<_, i64>(3)?.max(0) as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Использование приложений за период [from, to], суммарно по дням.
    pub fn apps_in_range(&self, from: &str, to: &str) -> Result<Vec<AppUsage>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT a.exe, a.name, SUM(u.active_ms), SUM(u.running_ms)
             FROM usage u JOIN apps a ON a.id = u.app_id
             WHERE u.day BETWEEN ?1 AND ?2
             GROUP BY a.id
             HAVING SUM(u.active_ms) > 0 OR SUM(u.running_ms) > 0
             ORDER BY SUM(u.active_ms) DESC, SUM(u.running_ms) DESC, a.exe",
        )?;
        let rows = stmt
            .query_map([from, to], |r| {
                Ok(AppUsage {
                    exe: r.get(0)?,
                    name: r.get(1)?,
                    active_ms: r.get::<_, i64>(2)?.max(0) as u64,
                    running_ms: r.get::<_, i64>(3)?.max(0) as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Активное время по дням (`YYYY-MM-DD`) или по месяцам (`YYYY-MM`) за период.
    pub fn buckets_in_range(&self, from: &str, to: &str, monthly: bool) -> Result<Vec<(String, u64)>> {
        let sql = if monthly {
            "SELECT substr(day, 1, 7), SUM(active_ms) FROM usage WHERE day BETWEEN ?1 AND ?2 GROUP BY 1"
        } else {
            "SELECT day, SUM(active_ms) FROM usage WHERE day BETWEEN ?1 AND ?2 GROUP BY 1"
        };
        let mut stmt = self.conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map([from, to], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?.max(0) as u64)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Запоминает читаемое имя приложения (из свойств файла). Остальные данные не затрагиваются.
    pub fn set_app_name(&mut self, exe: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO apps (exe, name) VALUES (?1, ?2) ON CONFLICT(exe) DO UPDATE SET name = excluded.name",
            params![exe, name],
        )?;
        Ok(())
    }

    pub fn app_names(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare_cached("SELECT exe, name FROM apps")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<HashMap<_, _>, _>>()?;
        Ok(rows)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        Ok(match rows.next()? {
            Some(r) => Some(r.get(0)?),
            None => None,
        })
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Сохраняет иконку приложения (PNG), заменяя прежнюю. Данные, не похожие на PNG, отвергаются.
    /// Иконки исключённых приложений не сохраняются.
    pub fn set_icon(&mut self, exe: &str, png: &[u8]) -> Result<bool> {
        if !looks_like_png(png) {
            return Ok(false);
        }
        let ignored: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM ignored_apps WHERE exe = ?1)", [exe], |r| r.get(0))?;
        if ignored {
            return Ok(false);
        }
        self.conn.execute("INSERT OR REPLACE INTO app_icons (exe, png) VALUES (?1, ?2)", params![exe, png])?;
        Ok(true)
    }

    /// Иконки для перечисленных приложений: `(exe, PNG)`. Приложения без иконки в ответ не попадают.
    pub fn icons_for(&self, exes: &[String]) -> Result<Vec<(String, Vec<u8>)>> {
        let mut stmt = self.conn.prepare_cached("SELECT png FROM app_icons WHERE exe = ?1")?;
        let mut out = Vec::new();
        for exe in exes {
            if let Ok(png) = stmt.query_row([exe], |r| r.get::<_, Vec<u8>>(0)) {
                out.push((exe.clone(), png));
            }
        }
        Ok(out)
    }

    /// Приложения, известные по сохранённой статистике: (exe, читаемое имя).
    pub fn known_apps(&self) -> Result<Vec<AppRef>> {
        let mut stmt = self.conn.prepare_cached("SELECT exe, name FROM apps ORDER BY name COLLATE NOCASE, exe")?;
        let rows = stmt
            .query_map([], |r| Ok(AppRef { exe: r.get(0)?, name: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Чёрный список приложений.
    pub fn ignored_apps(&self) -> Result<Vec<AppRef>> {
        let mut stmt = self.conn.prepare_cached("SELECT exe, name FROM ignored_apps ORDER BY name COLLATE NOCASE, exe")?;
        let rows = stmt
            .query_map([], |r| Ok(AppRef { exe: r.get(0)?, name: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Добавляет приложение в чёрный список и **безвозвратно удаляет** всю его сохранённую
    /// статистику (все дни) и иконку — одной транзакцией. `name` — читаемое имя для списка;
    /// если приложение уже есть в статистике, берётся сохранённое имя.
    pub fn add_ignored(&mut self, exe: &str, name: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        let stored: Option<String> =
            tx.query_row("SELECT name FROM apps WHERE exe = ?1", [exe], |r| r.get(0)).ok();
        let shown = stored.unwrap_or_else(|| if name.trim().is_empty() { display_name(exe) } else { name.to_string() });
        tx.execute(
            "INSERT INTO ignored_apps (exe, name) VALUES (?1, ?2) ON CONFLICT(exe) DO UPDATE SET name = excluded.name",
            params![exe, shown],
        )?;
        tx.execute("DELETE FROM usage WHERE app_id IN (SELECT id FROM apps WHERE exe = ?1)", [exe])?;
        tx.execute("DELETE FROM apps WHERE exe = ?1", [exe])?;
        tx.execute("DELETE FROM app_icons WHERE exe = ?1", [exe])?;
        tx.commit()?;
        self.app_ids.remove(exe); // кэш id не должен указывать на удалённую запись
        Ok(())
    }

    /// Убирает приложение из чёрного списка. Удалённая ранее статистика не восстанавливается.
    pub fn remove_ignored(&mut self, exe: &str) -> Result<()> {
        self.conn.execute("DELETE FROM ignored_apps WHERE exe = ?1", [exe])?;
        Ok(())
    }

    /// Удаляет всю статистику (настройки сохраняются).
    pub fn clear_all(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM usage", [])?;
        tx.execute("DELETE FROM apps", [])?;
        tx.execute("DELETE FROM app_icons", [])?;
        tx.commit()?;
        self.app_ids.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{merge_pending, Delta};

    fn row(day: &str, exe: &str, a: u64, r: u64) -> PendingRow {
        PendingRow { day: day.into(), exe: exe.into(), delta: Delta { active_ms: a, running_ms: r } }
    }

    #[test]
    fn flush_twice_accumulates_into_single_row() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("2026-01-01", "a.exe", 1_000, 2_000)]).unwrap();
        db.flush(&[row("2026-01-01", "a.exe", 500, 500)]).unwrap();
        let got = db.usage_for_day("2026-01-01").unwrap();
        assert_eq!(got.len(), 1, "дубликатов быть не должно");
        assert_eq!((got[0].active_ms, got[0].running_ms), (1_500, 2_500));
        assert_eq!(got[0].name, "A");
    }

    #[test]
    fn days_and_apps_are_independent_and_sorted_by_active() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[
            row("2026-01-01", "a.exe", 1_000, 9_000),
            row("2026-01-01", "b.exe", 5_000, 5_000),
            row("2026-01-02", "a.exe", 7_000, 7_000),
        ])
        .unwrap();
        let d1 = db.usage_for_day("2026-01-01").unwrap();
        assert_eq!(d1.iter().map(|x| x.exe.as_str()).collect::<Vec<_>>(), ["b.exe", "a.exe"]);
        assert_eq!(db.usage_for_day("2026-01-02").unwrap().len(), 1);
        assert!(db.usage_for_day("2026-01-03").unwrap().is_empty());
    }

    #[test]
    fn clear_all_removes_everything_and_db_is_reusable() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap();
        db.clear_all().unwrap();
        assert!(db.usage_for_day("d").unwrap().is_empty());
        db.flush(&[row("d", "a.exe", 2, 3)]).unwrap(); // кэш id не должен протухнуть
        assert_eq!(db.usage_for_day("d").unwrap()[0].active_ms, 2);
    }

    #[test]
    fn data_survives_reopen() {
        let path = std::env::temp_dir().join(format!("nexus-test-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut db = Db::open(&path).unwrap();
            db.flush(&[row("d", "a.exe", 4_000, 6_000)]).unwrap();
        }
        let db = Db::open(&path).unwrap();
        let got = db.usage_for_day("d").unwrap();
        assert_eq!((got[0].active_ms, got[0].running_ms), (4_000, 6_000));
        drop(db);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
        }
    }

    #[test]
    fn newer_schema_is_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(Db::init(conn), Err(Error::UnsupportedSchema(99))));
    }

    #[test]
    fn sql_injection_in_day_is_just_a_value() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.usage_for_day("x'; DROP TABLE usage;--").unwrap().is_empty());
        assert!(db.usage_for_day("2026-01-01").unwrap().is_empty()); // таблица жива
    }

    #[test]
    fn merge_pending_adds_unsaved_time_and_sorts() {
        let base = vec![AppUsage { exe: "a.exe".into(), name: "A".into(), active_ms: 1_000, running_ms: 1_000 }];
        let pending = vec![
            ("a.exe".to_string(), Delta { active_ms: 500, running_ms: 500 }),
            ("b.exe".to_string(), Delta { active_ms: 9_000, running_ms: 9_000 }),
        ];
        let m = merge_pending(base, &pending, &HashMap::new());
        assert_eq!(m[0].exe, "b.exe");
        assert_eq!(m[1].active_ms, 1_500);
    }

    #[test]
    fn display_name_is_readable() {
        assert_eq!(display_name("chrome.exe"), "Chrome");
        assert_eq!(display_name("Code.EXE"), "Code");
        assert_eq!(display_name("tool"), "Tool");
    }

    #[test]
    fn app_name_can_be_set_before_and_after_usage() {
        let mut db = Db::open_in_memory().unwrap();
        db.set_app_name("beamng.drive.x64.exe", "BeamNG.drive").unwrap(); // имя известно раньше данных
        db.flush(&[row("d", "beamng.drive.x64.exe", 1_000, 1_000)]).unwrap();
        assert_eq!(db.usage_for_day("d").unwrap()[0].name, "BeamNG.drive");
        db.set_app_name("beamng.drive.x64.exe", "BeamNG").unwrap(); // обновление имени
        assert_eq!(db.usage_for_day("d").unwrap()[0].name, "BeamNG");
        assert_eq!(db.app_names().unwrap()["beamng.drive.x64.exe"], "BeamNG");
        assert_eq!(db.usage_for_day("d").unwrap().len(), 1);
    }

    #[test]
    fn clear_all_keeps_settings() {
        let mut db = Db::open_in_memory().unwrap();
        db.set_setting("k", "v").unwrap();
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap();
        db.clear_all().unwrap();
        assert_eq!(db.get_setting("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn v1_database_is_migrated_without_losing_data() {
        let path = std::env::temp_dir().join(format!("nexus-mig-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            // база в точности как у версий 0.1.x–0.2.x
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE apps (id INTEGER PRIMARY KEY, exe TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
                 CREATE TABLE usage (day TEXT NOT NULL, app_id INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
                     active_ms INTEGER NOT NULL DEFAULT 0, running_ms INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (day, app_id)) WITHOUT ROWID;
                 INSERT INTO apps VALUES (1, 'old.exe', 'Old');
                 INSERT INTO usage VALUES ('2026-01-01', 1, 7000, 9000);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let mut db = Db::open(&path).unwrap();
        let got = db.usage_for_day("2026-01-01").unwrap();
        assert_eq!((got[0].active_ms, got[0].running_ms), (7_000, 9_000));
        db.set_setting("a", "b").unwrap(); // таблица settings появилась
        drop(db);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
        }
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nexus-{tag}-{}-{:?}.sqlite3", std::process::id(), std::thread::current().id()))
    }

    fn cleanup(path: &Path) {
        let dir = path.parent().unwrap();
        let stem = path.file_name().unwrap().to_string_lossy().to_string();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with(&stem) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_fresh_db_created() {
        let path = temp_path("corrupt");
        cleanup(&path);
        std::fs::write(&path, "это совсем не база данных SQLite ".repeat(200)).unwrap();
        let (mut db, backup) = Db::open_or_recover(&path).unwrap();
        let backup = backup.expect("должна быть резервная копия");
        assert!(backup.exists(), "повреждённый файл сохранён, а не удалён");
        assert!(std::fs::read(&backup).unwrap().starts_with("это совсем".as_bytes()));
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap(); // новая база рабочая
        assert_eq!(db.usage_for_day("d").unwrap().len(), 1);
        drop(db);
        cleanup(&path);
    }

    #[test]
    fn healthy_file_is_not_touched() {
        let path = temp_path("healthy");
        cleanup(&path);
        {
            let mut db = Db::open(&path).unwrap();
            db.flush(&[row("d", "a.exe", 5, 6)]).unwrap();
        }
        let (db, backup) = Db::open_or_recover(&path).unwrap();
        assert!(backup.is_none());
        assert_eq!(db.usage_for_day("d").unwrap()[0].active_ms, 5);
        drop(db);
        cleanup(&path);
    }

    #[test]
    fn newer_database_is_never_moved_or_replaced() {
        let path = temp_path("newer");
        cleanup(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(Db::open_or_recover(&path), Err(Error::UnsupportedSchema(99))));
        assert!(path.exists(), "файл остался на месте");
        cleanup(&path);
    }

    #[test]
    fn add_ignored_deletes_all_stored_history_of_that_app_only() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[
            row("2026-01-01", "a.exe", 1_000, 2_000),
            row("2026-01-02", "a.exe", 3_000, 4_000),
            row("2026-01-01", "b.exe", 5_000, 6_000),
        ])
        .unwrap();
        db.add_ignored("a.exe", "A").unwrap();
        assert!(db.usage_for_day("2026-01-02").unwrap().is_empty());
        let d1 = db.usage_for_day("2026-01-01").unwrap();
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].exe, "b.exe");
        assert_eq!(db.apps_in_range("2000-01-01", "2100-01-01").unwrap().len(), 1);
        assert_eq!(db.buckets_in_range("2000-01-01", "2100-01-01", false).unwrap().iter().map(|b| b.1).sum::<u64>(), 5_000);
        assert_eq!(db.ignored_apps().unwrap(), vec![AppRef { exe: "a.exe".into(), name: "A".into() }]);
        assert!(db.known_apps().unwrap().iter().all(|a| a.exe != "a.exe"));
    }

    #[test]
    fn add_ignored_keeps_stored_name_and_is_idempotent() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("d", "code.exe", 1, 1)]).unwrap();
        db.set_app_name("code.exe", "Visual Studio Code").unwrap();
        db.add_ignored("code.exe", "ignored-hint").unwrap();
        db.add_ignored("code.exe", "other").unwrap();
        let list = db.ignored_apps().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "other", "повторное добавление не создаёт дубликат");
        let mut db2 = Db::open_in_memory().unwrap();
        db2.add_ignored("new.exe", "").unwrap();
        assert_eq!(db2.ignored_apps().unwrap()[0].name, "New", "без имени берётся имя из exe");
    }

    #[test]
    fn writing_after_ignore_uses_fresh_app_id() {
        // Кэш id приложений не должен ссылаться на удалённую запись (иначе внешний ключ упадёт).
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap();
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap(); // кэш прогрет
        db.add_ignored("a.exe", "A").unwrap();
        db.remove_ignored("a.exe").unwrap();
        db.flush(&[row("d", "a.exe", 7, 8)]).unwrap();
        let got = db.usage_for_day("d").unwrap();
        assert_eq!((got[0].active_ms, got[0].running_ms), (7, 8), "старая статистика не вернулась");
    }

    #[test]
    fn remove_ignored_does_not_restore_data() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("d", "a.exe", 1, 1)]).unwrap();
        db.add_ignored("a.exe", "A").unwrap();
        db.remove_ignored("a.exe").unwrap();
        assert!(db.ignored_apps().unwrap().is_empty());
        assert!(db.usage_for_day("d").unwrap().is_empty());
    }

    #[test]
    fn clear_all_keeps_blacklist() {
        let mut db = Db::open_in_memory().unwrap();
        db.add_ignored("a.exe", "A").unwrap();
        db.clear_all().unwrap();
        assert_eq!(db.ignored_apps().unwrap().len(), 1, "чёрный список — настройка, а не статистика");
    }

    #[test]
    fn v2_database_is_migrated_to_v3_without_losing_data() {
        let path = std::env::temp_dir().join(format!("nexus-v2-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE apps (id INTEGER PRIMARY KEY, exe TEXT NOT NULL UNIQUE, name TEXT NOT NULL);
                 CREATE TABLE usage (day TEXT NOT NULL, app_id INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
                     active_ms INTEGER NOT NULL DEFAULT 0, running_ms INTEGER NOT NULL DEFAULT 0,
                     PRIMARY KEY (day, app_id)) WITHOUT ROWID;
                 CREATE INDEX usage_app ON usage(app_id);
                 CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
                 INSERT INTO apps (exe, name) VALUES ('a.exe', 'Alpha');
                 INSERT INTO usage VALUES ('2026-05-05', 1, 4000, 6000);
                 INSERT INTO settings VALUES ('language', 'ru');
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        }
        let mut db = Db::open(&path).unwrap();
        let got = db.usage_for_day("2026-05-05").unwrap();
        assert_eq!((got[0].name.as_str(), got[0].active_ms, got[0].running_ms), ("Alpha", 4_000, 6_000));
        assert_eq!(db.get_setting("language").unwrap().as_deref(), Some("ru"));
        assert!(db.ignored_apps().unwrap().is_empty());
        db.add_ignored("a.exe", "Alpha").unwrap();
        assert!(db.usage_for_day("2026-05-05").unwrap().is_empty());
        drop(db);
        let version: i32 = Connection::open(&path).unwrap().pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, 3);
        for ext in ["", "-wal", "-shm"] {
            let mut f = path.as_os_str().to_os_string();
            f.push(ext);
            let _ = std::fs::remove_file(f);
        }
    }

    fn fake_png() -> Vec<u8> {
        crate::icon::encode_png_rgba(2, 2, &[7; 16]).unwrap()
    }

    #[test]
    fn icons_are_stored_replaced_and_listed() {
        let mut db = Db::open_in_memory().unwrap();
        let png = fake_png();
        assert!(db.set_icon("a.exe", &png).unwrap());
        let other = crate::icon::encode_png_rgba(2, 2, &[9; 16]).unwrap();
        assert!(db.set_icon("a.exe", &other).unwrap(), "новая иконка заменяет прежнюю");
        assert!(db.set_icon("b.exe", &png).unwrap());
        let got = db.icons_for(&["a.exe".into(), "b.exe".into(), "none.exe".into()]).unwrap();
        assert_eq!(got.len(), 2, "для приложения без иконки записи нет");
        assert_eq!(got.iter().find(|(e, _)| e == "a.exe").unwrap().1, other);
    }

    #[test]
    fn garbage_and_ignored_apps_get_no_icon() {
        let mut db = Db::open_in_memory().unwrap();
        assert!(!db.set_icon("a.exe", b"definitely not a png, just text bytes here....").unwrap());
        assert!(!db.set_icon("a.exe", &[]).unwrap());
        db.add_ignored("secret.exe", "S").unwrap();
        assert!(!db.set_icon("secret.exe", &fake_png()).unwrap(), "исключённым иконка не нужна");
        assert!(db.icons_for(&["a.exe".into(), "secret.exe".into()]).unwrap().is_empty());
    }

    #[test]
    fn excluding_or_clearing_removes_icons() {
        let mut db = Db::open_in_memory().unwrap();
        db.set_icon("a.exe", &fake_png()).unwrap();
        db.set_icon("b.exe", &fake_png()).unwrap();
        db.add_ignored("a.exe", "A").unwrap();
        assert!(db.icons_for(&["a.exe".into()]).unwrap().is_empty(), "при исключении иконка удаляется");
        assert_eq!(db.icons_for(&["b.exe".into()]).unwrap().len(), 1);
        db.clear_all().unwrap();
        assert!(db.icons_for(&["b.exe".into()]).unwrap().is_empty(), "удаление статистики очищает иконки");
    }
}
