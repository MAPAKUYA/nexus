use crate::model::{Delta, PendingRow, Sample};
use std::collections::{HashMap, HashSet};

/// Если между двумя замерами прошло больше, это сон/зависание/пауза: время не засчитывается.
pub const DEFAULT_MAX_GAP_MS: u64 = 5_000;
/// Через сколько бездействия пользователь считается ушедшим.
pub const DEFAULT_IDLE_THRESHOLD_MS: u64 = 5 * 60_000;

#[derive(Debug, Clone, Copy)]
pub struct TrackerConfig {
    pub max_gap_ms: u64,
    pub idle_threshold_ms: u64,
    /// Считать время без ввода активным (определение простоя выключено). Блокировка по-прежнему не считается.
    pub count_idle: bool,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self { max_gap_ms: DEFAULT_MAX_GAP_MS, idle_threshold_ms: DEFAULT_IDLE_THRESHOLD_MS, count_idle: false }
    }
}

type Key = (String, String); // (день, exe)

/// Накопитель времени. Данные копятся в памяти и пачкой сбрасываются в БД (см. `Db::flush`),
/// поэтому запись на диск не происходит каждую секунду.
///
/// Простой учитывается без «отката»: пока от последнего ввода прошло больше одного интервала,
/// активное время откладывается в `tentative`. Если ввод вернулся — оно засчитывается (человек
/// просто читал или смотрел). Если бездействие дошло до порога или экран заблокировался —
/// отложенное время отбрасывается, и ушедший от компьютера пользователь «активным» не считается.
pub struct Tracker {
    last_tick: Option<u64>,
    cfg: TrackerConfig,
    acc: HashMap<Key, Delta>,
    tentative: HashMap<Key, u64>,
    /// Исключённые приложения (чёрный список): их время не считается вообще.
    ignored: HashSet<String>,
}

impl Tracker {
    pub fn new(cfg: TrackerConfig) -> Self {
        Self { last_tick: None, cfg, acc: HashMap::new(), tentative: HashMap::new(), ignored: HashSet::new() }
    }

    /// Обрабатывает замер. `now_ms` — монотонное время в мс. Интервал с предыдущего замера
    /// относится к состоянию, зафиксированному в `sample`. Возвращает засчитанные мс.
    pub fn tick(&mut self, now_ms: u64, sample: &Sample) -> u64 {
        let filtered;
        let sample = if self.ignored.is_empty() {
            sample
        } else {
            filtered = sample.without(&self.ignored);
            &filtered
        };
        let Some(prev) = self.last_tick.replace(now_ms) else { return 0 };
        if now_ms <= prev {
            return 0;
        }
        let dt = now_ms - prev;
        if dt > self.cfg.max_gap_ms {
            self.tentative.clear(); // сон, гибернация или зависание: пользователь, скорее всего, ушёл
            return 0;
        }

        let idle_away = !self.cfg.count_idle && sample.idle_ms >= self.cfg.idle_threshold_ms;
        let away = sample.locked || idle_away;
        // Ввод был в течение этого интервала. При выключенном определении простоя время считается сразу.
        let input_seen = !away && (self.cfg.count_idle || sample.idle_ms <= dt);
        if away {
            self.tentative.clear();
        } else if input_seen {
            self.commit_tentative();
        }

        let mut running: HashSet<&str> = sample.running.iter().map(String::as_str).collect();
        let mut active_exe: Option<&str> = None;
        if !away {
            if let Some(fg) = sample.foreground.as_deref() {
                running.insert(fg); // активное время всегда входит во время работы
                active_exe = Some(fg);
            }
        }

        for exe in running {
            self.acc.entry((sample.day.clone(), exe.to_string())).or_default().running_ms += dt;
        }
        if let Some(fg) = active_exe {
            let key = (sample.day.clone(), fg.to_string());
            if input_seen {
                self.acc.entry(key).or_default().active_ms += dt;
            } else {
                *self.tentative.entry(key).or_default() += dt;
            }
        }
        dt
    }

