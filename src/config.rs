use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub theme: Option<String>,
    pub font_size: Option<u32>,
    pub show_toc: Option<bool>,
    /// Body face: "sans" (黑体) or "serif" (宋体).
    pub face: Option<String>,
    pub recent: Vec<String>,
    pub window: WindowCfg,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct WindowCfg {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub maximized: bool,
}

impl Config {
    pub fn dir() -> PathBuf {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("MoyueMD")
    }

    pub fn file() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::file())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = Self::dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::file(), raw);
        }
    }

    pub fn theme(&self) -> String {
        match self.theme.as_deref() {
            Some("light") => "light".into(),
            _ => "dark".into(),
        }
    }

    pub fn font_size(&self) -> u32 {
        self.font_size.unwrap_or(17).clamp(12, 30)
    }

    pub fn face(&self) -> String {
        match self.face.as_deref() {
            Some("serif") => "serif".into(),
            _ => "sans".into(),
        }
    }

    pub fn show_toc(&self) -> bool {
        self.show_toc.unwrap_or(true)
    }

    /// Most-recent-first, de-duplicated, capped at 10 entries.
    pub fn push_recent(&mut self, path: &str) {
        self.recent.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(10);
    }

    pub fn drop_recent(&mut self, path: &str) {
        self.recent.retain(|p| !p.eq_ignore_ascii_case(path));
    }
}
