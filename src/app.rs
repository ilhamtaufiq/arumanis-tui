//! State aplikasi dan penanganan tombol. Tidak ada I/O di sini kecuali lewat `Client`.

use chrono::Datelike;
use crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value;

use crate::api::{Client, Module};
use crate::cache;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Menu,
    List,
    Detail,
}

pub struct App {
    pub client: Client,
    pub screen: Screen,
    pub email: String,
    pub password: String,
    /// 0 = email, 1 = password.
    pub login_field: usize,
    /// Indeks di menu: 0..3 modul, lalu 3 = keluar.
    pub menu_idx: usize,
    pub module: Module,
    pub page: u32,
    pub search: String,
    pub searching: bool,
    /// Tahun anggaran aktif. Default tahun berjalan.
    pub year: i32,
    /// Indeks dropdown tahun yang sedang terbuka.
    pub year_menu: Option<usize>,
    /// Data yang tampil berasal dari cache, dengan waktu simpan (Unix detik).
    pub cached_at: Option<i64>,
    pub rows: Vec<Value>,
    pub selected: usize,
    pub detail: Option<Value>,
    pub status: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            screen: Screen::Login,
            email: String::new(),
            password: String::new(),
            login_field: 0,
            menu_idx: 0,
            module: Module::Pekerjaan,
            page: 1,
            search: String::new(),
            searching: false,
            year: chrono::Local::now().year(),
            year_menu: None,
            cached_at: None,
            rows: Vec::new(),
            selected: 0,
            detail: None,
            status: "Masuk dengan akun Arumanis.".to_string(),
            should_quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::Login => self.key_login(key.code),
            Screen::Menu => self.key_menu(key.code),
            Screen::List => self.key_list(key.code),
            Screen::Detail => self.key_detail(key.code),
        }
    }

    fn key_login(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                self.login_field = 1 - self.login_field;
            }
            KeyCode::Enter => self.submit_login(),
            KeyCode::Backspace => {
                self.active_login_field().pop();
            }
            KeyCode::Char(c) => self.active_login_field().push(c),
            _ => {}
        }
    }

    fn active_login_field(&mut self) -> &mut String {
        if self.login_field == 0 {
            &mut self.email
        } else {
            &mut self.password
        }
    }

    fn submit_login(&mut self) {
        if self.email.trim().is_empty() || self.password.is_empty() {
            self.status = "Email dan password wajib diisi.".to_string();
            return;
        }
        let email = self.email.trim().to_string();
        let password = self.password.clone();
        match self.client.login(&email, &password) {
            Ok(()) => {
                self.password.clear();
                self.screen = Screen::Menu;
                self.menu_idx = 0;
                self.status = format!("Masuk sebagai {email}.");
            }
            Err(e) => self.status = format!("Login gagal: {e}"),
        }
    }

    fn key_menu(&mut self, code: KeyCode) {
        let count = Module::ALL.len() + 1;
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Up => self.menu_idx = (self.menu_idx + count - 1) % count,
            KeyCode::Down => self.menu_idx = (self.menu_idx + 1) % count,
            KeyCode::Enter => {
                if self.menu_idx < Module::ALL.len() {
                    self.open_module(Module::ALL[self.menu_idx]);
                } else {
                    self.logout();
                }
            }
            KeyCode::Char('l') => self.logout(),
            _ => {}
        }
    }

    fn open_module(&mut self, m: Module) {
        self.module = m;
        self.page = 1;
        self.search.clear();
        self.selected = 0;
        self.screen = Screen::List;
        self.load();
    }

    /// Muat halaman daftar aktif. Error ditampilkan di status bar, layar tetap.
    pub fn load(&mut self) {
        match self.client.list(self.module, self.page, &self.search, self.year) {
            Ok(rows) => {
                cache::save(self.module, self.year, self.page, &self.search, &rows);
                self.cached_at = None;
                self.status = format!("{} baris di halaman {}.", rows.len(), self.page);
                self.rows = rows;
            }
            // Status 0 = tidak ada koneksi. Tampilkan cache terakhir bila ada.
            Err(e) if e.status == 0 => match cache::load(self.module, self.year, self.page, &self.search) {
                Some((rows, saved_at)) => {
                    self.status = format!("Offline. Data dari cache tersimpan {}.", cache::format_time(saved_at));
                    self.cached_at = Some(saved_at);
                    self.rows = rows;
                }
                None => {
                    self.rows.clear();
                    self.cached_at = None;
                    self.status = "Offline dan belum ada cache untuk daftar ini.".to_string();
                }
            },
            Err(e) => {
                self.status = format!("Gagal memuat daftar: {e}");
                if e.status == 401 {
                    self.rows.clear();
                    self.screen = Screen::Login;
                }
            }
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }

    fn key_year_menu(&mut self, code: KeyCode) {
        let options = cache::year_options(chrono::Local::now().year());
        let Some(idx) = self.year_menu else {
            return;
        };
        match code {
            KeyCode::Esc => self.year_menu = None,
            KeyCode::Up => self.year_menu = Some(idx.saturating_sub(1)),
            KeyCode::Down => self.year_menu = Some((idx + 1).min(options.len() - 1)),
            KeyCode::Enter => {
                self.year = options[idx];
                self.year_menu = None;
                self.page = 1;
                self.selected = 0;
                self.load();
            }
            _ => {}
        }
    }

    fn key_list(&mut self, code: KeyCode) {
        if self.year_menu.is_some() {
            self.key_year_menu(code);
            return;
        }
        if self.searching {
            match code {
                KeyCode::Esc => self.searching = false,
                KeyCode::Enter => {
                    self.searching = false;
                    self.page = 1;
                    self.selected = 0;
                    self.load();
                }
                KeyCode::Backspace => {
                    self.search.pop();
                }
                KeyCode::Char(c) => self.search.push(c),
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Menu,
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Right | KeyCode::Char(']') => {
                if !self.rows.is_empty() {
                    self.page += 1;
                    self.selected = 0;
                    self.load();
                }
            }
            KeyCode::Left | KeyCode::Char('[') => {
                if self.page > 1 {
                    self.page -= 1;
                    self.selected = 0;
                    self.load();
                }
            }
            KeyCode::Char('/') => {
                self.searching = true;
            }
            KeyCode::Char('r') => self.load(),
            KeyCode::Char('t') => {
                let options = cache::year_options(chrono::Local::now().year());
                self.year_menu = options.iter().position(|y| *y == self.year).or(Some(0));
            }
            KeyCode::Enter => self.open_detail(),
            _ => {}
        }
    }

    fn open_detail(&mut self) {
        let Some(row) = self.rows.get(self.selected) else {
            return;
        };
        let Some(id) = row.get("id").and_then(Value::as_u64) else {
            self.status = "Baris ini tidak punya id.".to_string();
            return;
        };
        match self.client.detail(self.module, id) {
            Ok(v) => {
                self.detail = Some(v);
                self.screen = Screen::Detail;
            }
            Err(e) => self.status = format!("Gagal membuka detail: {e}"),
        }
    }

    fn key_detail(&mut self, code: KeyCode) {
        if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace) {
            self.detail = None;
            self.screen = Screen::List;
        }
    }

    /// Mencabut token di server, lalu kembali ke layar login.
    fn logout(&mut self) {
        let result = self.client.logout();
        self.rows.clear();
        self.detail = None;
        self.search.clear();
        self.screen = Screen::Login;
        self.login_field = 0;
        self.status = match result {
            Ok(()) => "Anda sudah keluar.".to_string(),
            Err(e) => format!("Keluar lokal saja, server menolak: {e}"),
        };
    }

    /// Label singkat sebuah baris untuk daftar, dari kunci yang tersedia.
    pub fn row_label(&self, row: &Value) -> String {
        let parts: Vec<String> = self
            .module
            .label_keys()
            .iter()
            .filter_map(|k| row.get(*k))
            .filter_map(scalar_text)
            .filter(|s| !s.is_empty())
            .take(2)
            .collect();
        if parts.is_empty() {
            format!("#{}", row.get("id").and_then(Value::as_u64).unwrap_or(0))
        } else {
            parts.join(" · ")
        }
    }
}

/// Teks untuk nilai skalar. Objek dan array tidak ditampilkan di daftar.
pub fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "ya" } else { "tidak" }.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::DEFAULT_BASE;
    use serde_json::json;

    fn app() -> App {
        App::new(Client::new(DEFAULT_BASE.to_string()).unwrap())
    }

    #[test]
    fn label_memakai_kunci_pertama_yang_ada() {
        let mut a = app();
        a.module = Module::Kontrak;
        let row = json!({ "id": 7, "spk": "SPK-1", "kode_paket": "K-9" });
        assert_eq!(a.row_label(&row), "SPK-1 · K-9");
        let kosong = json!({ "id": 7 });
        assert_eq!(a.row_label(&kosong), "#7");
    }

    #[test]
    fn tombol_q_di_login_menutup_aplikasi() {
        let mut a = app();
        a.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(a.should_quit);
    }

    #[test]
    fn scalar_text_membuang_objek_dan_array() {
        assert_eq!(scalar_text(&json!("x")), Some("x".to_string()));
        assert_eq!(scalar_text(&json!({ "a": 1 })), None);
        assert_eq!(scalar_text(&json!([1])), None);
    }
}
