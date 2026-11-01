use crate::dates::{Date, Period};
use crate::db::{Db, Result};
use crate::model::{merge_pending, AppUsage, Delta, PendingRow};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    /// `YYYY-MM-DD` или `YYYY-MM` (см. `Report::granularity`).
    pub key: String,
    pub active_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub period: &'static str,
    pub from: String,
    pub to: String,
    /// Опорная дата периода (для навигации и выбора даты).
    pub anchor: String,
    pub prev: Option<String>,
    pub next: Option<String>,
    /// Период содержит сегодняшний день.
    pub is_current: bool,
    /// `day` или `month` — по чему сгруппированы `buckets`.
    pub granularity: &'static str,
    pub apps: Vec<AppUsage>,
    /// История активности, новые записи сверху; только дни/месяцы с активностью.
    pub buckets: Vec<Bucket>,
    pub total_active_ms: u64,
}

/// Собирает отчёт за период: данные из БД плюс ещё не сброшенное из памяти.
pub fn build_report(db: &Db, pending: &[PendingRow], period: Period, anchor: Date, today: Date) -> Result<Report> {
    let (from, to) = period.range(anchor);
    let monthly = matches!(period, Period::Year | Period::All);

    let in_range: Vec<&PendingRow> = pending.iter().filter(|r| r.day >= from && r.day <= to).collect();

    // Приложения
    let mut by_exe: HashMap<&str, Delta> = HashMap::new();
    for r in &in_range {
        let e = by_exe.entry(r.exe.as_str()).or_default();
        e.active_ms += r.delta.active_ms;
        e.running_ms += r.delta.running_ms;
    }
    let pend_list: Vec<(String, Delta)> = by_exe.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let apps = merge_pending(db.apps_in_range(&from, &to)?, &pend_list, &db.app_names()?);

    // История
    let mut map: BTreeMap<String, u64> = db.buckets_in_range(&from, &to, monthly)?.into_iter().collect();
    for r in &in_range {
        let key = if monthly { r.day.get(..7).unwrap_or(&r.day) } else { r.day.as_str() };
        *map.entry(key.to_string()).or_default() += r.delta.active_ms;
    }
    let buckets: Vec<Bucket> = map
        .into_iter()
        .rev()
        .filter(|(_, ms)| *ms > 0)
        .map(|(key, active_ms)| Bucket { key, active_ms })
        .collect();

    let is_current = period == Period::All || (from.as_str() <= today.to_string().as_str() && today.to_string().as_str() <= to.as_str());
    let prev = period.shift(anchor, -1).map(|d| d.to_string());
    let today_s = today.to_string();
    let next = period
        .shift(anchor, 1)
        .filter(|d| period.range(*d).0 <= today_s)
        .map(|d| d.to_string());

    Ok(Report {
        period: period.as_str(),
        total_active_ms: apps.iter().map(|a| a.active_ms).sum(),
        from,
        to,
        anchor: anchor.to_string(),
        prev,
        next,
        is_current,
        granularity: if monthly { "month" } else { "day" },
        apps,
        buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn row(day: &str, exe: &str, a: u64, r: u64) -> PendingRow {
        PendingRow { day: day.into(), exe: exe.into(), delta: Delta { active_ms: a, running_ms: r } }
    }

    fn seeded() -> Db {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[
            row("2026-10-25", "a.exe", 1_000, 5_000), // воскресенье — прошлая неделя
            row("2026-10-26", "a.exe", 2_000, 6_000), // понедельник
            row("2026-10-26", "b.exe", 3_000, 3_000),
            row("2026-10-30", "a.exe", 4_000, 8_000), // пятница
            row("2026-09-10", "a.exe", 10_000, 10_000),
            row("2025-05-05", "b.exe", 7_000, 9_000),
        ])
        .unwrap();
        db
    }

    #[test]
    fn day_report() {
        let r = build_report(&seeded(), &[], Period::Day, d("2026-10-26"), d("2026-10-30")).unwrap();
        assert_eq!((r.from.as_str(), r.to.as_str()), ("2026-10-26", "2026-10-26"));
        assert_eq!(r.total_active_ms, 5_000);
        assert_eq!(r.apps[0].exe, "b.exe");
        assert!(!r.is_current);
        assert_eq!(r.next.as_deref(), Some("2026-10-27"));
        assert_eq!(r.prev.as_deref(), Some("2026-10-25"));
    }

    #[test]
    fn week_report_sums_days_and_excludes_neighbours() {
        let r = build_report(&seeded(), &[], Period::Week, d("2026-10-30"), d("2026-10-30")).unwrap();
        assert_eq!((r.from.as_str(), r.to.as_str()), ("2026-10-26", "2026-11-01"));
        assert!(r.is_current);
        assert_eq!(r.total_active_ms, 2_000 + 3_000 + 4_000);
        let a = r.apps.iter().find(|x| x.exe == "a.exe").unwrap();
        assert_eq!((a.active_ms, a.running_ms), (6_000, 14_000));
        assert_eq!(r.granularity, "day");
        assert_eq!(r.buckets.iter().map(|b| b.key.as_str()).collect::<Vec<_>>(), ["2026-10-30", "2026-10-26"]);
        assert!(r.next.is_none(), "в будущее листать нельзя");
    }

    #[test]
    fn month_and_year_reports() {
        let m = build_report(&seeded(), &[], Period::Month, d("2026-10-15"), d("2026-10-30")).unwrap();
        assert_eq!(m.total_active_ms, 1_000 + 2_000 + 3_000 + 4_000);
        let y = build_report(&seeded(), &[], Period::Year, d("2026-10-15"), d("2026-10-30")).unwrap();
        assert_eq!(y.granularity, "month");
        assert_eq!(y.total_active_ms, 10_000 + 10_000);
        assert_eq!(y.buckets.iter().map(|b| (b.key.as_str(), b.active_ms)).collect::<Vec<_>>(), [("2026-10", 10_000), ("2026-09", 10_000)]);
    }

    #[test]
    fn all_time_report_has_no_navigation() {
        let r = build_report(&seeded(), &[], Period::All, d("2026-10-30"), d("2026-10-30")).unwrap();
        assert_eq!(r.total_active_ms, 27_000);
        assert!(r.prev.is_none() && r.next.is_none() && r.is_current);
        assert_eq!(r.buckets.last().unwrap().key, "2025-05");
    }

    #[test]
    fn pending_data_is_merged_into_every_period() {
        let pending = vec![row("2026-10-30", "a.exe", 500, 500), row("2026-10-30", "c.exe", 9_000, 9_000)];
        let db = seeded();
        for p in [Period::Day, Period::Week, Period::Month, Period::Year, Period::All] {
            let with = build_report(&db, &pending, p, d("2026-10-30"), d("2026-10-30")).unwrap();
            let without = build_report(&db, &[], p, d("2026-10-30"), d("2026-10-30")).unwrap();
            assert_eq!(with.total_active_ms, without.total_active_ms + 9_500, "{p:?}");
            assert!(with.apps.iter().any(|a| a.exe == "c.exe"));
        }
    }

    #[test]
    fn pending_outside_range_is_ignored() {
        let pending = vec![row("2026-10-30", "a.exe", 5_000, 5_000)];
        let r = build_report(&seeded(), &pending, Period::Day, d("2026-10-26"), d("2026-10-30")).unwrap();
        assert_eq!(r.total_active_ms, 5_000);
    }

    #[test]
    fn empty_database_gives_empty_report() {
        let db = Db::open_in_memory().unwrap();
        let r = build_report(&db, &[], Period::Week, d("2026-10-30"), d("2026-10-30")).unwrap();
        assert!(r.apps.is_empty() && r.buckets.is_empty());
        assert_eq!(r.total_active_ms, 0);
    }

    #[test]
    fn running_only_apps_show_up_but_zero_active_days_do_not() {
        let mut db = Db::open_in_memory().unwrap();
        db.flush(&[row("2026-10-30", "a.exe", 0, 4_000)]).unwrap();
        let r = build_report(&db, &[], Period::Day, d("2026-10-30"), d("2026-10-30")).unwrap();
        assert_eq!(r.apps.len(), 1);
        assert!(r.buckets.is_empty());
    }

    #[test]
    fn json_shape_is_camel_case() {
        let r = build_report(&seeded(), &[], Period::Day, d("2026-10-26"), d("2026-10-30")).unwrap();
        let v = serde_json::to_value(&r).unwrap();
        for key in ["totalActiveMs", "isCurrent", "granularity", "apps", "buckets", "prev", "next", "anchor"] {
            assert!(v.get(key).is_some(), "нет поля {key}");
        }
        assert!(v["apps"][0].get("activeMs").is_some() && v["apps"][0].get("runningMs").is_some());
        assert!(v["buckets"][0].get("activeMs").is_some());
    }

    #[test]
    fn pending_only_app_uses_friendly_name_from_db() {
        let mut db = Db::open_in_memory().unwrap();
        db.set_app_name("new.exe", "Новая программа").unwrap();
        let pending = vec![row("2026-10-30", "new.exe", 1_000, 1_000), row("2026-10-30", "other.exe", 500, 500)];
        let r = build_report(&db, &pending, Period::Day, d("2026-10-30"), d("2026-10-30")).unwrap();
        assert_eq!(r.apps.iter().find(|a| a.exe == "new.exe").unwrap().name, "Новая программа");
        assert_eq!(r.apps.iter().find(|a| a.exe == "other.exe").unwrap().name, "Other");
    }
}