    fn commit_tentative(&mut self) {
        for (key, ms) in self.tentative.drain() {
            self.acc.entry(key).or_default().active_ms += ms;
        }
    }

    /// Засчитывает отложенное время. Вызывается при выходе и завершении сеанса Windows.
    pub fn finalize(&mut self) {
        self.commit_tentative();
    }

    /// Меняет порог простоя на лету (из настроек).
    pub fn set_idle_threshold_ms(&mut self, ms: u64) {
        self.cfg.idle_threshold_ms = ms;
    }

    /// Включает/выключает засчитывание простоя на лету. При включении отложенное время засчитывается.
    pub fn set_count_idle(&mut self, count_idle: bool) {
        self.cfg.count_idle = count_idle;
        if count_idle {
            self.commit_tentative();
        }
    }

    /// Задаёт чёрный список (имена exe в нижнем регистре). Всё, что по этим приложениям уже
    /// накоплено в памяти (в том числе отложенное время), немедленно отбрасывается.
    pub fn set_ignored(&mut self, ignored: HashSet<String>) {
        self.acc.retain(|(_, exe), _| !ignored.contains(exe));
        self.tentative.retain(|(_, exe), _| !ignored.contains(exe));
        self.ignored = ignored;
    }

    pub fn is_ignored(&self, exe: &str) -> bool {
        self.ignored.contains(exe)
    }

    /// Замер так, как его видит учёт: без исключённых приложений. Нужен и для статуса в интерфейсе.
    pub fn visible_sample(&self, sample: &Sample) -> Sample {
        sample.without(&self.ignored)
    }

    /// Имена exe, по которым есть несохранённое время (для списка «доступных для исключения»).
    pub fn pending_exes(&self) -> Vec<String> {
        let mut set: HashSet<&String> = self.acc.keys().map(|(_, e)| e).collect();
        set.extend(self.tentative.keys().map(|(_, e)| e));
        set.into_iter().cloned().collect()
    }

    /// Забывает предыдущий замер, не теряя накопленное.
    pub fn reset_clock(&mut self) {
        self.last_tick = None;
    }

    pub fn has_pending(&self) -> bool {
        !self.acc.is_empty()
    }

    /// Всё, что ещё не записано в БД (включая отложенное активное время), для показа в интерфейсе.
    pub fn pending_all(&self) -> Vec<PendingRow> {
        let mut merged: HashMap<&Key, Delta> = self.acc.iter().map(|(k, d)| (k, *d)).collect();
        for (k, ms) in &self.tentative {
            merged.entry(k).or_default().active_ms += ms;
        }
        merged
            .into_iter()
            .map(|((day, exe), delta)| PendingRow { day: day.clone(), exe: exe.clone(), delta })
            .collect()
    }

    /// Забирает накопленное для записи в БД. Отложенное время остаётся в памяти.
    pub fn take_pending(&mut self) -> Vec<PendingRow> {
        self.acc.drain().map(|((day, exe), delta)| PendingRow { day, exe, delta }).collect()
    }

    /// Возвращает строки обратно, если запись в БД не удалась.
    pub fn restore(&mut self, rows: Vec<PendingRow>) {
        for r in rows {
            let e = self.acc.entry((r.day, r.exe)).or_default();
            e.active_ms += r.delta.active_ms;
            e.running_ms += r.delta.running_ms;
        }
    }

