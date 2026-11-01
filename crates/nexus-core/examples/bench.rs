//! Замеры производительности ядра (не входит в приложение): `cargo run --release --example bench -p nexus-core`.
use nexus_core::*;
use std::time::Instant;

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse().ok())))
        .unwrap_or(0)
}

fn main() {
    let apps: Vec<String> = (0..30).map(|i| format!("app{i}.exe")).collect();
    let running: Vec<String> = apps.iter().take(25).cloned().collect();

    // 1. Один замер трекера
    let mut t = Tracker::new(TrackerConfig::default());
    let n = 2_000_000u64;
    let sample = Sample { day: "2026-10-30".into(), foreground: Some(apps[3].clone()), running: running.clone(), idle_ms: 0, locked: false };
    let start = Instant::now();
    for i in 0..n {
        t.tick(i * 1000, &sample);
    }
    let per_tick = start.elapsed().as_nanos() as f64 / n as f64;
    println!("tick(): {:.0} нс на замер (25 запущенных приложений); при 1 замере/с это {:.6}% одного ядра", per_tick, per_tick / 1e9 * 100.0);

    // 2. Запись пачки в файловую БД
    let path = std::env::temp_dir().join(format!("nexus-bench-{}.sqlite3", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut db = Db::open(&path).unwrap();
    let mut t = Tracker::new(TrackerConfig::default());
    for i in 0..31u64 {
        t.tick(i * 1000, &Sample { running: running.clone(), ..sample.clone() });
    }
    let rows = t.take_pending();
    let runs = 200;
    let start = Instant::now();
    for _ in 0..runs {
        db.flush(&rows).unwrap();
    }
    println!("flush(): {:.2} мс на запись {} строк в файл на диске (раз в 30 с)", start.elapsed().as_secs_f64() * 1000.0 / runs as f64, rows.len());

    // 3. Пять лет данных: 30 приложений × 1826 дней
    let _ = std::fs::remove_file(&path);
    for ext in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
    let mut db = Db::open(&path).unwrap();
    let mut day = Date::parse("2021-11-01").unwrap();
    let mut batch = Vec::new();
    let mut total_rows = 0usize;
    for d in 0..1826 {
        for (i, a) in apps.iter().enumerate() {
            batch.push(PendingRow { day: day.to_string(), exe: a.clone(), delta: Delta { active_ms: ((d * 7 + i * 13) % 5000) as u64 * 1000, running_ms: 20_000_000 } });
            total_rows += 1;
        }
        day = day.add_days(1).unwrap();
        if batch.len() > 3000 {
            db.flush(&batch).unwrap();
            batch.clear();
        }
    }
    db.flush(&batch).unwrap();
    drop(db);
    let size = std::fs::metadata(&path).unwrap().len();
    println!("\nБаза за 5 лет: {} строк, файл {:.2} МБ", total_rows, size as f64 / 1_048_576.0);

    let db = Db::open(&path).unwrap();
    let today = Date::parse("2026-10-30").unwrap();
    let pending = {
        let mut t = Tracker::new(TrackerConfig::default());
        for i in 0..31u64 {
            t.tick(i * 1000, &Sample { running: running.clone(), ..sample.clone() });
        }
        t.pending_all()
    };
    let before = rss_kb();
    for (name, p) in [("день", Period::Day), ("неделя", Period::Week), ("месяц", Period::Month), ("год", Period::Year), ("всё время", Period::All)] {
        let runs = 50;
        let start = Instant::now();
        let mut apps_n = 0;
        for _ in 0..runs {
            let r = build_report(&db, &pending, p, today, today).unwrap();
            apps_n = r.apps.len();
        }
        println!("отчёт «{name}»: {:.2} мс (приложений в ответе: {apps_n})", start.elapsed().as_secs_f64() * 1000.0 / runs as f64);
    }
    println!("\nRSS процесса замера: {} КБ → {} КБ", before, rss_kb());
    drop(db);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
}
