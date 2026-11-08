use serde::Serialize;
use std::collections::HashMap;

/// Накопленное время в миллисекундах.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Delta {
    /// Приложение было в фокусе, пользователь присутствовал.
    pub active_ms: u64,
    /// Процесс приложения был запущен (имел видимое окно).
    pub running_ms: u64,
}

/// Один замер состояния системы.
#[derive(Debug, Clone)]
pub struct Sample {
    /// Локальная дата замера, `YYYY-MM-DD`.
    pub day: String,
    /// Имя exe окна в фокусе (нижний регистр), если есть.
    pub foreground: Option<String>,
    /// Имена exe запущенных приложений с видимыми окнами.
    pub running: Vec<String>,
    /// Сколько мс прошло с последнего ввода (мыши/клавиатуры). Сам ввод не читается.
    pub idle_ms: u64,
    /// Экран заблокирован (или показан защищённый рабочий стол).
    pub locked: bool,
}

impl Sample {
    /// Замер без исключённых приложений: их нет ни среди запущенных, ни в фокусе
    /// (фокус на исключённом приложении означает «нет активного приложения»).
    pub fn without(&self, ignored: &std::collections::HashSet<String>) -> Sample {
        let mut s = self.clone();
        s.running.retain(|e| !ignored.contains(e));
        if s.foreground.as_deref().is_some_and(|f| ignored.contains(f)) {
            s.foreground = None;
        }
        s
    }
}

/// Текущее состояние для индикатора в интерфейсе.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Status {
    /// `starting` | `active` | `idle` | `locked`
    pub kind: &'static str,
    pub app: Option<String>,
}

impl Status {
    pub fn starting() -> Self {
        Status { kind: "starting", app: None }
    }

    pub fn from_sample(s: &Sample, idle_threshold_ms: u64, count_idle: bool) -> Self {
        if s.locked {
            Status { kind: "locked", app: None }
        } else if !count_idle && s.idle_ms >= idle_threshold_ms {
            Status { kind: "idle", app: None }
        } else {
            Status { kind: "active", app: s.foreground.as_deref().map(display_name) }
        }
    }
}

/// Строка, ожидающая записи в БД.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRow {
    pub day: String,
    pub exe: String,
    pub delta: Delta,
}

/// Использование приложения за день, как его видит интерфейс.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub exe: String,
    pub name: String,
    pub active_ms: u64,
    pub running_ms: u64,
}

/// Приложение для списков в настройках (исключённые, доступные для исключения).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    pub exe: String,
    pub name: String,
}

/// Приводит имя exe от интерфейса к виду, в котором оно хранится (нижний регистр).
/// Данные из интерфейса не считаются доверенными: пустые строки, пути, управляющие символы
/// и слишком длинные имена отвергаются.
pub fn normalize_exe(raw: &str) -> Option<String> {
    let exe = raw.trim().to_lowercase();
    let bad = |c: char| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
    if exe.is_empty() || exe.chars().count() > 260 || exe.chars().any(bad) {
        return None;
    }
    Some(exe)
}

/// `chrome.exe` -> `Chrome`.
pub fn display_name(exe: &str) -> String {
    let lower = exe.to_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    let mut chars = stem.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Добавляет ещё не записанные в БД данные к строкам из БД и сортирует результат.
pub fn merge_pending(
    mut rows: Vec<AppUsage>,
    pending: &[(String, Delta)],
    names: &HashMap<String, String>,
) -> Vec<AppUsage> {
    let mut index: HashMap<String, usize> =
        rows.iter().enumerate().map(|(i, r)| (r.exe.clone(), i)).collect();
    for (exe, d) in pending {
        match index.get(exe) {
            Some(&i) => {
                rows[i].active_ms += d.active_ms;
                rows[i].running_ms += d.running_ms;
            }
            None => {
                index.insert(exe.clone(), rows.len());
                rows.push(AppUsage {
                    exe: exe.clone(),
                    name: names.get(exe).cloned().unwrap_or_else(|| display_name(exe)),
                    active_ms: d.active_ms,
                    running_ms: d.running_ms,
                });
            }
        }
    }
    rows.retain(|r| r.active_ms > 0 || r.running_ms > 0);
    rows.sort_by(|a, b| {
        b.active_ms
            .cmp(&a.active_ms)
            .then(b.running_ms.cmp(&a.running_ms))
            .then(a.exe.cmp(&b.exe))
    });
    rows
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn sample(idle_ms: u64, locked: bool) -> Sample {
        Sample { day: "d".into(), foreground: Some("chrome.exe".into()), running: vec![], idle_ms, locked }
    }

    #[test]
    fn status_reflects_state_and_idle_toggle() {
        assert_eq!(Status::from_sample(&sample(0, false), 60_000, false).kind, "active");
        assert_eq!(Status::from_sample(&sample(0, false), 60_000, false).app.as_deref(), Some("Chrome"));
        assert_eq!(Status::from_sample(&sample(70_000, false), 60_000, false).kind, "idle");
        assert_eq!(Status::from_sample(&sample(70_000, false), 60_000, true).kind, "active");
        assert_eq!(Status::from_sample(&sample(0, true), 60_000, true).kind, "locked");
    }
}

#[cfg(test)]
mod exe_tests {
    use super::*;

    #[test]
    fn normalize_exe_accepts_names_and_rejects_garbage() {
        assert_eq!(normalize_exe("  Chrome.EXE ").as_deref(), Some("chrome.exe"));
        assert_eq!(normalize_exe("Яндекс.exe").as_deref(), Some("яндекс.exe"));
        for bad in ["", "   ", "a/b.exe", "c:\\x.exe", "a\0.exe", "a\n.exe", "con*.exe", "a|b"] {
            assert_eq!(normalize_exe(bad), None, "{bad:?}");
        }
        assert_eq!(normalize_exe(&"a".repeat(261)), None);
        assert!(normalize_exe(&"a".repeat(260)).is_some());
    }
}
