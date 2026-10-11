//! Klien HTTP ke apiamis. Panggilan bersifat blocking; token hanya disimpan di memori.

use std::fmt;

use anyhow::Result;
use reqwest::blocking::{Client as Http, Response};
use reqwest::header::{AUTHORIZATION, SET_COOKIE};
use reqwest::Method;
use serde_json::{json, Value};

pub const DEFAULT_BASE: &str = "https://apiamis.cianjur.space/api";
const TOKEN_COOKIE: &str = "arumanis_token";
const PER_PAGE: u32 = 20;
const LOOKUP_PER_PAGE: u32 = 100;
/// Batas halaman saat menarik seluruh lookup, supaya tidak berputar tanpa akhir.
const MAX_HALAMAN: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Pekerjaan,
    Kontrak,
    DocumentRegister,
}

impl Module {
    pub const ALL: [Module; 3] = [Module::Pekerjaan, Module::Kontrak, Module::DocumentRegister];

    pub fn title(self) -> &'static str {
        match self {
            Module::Pekerjaan => "Pekerjaan",
            Module::Kontrak => "Kontrak",
            Module::DocumentRegister => "Register Dokumen",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Module::Pekerjaan => "pekerjaan",
            Module::Kontrak => "kontrak",
            // Register dimuat per pekerjaan, sama dengan halaman Register di frontend.
            Module::DocumentRegister => "pekerjaan/document-register",
        }
    }

    /// Nama parameter filter tahun anggaran di API. Pekerjaan memfilter lewat kegiatan induknya.
    pub fn year_param(self) -> &'static str {
        match self {
            Module::Pekerjaan | Module::Kontrak | Module::DocumentRegister => "tahun",
        }
    }

    /// Kunci yang dipakai untuk judul baris, dicoba berurutan. Field lain tetap tampil di detail.
    pub fn label_keys(self) -> &'static [&'static str] {
        match self {
            Module::Pekerjaan => &["nama_paket", "kode_rekening"],
            Module::Kontrak => &["nomor", "spk", "kode_paket"],
            Module::DocumentRegister => &["nama_paket", "kode_rekening"],
        }
    }
}

