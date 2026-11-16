//! What the menu remembers between games, in `$XDG_STATE_HOME/asciiarena/settings`
//! (`~/.local/state/asciiarena` without it, `%LOCALAPPDATA%\asciiarena` on Windows).
//! Plain `key = value` lines.

use std::collections::{BTreeMap};
use std::path::{PathBuf};

pub fn state_dir() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None if cfg!(windows) => PathBuf::from(std::env::var_os("LOCALAPPDATA")?),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".local").join("state"),
    };
    let dir = base.join("asciiarena");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Settings {
    values: BTreeMap<String, String>,
}

impl Settings {
    pub fn load() -> Settings {
        state_dir()
            .and_then(|dir| std::fs::read_to_string(dir.join("settings")).ok())
            .map(|text| Settings::parse(&text))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Settings {
        let values = text
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect();
        Settings { values }
    }

    pub fn to_text(&self) -> String {
        self.values.iter().map(|(key, value)| format!("{} = {}\n", key, value)).collect()
    }

    pub fn save(&self) {
        if let Some(dir) = state_dir() {
            if let Err(error) = std::fs::write(dir.join("settings"), self.to_text()) {
                log::warn!("Can not save settings: {}", error);
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|value| value.as_str())
    }

    pub fn get_number(&self, key: &str) -> Option<usize> {
        self.get(key).and_then(|value| value.parse().ok())
    }

    pub fn set(&mut self, key: &str, value: impl ToString) {
        self.values.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut settings = Settings::default();
        settings.set("letter", 'Q');
        settings.set("bots", 4);
        settings.set("server", "192.168.1.20:3549");
        let read = Settings::parse(&settings.to_text());
        assert_eq!(read, settings);
        assert_eq!(read.get_number("bots"), Some(4));
        assert_eq!(read.get("server"), Some("192.168.1.20:3549"));
        assert_eq!(Settings::parse("# hello\nbroken line\nx=1").get("x"), Some("1"));
    }
}
