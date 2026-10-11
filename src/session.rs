//! Sesi offline: token login disimpan terenkripsi di disk dan berlaku sampai batas waktu.
//! Kunci diturunkan dari password dengan PBKDF2, jadi password yang benar satu-satunya cara membukanya.
//! Format berkas: salt (16 byte) | nonce (12 byte) | ciphertext AES-256-GCM.

use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

/// Masa berlaku sesi offline. Dibatasi juga oleh masa token di server: saat sinkron, server tetap memutuskan.
pub const MASA_BERLAKU_DETIK: i64 = 12 * 3600;
const ITERASI: u32 = 100_000;
const SALT: usize = 16;
const NONCE: usize = 12;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Isi {
    email: String,
    token: String,
    /// Waktu berakhir (Unix detik).
    berakhir: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Galat {
    /// Belum pernah login online di perangkat ini.
    Kosong,
    /// Password salah, atau berkas rusak.
    PasswordSalah,
    /// Sesi milik akun lain di perangkat ini.
    AkunBeda,
    Kedaluwarsa,
    Io(String),
}

/// Sesi yang berhasil dibuka: email, token, dan waktu berakhir.
#[derive(Debug, PartialEq)]
pub struct Terbuka {
    pub token: String,
    pub berakhir: i64,
}

fn file() -> Option<PathBuf> {
    crate::cache::dir().map(|d| d.join("sesi.bin"))
}

fn kunci(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, ITERASI, &mut out);
    out
}

/// Menyimpan sesi baru, menggantikan sesi sebelumnya.
pub fn simpan(email: &str, password: &str, token: &str, sekarang: i64) -> Result<(), Galat> {
    match file() {
        Some(p) => simpan_ke(&p, email, password, token, sekarang),
        None => Ok(()),
    }
}

/// Membuka sesi dengan email dan password. Mengembalikan token bila sesinya masih berlaku.
pub fn buka(email: &str, password: &str, sekarang: i64) -> Result<Terbuka, Galat> {
    match file() {
        Some(p) => buka_dari(&p, email, password, sekarang),
        None => Err(Galat::Kosong),
    }
}

/// Menghapus sesi offline, misalnya saat logout.
pub fn hapus() {
    if let Some(p) = file() {
        let _ = std::fs::remove_file(p);
    }
}

pub fn simpan_ke(
    path: &Path,
    email: &str,
    password: &str,
    token: &str,
    sekarang: i64,
) -> Result<(), Galat> {
    let isi = Isi {
        email: email.to_string(),
        token: token.to_string(),
        berakhir: sekarang + MASA_BERLAKU_DETIK,
    };
    let teks = serde_json::to_vec(&isi).map_err(|e| Galat::Io(e.to_string()))?;
    let mut salt = [0u8; SALT];
    let mut nonce = [0u8; NONCE];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher =
        Aes256Gcm::new_from_slice(&kunci(password, &salt)).map_err(|e| Galat::Io(e.to_string()))?;
    let cipher_teks = cipher
        .encrypt(Nonce::from_slice(&nonce), teks.as_ref())
        .map_err(|e| Galat::Io(e.to_string()))?;

    let mut out = Vec::with_capacity(SALT + NONCE + cipher_teks.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&cipher_teks);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Galat::Io(e.to_string()))?;
    }
    let tmp = path.with_extension("bin.tmp");
    std::fs::write(&tmp, out).map_err(|e| Galat::Io(e.to_string()))?;
    std::fs::rename(tmp, path).map_err(|e| Galat::Io(e.to_string()))
}

pub fn buka_dari(
    path: &Path,
    email: &str,
    password: &str,
    sekarang: i64,
) -> Result<Terbuka, Galat> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Galat::Kosong),
        Err(e) => return Err(Galat::Io(e.to_string())),
    };
    if bytes.len() < SALT + NONCE {
        return Err(Galat::PasswordSalah);
    }
    let (salt, sisa) = bytes.split_at(SALT);
    let (nonce, cipher_teks) = sisa.split_at(NONCE);
    let cipher =
        Aes256Gcm::new_from_slice(&kunci(password, salt)).map_err(|e| Galat::Io(e.to_string()))?;
    let teks = cipher
        .decrypt(Nonce::from_slice(nonce), cipher_teks)
        .map_err(|_| Galat::PasswordSalah)?;
    let isi: Isi = serde_json::from_slice(&teks).map_err(|_| Galat::PasswordSalah)?;
    if !isi.email.eq_ignore_ascii_case(email.trim()) {
        return Err(Galat::AkunBeda);
    }
    if sekarang >= isi.berakhir {
        return Err(Galat::Kedaluwarsa);
    }
    Ok(Terbuka {
        token: isi.token,
        berakhir: isi.berakhir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berkas(nama: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arumanis-sesi-{}", std::process::id()));
        dir.join(nama)
    }

    #[test]
    fn simpan_lalu_buka_dengan_password_benar() {
        let p = berkas("ok.bin");
        simpan_ke(&p, "a@b.id", "rahasia", "1|tok", 1_000).unwrap();
        let t = buka_dari(&p, "A@B.id", "rahasia", 2_000).unwrap();
        assert_eq!(t.token, "1|tok");
        assert_eq!(t.berakhir, 1_000 + MASA_BERLAKU_DETIK);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn password_salah_dan_akun_beda_ditolak() {
        let p = berkas("salah.bin");
        simpan_ke(&p, "a@b.id", "rahasia", "1|tok", 1_000).unwrap();
        assert_eq!(
            buka_dari(&p, "a@b.id", "lain", 2_000),
            Err(Galat::PasswordSalah)
        );
        assert_eq!(
            buka_dari(&p, "c@d.id", "rahasia", 2_000),
            Err(Galat::AkunBeda)
        );
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn sesi_kedaluwarsa_ditolak() {
        let p = berkas("exp.bin");
        simpan_ke(&p, "a@b.id", "rahasia", "1|tok", 1_000).unwrap();
        let akhir = 1_000 + MASA_BERLAKU_DETIK;
        assert_eq!(
            buka_dari(&p, "a@b.id", "rahasia", akhir),
            Err(Galat::Kedaluwarsa)
        );
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn belum_ada_sesi_dan_isi_tidak_terbaca() {
        let p = berkas("tidak-ada.bin");
        assert_eq!(buka_dari(&p, "a@b.id", "x", 0), Err(Galat::Kosong));
        let rusak = berkas("rusak.bin");
        std::fs::create_dir_all(rusak.parent().unwrap()).unwrap();
        std::fs::write(&rusak, b"bukan sesi").unwrap();
        assert_eq!(
            buka_dari(&rusak, "a@b.id", "x", 0),
            Err(Galat::PasswordSalah)
        );
        let _ = std::fs::remove_file(rusak);
    }
}
