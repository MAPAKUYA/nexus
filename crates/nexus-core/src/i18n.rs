//! Тексты, которые показывает не веб-интерфейс, а сама оболочка Windows (меню и подсказка значка в трее).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayText {
    pub open: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
}

/// Тексты трея для языка; неизвестный код даёт английский.
pub fn tray_text(lang: &str) -> TrayText {
    match lang {
        "ru" => TrayText { open: "Открыть NEXUS", quit: "Выйти", tooltip: "NEXUS — учёт времени" },
        "es" => TrayText { open: "Abrir NEXUS", quit: "Salir", tooltip: "NEXUS — control de tiempo" },
        "de" => TrayText { open: "NEXUS öffnen", quit: "Beenden", tooltip: "NEXUS — Zeiterfassung" },
        "fr" => TrayText { open: "Ouvrir NEXUS", quit: "Quitter", tooltip: "NEXUS — suivi du temps" },
        "pt" => TrayText { open: "Abrir o NEXUS", quit: "Sair", tooltip: "NEXUS — controle de tempo" },
        "zh" => TrayText { open: "打开 NEXUS", quit: "退出", tooltip: "NEXUS — 时间统计" },
        "ja" => TrayText { open: "NEXUS を開く", quit: "終了", tooltip: "NEXUS — 時間トラッカー" },
        _ => TrayText { open: "Open NEXUS", quit: "Quit", tooltip: "NEXUS — time tracker" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SUPPORTED_LANGUAGES;

    #[test]
    fn every_supported_language_has_distinct_nonempty_texts() {
        let mut seen = std::collections::HashSet::new();
        for code in SUPPORTED_LANGUAGES {
            let t = tray_text(code);
            assert!(!t.open.is_empty() && !t.quit.is_empty() && !t.tooltip.is_empty(), "{code}");
            assert!(t.open.contains("NEXUS") && t.tooltip.contains("NEXUS"), "{code}");
            assert!(seen.insert(t.open), "тексты языка {code} совпали с другим языком");
        }
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        assert_eq!(tray_text("xx"), tray_text("en"));
        assert_eq!(tray_text(""), tray_text("en"));
        assert_eq!(tray_text("en").quit, "Quit");
    }
}