    /// Сбрасывает несохранённые данные (при удалении всей статистики).
    pub fn clear(&mut self) {
        self.acc.clear();
        self.tentative.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THR: u64 = 60_000;

    fn tracker() -> Tracker {
        Tracker::new(TrackerConfig { max_gap_ms: 5_000, idle_threshold_ms: THR, count_idle: false })
    }

    fn s(day: &str, fg: Option<&str>, running: &[&str], idle_ms: u64, locked: bool) -> Sample {
        Sample {
            day: day.into(),
            foreground: fg.map(Into::into),
            running: running.iter().map(|x| x.to_string()).collect(),
            idle_ms,
            locked,
        }
    }

    fn total(t: &Tracker, day: &str, exe: &str) -> Delta {
        t.pending_all().into_iter().find(|r| r.day == day && r.exe == exe).map(|r| r.delta).unwrap_or_default()
    }

    fn present(fg: &str) -> Sample {
        s("d", Some(fg), &[fg], 0, false)
    }

    #[test]
    fn first_tick_credits_nothing() {
        let mut t = tracker();
        assert_eq!(t.tick(1_000, &present("a.exe")), 0);
        assert!(t.pending_all().is_empty());
    }

    #[test]
    fn active_and_running_are_counted_separately() {
        let mut t = tracker();
        t.tick(0, &s("d", Some("a.exe"), &["a.exe", "b.exe"], 0, false));
        for i in 1..=10 {
            t.tick(i * 1_000, &s("d", Some("a.exe"), &["a.exe", "b.exe"], 0, false));
        }
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 10_000, running_ms: 10_000 });
        assert_eq!(total(&t, "d", "b.exe"), Delta { active_ms: 0, running_ms: 10_000 });
    }

    #[test]
    fn walking_away_is_never_counted_as_active() {
        // Последний ввод в момент 0; дальше тишина 100 секунд, порог 60 с.
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        for i in 1..=100u64 {
            t.tick(i * 1_000, &s("d", Some("a.exe"), &["a.exe"], i * 1_000, false));
            if i == 30 {
                // пока порог не достигнут, время показывается в интерфейсе как отложенное
                assert_eq!(total(&t, "d", "a.exe").active_ms, 30_000);
            }
        }
        let got = total(&t, "d", "a.exe");
        assert_eq!(got.active_ms, 1_000, "засчитана только секунда с реальным вводом");
        assert_eq!(got.running_ms, 100_000, "приложение при этом продолжало работать");
    }

    #[test]
    fn reading_without_input_is_credited_when_user_returns() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        for i in 1..=30u64 {
            t.tick(i * 1_000, &s("d", Some("a.exe"), &["a.exe"], i * 1_000, false));
        }
        t.tick(31_000, &present("a.exe")); // ввод вернулся
        assert_eq!(total(&t, "d", "a.exe").active_ms, 31_000);
        let _ = t.take_pending();
        assert!(t.pending_all().is_empty(), "отложенное время не должно остаться после коммита");
    }

    #[test]
    fn idle_threshold_boundary_is_away() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], THR - 1, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 1_000);
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], THR, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 0, "на пороге отложенное время отбрасывается");
    }

    #[test]
    fn locked_screen_is_not_active_but_apps_keep_running() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        for i in 2..=11u64 {
            t.tick(i * 1_000, &s("d", Some("lockapp.exe"), &["a.exe"], 0, true));
        }
        let a = total(&t, "d", "a.exe");
        assert_eq!((a.active_ms, a.running_ms), (1_000, 11_000));
        assert_eq!(total(&t, "d", "lockapp.exe"), Delta::default());
    }

    #[test]
    fn lock_discards_tentative_time() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        for i in 1..=20u64 {
            t.tick(i * 1_000, &s("d", Some("a.exe"), &["a.exe"], i * 1_000, false));
        }
        t.tick(21_000, &s("d", Some("a.exe"), &["a.exe"], 21_000, true)); // экран заблокировался сам
        t.tick(22_000, &present("a.exe")); // и разблокирован — ввод
        // 1 с — реальный ввод в начале (i = 1); 19 с «тишины» перед блокировкой отброшены;
        // ещё 1 с — ввод после разблокировки.
        assert_eq!(total(&t, "d", "a.exe").active_ms, 2_000);
    }

    #[test]
    fn input_resets_idle_and_resumes_counting() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], THR + 5, false)); // ушёл
        t.tick(2_000, &present("a.exe")); // вернулся
        t.tick(3_000, &present("a.exe"));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 2_000);
    }

    #[test]
    fn foreground_implies_running_even_if_missing_from_list() {
        let mut t = tracker();
        t.tick(0, &s("d", Some("a.exe"), &[], 0, false));
        t.tick(1_000, &s("d", Some("a.exe"), &[], 0, false));
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 1_000, running_ms: 1_000 });
    }

    #[test]
    fn duplicates_in_running_list_are_counted_once() {
        let mut t = tracker();
        t.tick(0, &s("d", None, &["a.exe", "a.exe"], 0, false));
        t.tick(1_000, &s("d", None, &["a.exe", "a.exe"], 0, false));
        assert_eq!(total(&t, "d", "a.exe").running_ms, 1_000);
    }

    #[test]
    fn no_foreground_means_no_active_time() {
        let mut t = tracker();
        t.tick(0, &s("d", None, &["a.exe"], 0, false));
        t.tick(1_000, &s("d", None, &["a.exe"], 0, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 0);
    }

    #[test]
    fn large_gap_from_sleep_is_discarded_with_tentative() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], 5_000, false)); // отложено 1 с
        let slept = 2_000 + 8 * 3_600_000;
        assert_eq!(t.tick(slept, &present("a.exe")), 0);
        t.tick(slept + 1_000, &present("a.exe"));
        // 1 с до сна + 1 с после; отложенная секунда перед сном отброшена
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 2_000, running_ms: 3_000 });
    }

    #[test]
    fn finalize_commits_tentative() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], 10_000, false));
        assert!(t.take_pending().iter().all(|r| r.delta.active_ms == 0));
        t.finalize();
        assert_eq!(t.take_pending()[0].delta.active_ms, 1_000);
    }

    #[test]
    fn clock_going_backwards_is_ignored() {
        let mut t = tracker();
        t.tick(10_000, &present("a.exe"));
        assert_eq!(t.tick(9_000, &present("a.exe")), 0);
        assert_eq!(t.tick(10_000, &present("a.exe")), 1_000);
    }

    #[test]
    fn midnight_rollover_splits_days() {
        let mut t = tracker();
        t.tick(0, &s("2026-01-01", Some("a.exe"), &["a.exe"], 0, false));
        t.tick(1_000, &s("2026-01-01", Some("a.exe"), &["a.exe"], 0, false));
        t.tick(2_000, &s("2026-01-02", Some("a.exe"), &["a.exe"], 0, false));
        assert_eq!(total(&t, "2026-01-01", "a.exe").active_ms, 1_000);
        assert_eq!(total(&t, "2026-01-02", "a.exe").active_ms, 1_000);
    }

    #[test]
    fn take_pending_drains_and_restore_puts_back() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        let rows = t.take_pending();
        assert_eq!(rows.len(), 1);
        assert!(!t.has_pending());
        t.tick(2_000, &present("a.exe"));
        t.restore(rows);
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 2_000, running_ms: 2_000 });
    }

    #[test]
    fn reset_clock_skips_next_interval_but_keeps_data() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        t.reset_clock();
        assert_eq!(t.tick(2_000, &present("a.exe")), 0);
        assert_eq!(total(&t, "d", "a.exe").active_ms, 1_000);
    }

    #[test]
    fn clear_drops_everything() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], 10_000, false));
        t.clear();
        assert!(t.pending_all().is_empty());
    }

    #[test]
    fn changing_threshold_applies_immediately() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        // 30 с тишины: при пороге 60 с это ещё «на месте» (отложено), при 20 с — уже «ушёл».
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], 30_000, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 1_000);
        t.set_idle_threshold_ms(20_000);
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], 31_000, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 0);
    }

    fn counting_idle_tracker() -> Tracker {
        Tracker::new(TrackerConfig { max_gap_ms: 5_000, idle_threshold_ms: THR, count_idle: true })
    }

    #[test]
    fn with_idle_counting_on_long_silence_is_still_active() {
        let mut t = counting_idle_tracker();
        t.tick(0, &present("a.exe"));
        for i in 1..=200u64 {
            t.tick(i * 1_000, &s("d", Some("a.exe"), &["a.exe"], i * 1_000, false)); // тишина дольше порога
        }
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 200_000, running_ms: 200_000 });
        let _ = t.take_pending();
        assert!(t.pending_all().is_empty(), "ничего не должно откладываться");
    }

    #[test]
    fn with_idle_counting_on_locked_screen_is_still_excluded() {
        let mut t = counting_idle_tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], 0, true));
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], 90_000, true));
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 0, running_ms: 2_000 });
    }

    #[test]
    fn enabling_idle_counting_commits_tentative_time() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &s("d", Some("a.exe"), &["a.exe"], 20_000, false)); // отложено
        t.set_count_idle(true);
        assert_eq!(t.take_pending()[0].delta.active_ms, 1_000);
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], 500_000, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 1_000, "после включения тишина считается сразу");
        t.set_count_idle(false);
        t.tick(3_000, &s("d", Some("a.exe"), &["a.exe"], 500_000, false));
        assert_eq!(total(&t, "d", "a.exe").active_ms, 1_000, "после выключения — снова простой");
    }
    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn ignored_app_is_never_counted_neither_running_nor_active() {
        let mut t = tracker();
        t.set_ignored(set(&["secret.exe"]));
        let smp = s("d", Some("secret.exe"), &["secret.exe", "b.exe"], 0, false);
        t.tick(0, &smp);
        for i in 1..=10 {
            t.tick(i * 1_000, &smp);
        }
        assert_eq!(total(&t, "d", "secret.exe"), Delta::default());
        assert_eq!(total(&t, "d", "b.exe"), Delta { active_ms: 0, running_ms: 10_000 });
        assert!(t.pending_all().iter().all(|r| r.exe != "secret.exe"));
    }

    #[test]
    fn focus_on_ignored_app_credits_nobody() {
        let mut t = tracker();
        t.set_ignored(set(&["secret.exe"]));
        let smp = s("d", Some("secret.exe"), &["secret.exe", "b.exe"], 0, false);
        t.tick(0, &smp);
        t.tick(1_000, &smp);
        assert_eq!(total(&t, "d", "b.exe").active_ms, 0, "фокус на исключённом не засчитывается соседу");
    }

    #[test]
    fn adding_to_blacklist_drops_pending_and_tentative_time() {
        let mut t = tracker();
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        // отложенное время: тишина дольше интервала, но меньше порога
        t.tick(2_000, &s("d", Some("a.exe"), &["a.exe"], 30_000, false));
        assert!(total(&t, "d", "a.exe").running_ms > 0);
        assert!(t.pending_exes().contains(&"a.exe".to_string()));
        t.set_ignored(set(&["a.exe"]));
        assert_eq!(total(&t, "d", "a.exe"), Delta::default());
        assert!(!t.pending_exes().contains(&"a.exe".to_string()));
        t.finalize();
        assert!(t.take_pending().is_empty(), "отложенное время не должно воскреснуть при finalize");
    }

    #[test]
    fn removing_from_blacklist_resumes_counting_without_old_data() {
        let mut t = tracker();
        t.set_ignored(set(&["a.exe"]));
        t.tick(0, &present("a.exe"));
        t.tick(1_000, &present("a.exe"));
        assert_eq!(total(&t, "d", "a.exe"), Delta::default());
        t.set_ignored(HashSet::new());
        t.tick(2_000, &present("a.exe"));
        t.tick(3_000, &present("a.exe"));
        assert_eq!(total(&t, "d", "a.exe"), Delta { active_ms: 2_000, running_ms: 2_000 });
    }

    #[test]
    fn visible_sample_hides_ignored_for_status() {
        let mut t = tracker();
        t.set_ignored(set(&["a.exe"]));
        let v = t.visible_sample(&present("a.exe"));
        assert_eq!(v.foreground, None);
        assert!(v.running.is_empty());
        assert!(t.is_ignored("a.exe") && !t.is_ignored("b.exe"));
    }
}
