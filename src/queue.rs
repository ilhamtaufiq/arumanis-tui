//! Antrean tulis offline: perubahan yang gagal karena tidak ada koneksi disimpan di disk dan dikirim ulang saat online.
//! Isinya body JSON seperti yang akan dikirim. Token login tidak pernah ditulis ke sini.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pending {
    pub id: u64,
    /// "pekerjaan" atau "register", sesuai modul tujuan.
    pub modul: String,
    /// "POST" atau "PUT".
    pub metode: String,
    pub path: String,
    pub body: Value,
    /// Judul singkat untuk daftar.
    pub label: String,
    /// Waktu dibuat (Unix detik).
    pub dibuat: i64,
    /// Pesan penolakan dari server. Terisi bila server menolak saat sinkron.
    #[serde(default)]
    pub galat: Option<String>,
}

fn file() -> Option<PathBuf> {
    crate::cache::dir().map(|d| d.join("antrean.json"))
}

pub fn load_from(path: &Path) -> Vec<Pending> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, items: &[Pending]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string(items).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

pub fn load() -> Vec<Pending> {
    file().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn save(items: &[Pending]) -> std::io::Result<()> {
    match file() {
        Some(p) => save_to(&p, items),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simpan_lalu_muat_mengembalikan_antrean_utuh() {
        let dir = std::env::temp_dir().join(format!("arumanis-antrean-{}", std::process::id()));
        let path = dir.join("antrean.json");
        let items = vec![Pending {
            id: 1,
            modul: "register".into(),
            metode: "POST".into(),
            path: "document-registers".into(),
            body: json!({ "kontrak_id": 7, "type_id": 2 }),
            label: "Register baru di kontrak #7".into(),
            dibuat: 1_700_000_000,
            galat: Some("Nomor sudah dipakai.".into()),
        }];
        save_to(&path, &items).expect("tulis");
        assert_eq!(load_from(&path), items);
        assert!(load_from(&dir.join("tidak-ada.json")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
