use crate::model::{display_name, AppUsage, PendingRow};
use rusqlite::{params, Connection};
use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA_VERSION: i32 = 2;

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

    /// Удаляет всю статистику (настройки сохраняются).
    pub fn clear_all(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM usage", [])?;
        tx.execute("DELETE FROM apps", [])?;
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
}