/// Error dari API atau jaringan. `errors` berisi pesan validasi per field (status 422).
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
    pub errors: Vec<(String, String)>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((field, msg)) = self.errors.first() {
            write!(f, "{field}: {msg}")
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl ApiError {
    fn network(e: reqwest::Error) -> Self {
        Self {
            status: 0,
            message: format!("Gagal terhubung ke server: {e}"),
            errors: Vec::new(),
        }
    }

    fn local(message: &str) -> Self {
        Self {
            status: 401,
            message: message.to_string(),
            errors: Vec::new(),
        }
    }
}

pub struct Client {
    base: String,
    http: Http,
    token: Option<String>,
}

impl Client {
    pub fn new(base: String) -> Result<Self> {
        let http = Http::builder().user_agent("arumanis-tui/0.1").build()?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
            token: None,
        })
    }

    pub fn login(&mut self, email: &str, password: &str) -> Result<(), ApiError> {
        let resp = self
            .http
            .post(format!("{}/auth/login", self.base))
            .json(&json!({ "email": email, "password": password }))
            .send()
            .map_err(ApiError::network)?;
        let status = resp.status().as_u16();
        let token = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| {
                c.strip_prefix(&format!("{TOKEN_COOKIE}="))
                    .map(cookie_value)
            });
        check(resp)?;
        match token {
            Some(t) if !t.is_empty() => {
                self.token = Some(t);
                Ok(())
            }
            _ => Err(ApiError {
                status,
                message: "Server tidak mengirim token login".to_string(),
                errors: Vec::new(),
            }),
        }
    }

    /// Mencabut token di server, lalu membuangnya dari memori meskipun request gagal.
    pub fn logout(&mut self) -> Result<(), ApiError> {
        let token = self
            .token
            .take()
            .ok_or_else(|| ApiError::local("Belum login"))?;
        let resp = self
            .http
            .post(format!("{}/auth/logout", self.base))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .map_err(ApiError::network)?;
        check(resp).map(|_| ())
    }

    pub fn list(
        &self,
        m: Module,
        page: u32,
        search: &str,
        year: i32,
    ) -> Result<Vec<Value>, ApiError> {
        let mut req = self
            .auth(self.http.get(format!("{}/{}", self.base, m.path())))?
            .query(&[
                ("page", page.to_string()),
                ("per_page", PER_PAGE.to_string()),
            ])
            .query(&[(m.year_param(), year.to_string())]);
        if !search.trim().is_empty() {
            req = req.query(&[("search", search.trim())]);
        }
        let body = check(req.send().map_err(ApiError::network)?)?;
        halaman(&body).map(|(rows, _)| rows)
    }

    pub fn detail(&self, m: Module, id: u64) -> Result<Value, ApiError> {
        let req = self.auth(self.http.get(format!("{}/{}/{id}", self.base, m.path())))?;
        let body = check(req.send().map_err(ApiError::network)?)?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    /// Seluruh isi lookup (kegiatan, kecamatan, desa, pengawas, jenis dokumen). Mengikuti halaman bila respons paginasi.
    pub fn lookup(&self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>, ApiError> {
        let mut rows = Vec::new();
        let mut page = 1u32;
        loop {
            let req = self
                .auth(self.http.get(self.url(path)))?
                .query(query)
                .query(&[
                    ("page", page.to_string()),
                    ("per_page", LOOKUP_PER_PAGE.to_string()),
                ]);
            let body = check(req.send().map_err(ApiError::network)?)?;
            let (data, last_page) = halaman(&body)?;
            rows.extend(data);
            match last_page {
                Some(last) if page < last && page < MAX_HALAMAN => page += 1,
                _ => return Ok(rows),
            }
        }
    }

    /// GET satu objek JSON (bukan daftar), mis. `berita-acara/sequence`.
    pub fn get_value(&self, path: &str, query: &[(&str, String)]) -> Result<Value, ApiError> {
        let req = self.auth(self.http.get(self.url(path)))?.query(query);
        check(req.send().map_err(ApiError::network)?)
    }

    /// Kirim body JSON dengan POST atau PUT. Mengembalikan isi `data` bila ada.
    pub fn kirim(&self, metode: &str, path: &str, body: &Value) -> Result<Value, ApiError> {
        let method = if metode == "PUT" {
            Method::PUT
        } else {
            Method::POST
        };
        let req = self
            .auth(self.http.request(method, self.url(path)))?
            .json(body);
        let resp = check(req.send().map_err(ApiError::network)?)?;
        Ok(resp.get("data").cloned().unwrap_or(resp))
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    fn auth(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::RequestBuilder, ApiError> {
        match &self.token {
            Some(t) => Ok(req.header(AUTHORIZATION, format!("Bearer {t}"))),
            None => Err(ApiError::local("Belum login")),
        }
    }
}

/// Baris `data` dari respons daftar, dan `last_page` bila respons paginasi. Array polos juga diterima.
fn halaman(body: &Value) -> Result<(Vec<Value>, Option<u32>), ApiError> {
    if let Some(rows) = body.as_array() {
        return Ok((rows.clone(), None));
    }
    let rows = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError {
            status: 200,
            message: "Format respons daftar tidak dikenali".to_string(),
            errors: Vec::new(),
        })?;
    let last = body
        .pointer("/meta/last_page")
        .or_else(|| body.get("last_page"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    Ok((rows.clone(), last))
}

/// Nilai cookie sampai `;`, lalu di-decode dari persen-encoding (`|` sering datang sebagai `%7C`).
fn cookie_value(raw: &str) -> String {
    let value = raw.split(';').next().unwrap_or("");
    percent_decode(value)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Memetakan respons ke JSON, atau ke `ApiError` bila status tidak sukses.
fn check(resp: Response) -> Result<Value, ApiError> {
    let status = resp.status().as_u16();
    if resp.status().is_success() {
        if status == 204 {
            return Ok(Value::Null);
        }
        return resp.json::<Value>().map_err(|e| ApiError {
            status,
            message: format!("Respons tidak valid: {e}"),
            errors: Vec::new(),
        });
    }
    let body: Value = resp.json().unwrap_or(Value::Null);
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Permintaan gagal")
        .to_string();
    let errors = body
        .get("errors")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .flat_map(|(field, msgs)| {
                    msgs.as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(move |m| (field.clone(), m.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Err(ApiError {
        status,
        message,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_value_membuang_atribut_dan_men_decode() {
        assert_eq!(cookie_value("1%7Cabc123; Path=/; HttpOnly"), "1|abc123");
        assert_eq!(cookie_value("plain"), "plain");
    }

    #[test]
    fn halaman_membaca_array_polos_dan_paginasi() {
        let (rows, last) = halaman(&json!([{ "id": 1 }])).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(last, None);
        let (_, last) = halaman(&json!({ "data": [], "meta": { "last_page": 3 } })).unwrap();
        assert_eq!(last, Some(3));
        assert!(halaman(&json!({ "pesan": "x" })).is_err());
    }

    #[test]
    fn percent_decode_menangani_karakter_di_akhir() {
        assert_eq!(percent_decode("a%7C"), "a|");
        assert_eq!(percent_decode("100%"), "100%");
    }

    /// Jalankan manual terhadap server: APIAMIS_TEST_URL, APIAMIS_TEST_EMAIL, APIAMIS_TEST_PASSWORD.
    #[test]
    #[ignore = "butuh server apiamis dan akun uji"]
    fn alur_login_daftar_detail_logout() {
        let base = std::env::var("APIAMIS_TEST_URL").expect("APIAMIS_TEST_URL");
        let email = std::env::var("APIAMIS_TEST_EMAIL").expect("APIAMIS_TEST_EMAIL");
        let password = std::env::var("APIAMIS_TEST_PASSWORD").expect("APIAMIS_TEST_PASSWORD");
        let mut c = Client::new(base).unwrap();

        assert!(
            c.list(Module::Pekerjaan, 1, "", 2026).is_err(),
            "sebelum login harus ditolak"
        );
        c.login(&email, &password).expect("login");

        for m in Module::ALL {
            let rows = c
                .list(m, 1, "", 2026)
                .unwrap_or_else(|e| panic!("list {}: {e}", m.title()));
            println!("{}: {} baris", m.title(), rows.len());
            if let Some(id) = rows
                .first()
                .and_then(|r| r.get("id"))
                .and_then(Value::as_u64)
            {
                let d = c
                    .detail(m, id)
                    .unwrap_or_else(|e| panic!("detail {}: {e}", m.title()));
                assert!(d.is_object(), "detail {} harus objek", m.title());
            }
        }

        c.logout().expect("logout");
        assert!(
            c.list(Module::Pekerjaan, 1, "", 2026).is_err(),
            "setelah logout harus ditolak"
        );
    }
}
