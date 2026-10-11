//! Cache baca offline: hasil daftar terakhir disimpan sebagai JSON per modul, tahun, dan halaman.
//! Hanya data daftar. Token login tidak pernah ditulis ke disk.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::Module;

#[derive(Serialize, Deserialize)]
struct Snapshot {
    saved_at: i64,
    rows: Vec<Value>,
}

/// Folder cache: `%LOCALAPPDATA%\arumanis-tui` di Windows, `~/.cache/arumanis-tui` di sistem lain.
pub fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("arumanis-tui"))
}

fn file_for(m: Module, year: i32, page: u32, search: &str) -> Option<PathBuf> {
    let key: String = search
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(40)
        .collect();
    let name = format!("{}-{year}-p{page}-{key}.json", m.title().replace(' ', "_").to_lowercase());
    dir().map(|d| d.join(name))
}

pub fn save(m: Module, year: i32, page: u32, search: &str, rows: &[Value]) {
    let Some(path) = file_for(m, year, page, search) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let snap = Snapshot {
        saved_at: chrono::Utc::now().timestamp(),
        rows: rows.to_vec(),
    };
    if let Ok(text) = serde_json::to_string(&snap) {
        let _ = std::fs::write(path, text);
    }
}

/// Baris tersimpan dan waktu penyimpanannya (Unix detik). `None` bila belum ada atau rusak.
pub fn load(m: Module, year: i32, page: u32, search: &str) -> Option<(Vec<Value>, i64)> {
    let path = file_for(m, year, page, search)?;
    let text = std::fs::read_to_string(path).ok()?;
    let snap: Snapshot = serde_json::from_str(&text).ok()?;
    Some((snap.rows, snap.saved_at))
}

/// Waktu simpan untuk ditampilkan di status bar, dalam zona waktu lokal.
pub fn format_time(unix: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(unix, 0)
        .single()
        .map(|t| t.format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_else(|| "tidak diketahui".to_string())
}

/// Tahun untuk dropdown: tahun berjalan + 1 sampai 2020.
pub fn year_options(current: i32) -> Vec<i32> {
    (2020..=current + 1).rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn year_options_urut_turun_dan_mulai_tahun_berjalan() {
        let opts = year_options(2026);
        assert_eq!(opts.first(), Some(&2027));
        assert_eq!(opts.last(), Some(&2020));
        assert!(opts.contains(&2026));
    }

    #[test]
    fn file_cache_berbeda_per_tahun_dan_halaman() {
        let a = file_for(Module::Kontrak, 2026, 1, "").unwrap();
        let b = file_for(Module::Kontrak, 2025, 1, "").unwrap();
        let c = file_for(Module::Kontrak, 2026, 2, "").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn save_lalu_load_mengembalikan_baris() {
        // Pakai folder sementara supaya tidak menyentuh cache pengguna.
        let tmp = std::env::temp_dir().join(format!("arumanis-tui-test-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &tmp);
        let rows = vec![json!({ "id": 1, "nomor": "A" })];
        save(Module::DocumentRegister, 2026, 1, "surat", &rows);
        let (loaded, saved_at) = load(Module::DocumentRegister, 2026, 1, "surat").expect("cache ada");
        assert_eq!(loaded, rows);
        assert!(saved_at > 0);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
