// Settings that persist in settings.txt next to the executable.

use crate::win::*;
use std::fs;
use std::path::PathBuf;

pub const SETTINGS_FILE: &str = "settings.txt";

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub zoom_step: f32,
    pub hide_blank: bool,
    pub show_checksums: bool,
    pub preview_width: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            zoom_step: 0.5,
            hide_blank: false,
            show_checksums: true,
            preview_width: -1,
        }
    }
}

/// Folder that holds the executable (settings.txt lives there).
pub fn exe_dir() -> PathBuf {
    unsafe {
        let mut buf = vec![0u16; 1024];
        let len = GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) as usize;
        if len == 0 {
            return PathBuf::from(".");
        }
        let path = PathBuf::from(from_wide(buf.as_ptr()));
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

pub fn path() -> PathBuf {
    exe_dir().join(SETTINGS_FILE)
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn load() -> Settings {
    let mut settings = Settings::default();
    let Ok(text) = fs::read_to_string(path()) else {
        return settings;
    };

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim().to_lowercase().as_str() {
            "zoom_step" | "zoom" => {
                if let Ok(v) = value.trim().replace(',', ".").parse::<f32>() {
                    if v.is_finite() {
                        settings.zoom_step = v.clamp(0.01, 10.0);
                    }
                }
            }
            "hide_blank" => settings.hide_blank = parse_bool(value),
            "show_checksums" => settings.show_checksums = parse_bool(value),
            "preview_width" => {
                if let Ok(v) = value.trim().parse::<i32>() {
                    settings.preview_width = v;
                }
            }
            _ => {}
        }
    }

    settings
}

pub fn save(settings: &Settings) {
    let text = format!(
        "# Thumbcache Viewer settings\n\
         # Read at startup and rewritten whenever a setting changes.\n\
         zoom_step={:.2}\n\
         hide_blank={}\n\
         show_checksums={}\n\
         preview_width={}\n",
        settings.zoom_step,
        if settings.hide_blank { 1 } else { 0 },
        if settings.show_checksums { 1 } else { 0 },
        settings.preview_width
    );
    let _ = fs::write(path(), text);
}
