//! State aplikasi dan penanganan tombol. I/O hanya lewat `Client`, cache baca, dan antrean tulis.

use chrono::Datelike;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::api::{Client, Module};
use crate::cache;
use crate::form::{self, Form, Kind};
use crate::queue::{self, Pending};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Menu,
    List,
    Detail,
    Form,
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
    pub form: Option<Form>,
    /// Layar tempat formulir dibuka, untuk kembali setelah simpan atau batal.
    pub form_back: Screen,
    /// Perubahan yang menunggu dikirim: offline, atau gagal saat sinkron.
    pub antrean: Vec<Pending>,
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
            form: None,
            form_back: Screen::List,
            antrean: queue::load(),
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
            Screen::Form => self.key_form(key),
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
                if !self.antrean.is_empty() {
                    self.sinkron();
                }
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
        match self
            .client
            .list(self.module, self.page, &self.search, self.year)
        {
            Ok(rows) => {
                cache::save(self.module, self.year, self.page, &self.search, &rows);
                self.cached_at = None;
                self.status = format!("{} baris di halaman {}.", rows.len(), self.page);
                self.rows = rows;
            }
            // Status 0 = tidak ada koneksi. Tampilkan cache terakhir bila ada.
            Err(e) if e.status == 0 => {
                match cache::load(self.module, self.year, self.page, &self.search) {
                    Some((rows, saved_at)) => {
                        self.status = format!(
                            "Offline. Data dari cache tersimpan {}.",
                            cache::format_time(saved_at)
                        );
                        self.cached_at = Some(saved_at);
                        self.rows = rows;
                    }
                    None => {
                        self.rows.clear();
                        self.cached_at = None;
                        self.status = "Offline dan belum ada cache untuk daftar ini.".to_string();
                    }
                }
            }
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
            KeyCode::Char('n') if self.module == Module::Pekerjaan => {
                self.open_form_pekerjaan(None)
            }
            KeyCode::Char('s') => self.sinkron(),
            KeyCode::Char('x') => self.buang_ditolak(),
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
        // Register: server belum punya GET /document-registers/{id}. Baris daftar sudah memuat
        // kontrak, pekerjaan, dan jenis dokumen, jadi itu yang dipakai sebagai detail.
        if self.module == Module::DocumentRegister {
            self.detail = Some(row.clone());
            self.screen = Screen::Detail;
            return;
        }
        match self.client.detail(self.module, id) {
            Ok(v) => {
                self.detail = Some(v);
                self.screen = Screen::Detail;
            }
            // Offline: detail diambil dari baris daftar yang sudah ada.
            Err(e) if e.status == 0 => {
                self.detail = Some(row.clone());
                self.screen = Screen::Detail;
                self.status = "Offline: menampilkan data dari daftar.".to_string();
            }
            Err(e) => self.status = format!("Gagal membuka detail: {e}"),
        }
    }

    fn key_detail(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.detail = None;
                self.screen = Screen::List;
            }
            KeyCode::Char('e') if self.module != Module::Kontrak => self.open_form_ubah(),
            KeyCode::Char('n') if self.module == Module::Kontrak => self.open_form_register_baru(),
            _ => {}
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

    // ----- Formulir -----

    fn open_form_pekerjaan(&mut self, id: Option<u64>) {
        let dasar = if id.is_some() {
            self.detail.clone()
        } else {
            None
        };
        let tahun = self.year;
        let kegiatan = self.lookup(
            &format!("kegiatan-{tahun}"),
            "kegiatan",
            &[("tahun", tahun.to_string())],
        );
        let kecamatan = self.lookup("kecamatan", "kecamatan", &[]);
        let kec = form::id_teks(dasar.as_ref(), "kecamatan_id", "kecamatan");
        let desa = self.muat_desa(&kec);
        let pengawas = self.lookup("pengawas", "pengawas", &[]);
        let f = Form::pekerjaan(
            id,
            dasar.as_ref(),
            form::pilihan(&kegiatan, &["nama_kegiatan", "nama_sub_kegiatan"]),
            form::pilihan(&kecamatan, &["nama_kecamatan"]),
            form::pilihan(&desa, &["nama_desa"]),
            form::pilihan(&pengawas, &["nama"]),
        );
        self.buka_form(f);
    }

    fn open_form_ubah(&mut self) {
        let Some(detail) = self.detail.clone() else {
            return;
        };
        let Some(id) = detail.get("id").and_then(Value::as_u64) else {
            self.status = "Data ini tidak punya id.".to_string();
            return;
        };
        match self.module {
            Module::Pekerjaan => self.open_form_pekerjaan(Some(id)),
            Module::DocumentRegister => {
                let kontrak_id = detail
                    .get("kontrak_id")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let jenis = self.lookup("jenis-dokumen", "document-types", &[]);
                let f = Form::register(
                    Some(id),
                    kontrak_id,
                    Some(&detail),
                    form::pilihan(&jenis, &["name", "code"]),
                    &hari_ini(),
                );
                self.buka_form(f);
            }
            Module::Kontrak => {}
        }
    }

    /// Register dibuat di dalam kontrak yang sedang dibuka (keputusan 2A).
    fn open_form_register_baru(&mut self) {
        let Some(kontrak_id) = self
            .detail
            .as_ref()
            .and_then(|d| d.get("id"))
            .and_then(Value::as_u64)
        else {
            self.status = "Kontrak ini tidak punya id.".to_string();
            return;
        };
        let jenis = self.lookup("jenis-dokumen", "document-types", &[]);
        let f = Form::register(
            None,
            kontrak_id,
            None,
            form::pilihan(&jenis, &["name", "code"]),
            &hari_ini(),
        );
        self.buka_form(f);
    }

    fn buka_form(&mut self, f: Form) {
        self.form_back = self.screen;
        self.form = Some(f);
        self.screen = Screen::Form;
        self.status = "Isi formulir. Ctrl+S simpan, Esc batal.".to_string();
    }

    /// Lookup dari server. Bila gagal, pakai cache terakhir dan beri tahu di status bar.
    fn lookup(&mut self, nama: &str, path: &str, query: &[(&str, String)]) -> Vec<Value> {
        match self.client.lookup(path, query) {
            Ok(rows) => {
                cache::save_named(nama, &rows);
                rows
            }
            Err(e) => match cache::load_named(nama) {
                Some((rows, saved_at)) => {
                    self.status = format!(
                        "Lookup {nama} dari cache {}: {e}",
                        cache::format_time(saved_at)
                    );
                    rows
                }
                None => {
                    self.status = format!("Gagal memuat {nama}: {e}");
                    Vec::new()
                }
            },
        }
    }

    fn muat_desa(&mut self, kecamatan_id: &str) -> Vec<Value> {
        if kecamatan_id.is_empty() {
            return Vec::new();
        }
        let path = format!("desa/kecamatan/{kecamatan_id}");
        self.lookup(&format!("desa-{kecamatan_id}"), &path, &[])
    }

    fn key_form(&mut self, key: KeyEvent) {
        if self.form.as_ref().is_some_and(|f| f.pemilih.is_some()) {
            self.key_pemilih(key.code);
            return;
        }
        if key.code == KeyCode::Esc {
            self.batal_form();
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('s') {
            self.simpan_form();
            return;
        }
        let Some(form) = self.form.as_mut() else {
            self.screen = self.form_back;
            return;
        };
        match key.code {
            KeyCode::Tab | KeyCode::Down => form.geser_fokus(1),
            KeyCode::BackTab | KeyCode::Up => form.geser_fokus(-1),
            KeyCode::Enter => match form.fokus_field().kind {
                Kind::Pilih => form.buka_pemilih(),
                Kind::Centang => form.centang(),
                _ => form.geser_fokus(1),
            },
            KeyCode::Char(' ') if form.fokus_field().kind == Kind::Centang => form.centang(),
            KeyCode::Backspace => form.hapus(),
            KeyCode::Char(c) if !ctrl => form.ketik(c),
            _ => {}
        }
    }

    fn key_pemilih(&mut self, code: KeyCode) {
        let mut ganti_kecamatan = false;
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let Some(idx) = form.pemilih else {
            return;
        };
        let jumlah = form.fokus_field().pilihan.len();
        match code {
            KeyCode::Esc => form.pemilih = None,
            KeyCode::Up => form.pemilih = Some(idx.saturating_sub(1)),
            KeyCode::Down => form.pemilih = Some((idx + 1).min(jumlah.saturating_sub(1))),
            KeyCode::Enter => {
                if let Some((nilai, _)) = form.fokus_field().pilihan.get(idx).cloned() {
                    ganti_kecamatan = form.fokus_field().key == "kecamatan_id";
                    form.pilih_nilai(nilai);
                }
            }
            _ => {}
        }
        if ganti_kecamatan {
            self.ganti_kecamatan();
        }
    }

    /// Dropdown desa mengikuti kecamatan yang dipilih.
    fn ganti_kecamatan(&mut self) {
        let kec = self
            .form
            .as_ref()
            .map(|f| f.nilai("kecamatan_id").to_string())
            .unwrap_or_default();
        let desa = self.muat_desa(&kec);
        if let Some(form) = self.form.as_mut() {
            form.set_pilihan("desa_id", form::pilihan(&desa, &["nama_desa"]));
        }
    }

    fn simpan_form(&mut self) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        if !form.validasi() {
            self.status = "Periksa isian yang bertanda merah.".to_string();
            return;
        }
        let (metode, path) = form.target.tujuan();
        let body = form.ke_body();
        let label = form.target.label(&body);
        let modul = form.target.modul();

        match self.client.kirim(metode, &path, &body) {
            Ok(_) => {
                self.form = None;
                self.status = "Tersimpan.".to_string();
                self.setelah_simpan();
            }
            // Status 0 = tidak ada koneksi: simpan di antrean dan kirim saat online.
            Err(e) if e.status == 0 => {
                self.form = None;
                let item = Pending {
                    id: self.baru_id(),
                    modul: modul.to_string(),
                    metode: metode.to_string(),
                    path,
                    body,
                    label,
                    dibuat: chrono::Utc::now().timestamp(),
                    galat: None,
                };
                self.antrean.push(item);
                self.status = match queue::save(&self.antrean) {
                    Ok(()) => "Tidak ada koneksi. Disimpan di antrean; tekan s saat online untuk mengirim.".to_string(),
                    Err(err) => format!("Tidak ada koneksi dan antrean gagal ditulis: {err}"),
                };
                self.setelah_simpan();
            }
            Err(e) if e.status == 401 => {
                self.form = None;
                self.screen = Screen::Login;
                self.status = "Sesi habis. Masuk lagi.".to_string();
            }
            Err(e) => {
                let sisa = self
                    .form
                    .as_mut()
                    .map(|f| f.terapkan_galat(&e.errors))
                    .unwrap_or_default();
                self.status = format!(
                    "Gagal menyimpan: {}",
                    if sisa.is_empty() { e.to_string() } else { sisa }
                );
            }
        }
    }

    fn batal_form(&mut self) {
        self.form = None;
        self.screen = self.form_back;
        self.status = "Formulir ditutup tanpa menyimpan.".to_string();
    }

    /// Setelah simpan: muat ulang daftar, lalu kembali ke detail (bila dari detail) atau daftar.
    fn setelah_simpan(&mut self) {
        let pesan = self.status.clone();
        let kembali_ke_detail = self.form_back == Screen::Detail;
        self.load();
        self.status = pesan;
        if kembali_ke_detail {
            if let Some(id) = self
                .detail
                .as_ref()
                .and_then(|d| d.get("id"))
                .and_then(Value::as_u64)
            {
                if let Ok(v) = self.client.detail(self.module, id) {
                    self.detail = Some(v);
                }
            }
            self.screen = Screen::Detail;
        } else {
            self.screen = Screen::List;
        }
    }

    // ----- Antrean offline -----

    /// Kirim seluruh antrean sesuai urutan. Berhenti saat tidak ada koneksi atau sesi habis.
    /// Penolakan server ditandai di antrean dan bisa dibuang dengan x.
    pub fn sinkron(&mut self) {
        if self.antrean.is_empty() {
            self.status = "Tidak ada perubahan yang menunggu.".to_string();
            return;
        }
        let semua = std::mem::take(&mut self.antrean);
        let total = semua.len();
        let mut sisa: Vec<Pending> = Vec::new();
        let mut alasan: Option<&str> = None;
        for mut p in semua {
            if alasan.is_some() {
                sisa.push(p);
                continue;
            }
            match self.client.kirim(&p.metode, &p.path, &p.body) {
                Ok(_) => {}
                Err(e) if e.status == 0 => {
                    alasan = Some("tidak ada koneksi");
                    sisa.push(p);
                }
                Err(e) if e.status == 401 => {
                    alasan = Some("sesi habis, masuk lagi lalu tekan s");
                    sisa.push(p);
                }
                Err(e) => {
                    p.galat = Some(e.to_string());
                    sisa.push(p);
                }
            }
        }
        let terkirim = total - sisa.len();
        let tersisa = sisa.len();
        self.antrean = sisa;
        let tulis = queue::save(&self.antrean);
        self.status = match (tulis, alasan) {
            (Err(err), _) => format!("Gagal menulis antrean: {err}"),
            (Ok(()), Some(a)) => format!("{terkirim} terkirim, {tersisa} masih di antrean ({a})."),
            (Ok(()), None) => format!("{terkirim} terkirim, {tersisa} ditolak atau tertunda."),
        };
    }

    /// Membuang perubahan yang ditolak server di modul aktif.
    fn buang_ditolak(&mut self) {
        let modul = form::modul_antrean(self.module);
        let sebelum = self.antrean.len();
        self.antrean
            .retain(|p| !(p.modul == modul && p.galat.is_some()));
        let dibuang = sebelum - self.antrean.len();
        if dibuang == 0 {
            self.status = "Tidak ada perubahan yang ditolak server di modul ini.".to_string();
            return;
        }
        self.status = match queue::save(&self.antrean) {
            Ok(()) => format!("{dibuang} perubahan yang ditolak server dibuang."),
            Err(err) => format!("Gagal menulis antrean: {err}"),
        };
    }

    /// Perubahan antrean untuk modul yang sedang tampil.
    pub fn antrean_modul(&self) -> Vec<&Pending> {
        let kunci = form::modul_antrean(self.module);
        self.antrean.iter().filter(|p| p.modul == kunci).collect()
    }

    fn baru_id(&self) -> u64 {
        chrono::Utc::now().timestamp_millis().max(0) as u64 + self.antrean.len() as u64
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

fn hari_ini() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Nama pekerjaan induk sebuah register: `kontrak.pekerjaan.nama_paket`.
pub fn nama_pekerjaan_register(value: &Value) -> Option<String> {
    value
        .pointer("/kontrak/pekerjaan/nama_paket")
        .and_then(scalar_text)
        .filter(|s| !s.is_empty())
}

/// Baris detail datar: `(label, nilai)`. Objek bersarang diberi awalan `induk.`, sampai dua tingkat.
/// Daftar ditampilkan sebagai jumlah lalu satu baris per item (maks 10), dengan teks dari kunci judul.
pub fn detail_rows(value: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    push_rows(value, "", 0, &mut out);
    out
}

fn push_rows(value: &Value, prefix: &str, depth: usize, out: &mut Vec<(String, String)>) {
    let Some(obj) = value.as_object() else {
        out.push((
            prefix.trim_end_matches('.').to_string(),
            scalar_text(value).unwrap_or_default(),
        ));
        return;
    };
    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();
    for key in keys {
        let v = &obj[key];
        let label = format!("{prefix}{key}");
        match v {
            Value::Object(_) if depth < 2 => push_rows(v, &format!("{label}."), depth + 1, out),
            Value::Array(items) => {
                out.push((label.clone(), format!("{} item", items.len())));
                for item in items.iter().take(10) {
                    out.push((format!("  {label}"), item_title(item)));
                }
                if items.len() > 10 {
                    out.push((
                        format!("  {label}"),
                        format!("…dan {} lagi", items.len() - 10),
                    ));
                }
            }
            Value::Object(_) => out.push((label, "(objek)".to_string())),
            _ => out.push((label, scalar_text(v).unwrap_or_else(|| "-".to_string()))),
        }
    }
}

/// Judul satu item dalam daftar relasi: kunci nama yang umum, lalu id.
fn item_title(item: &Value) -> String {
    const KEYS: [&str; 6] = ["nama_paket", "nomor", "name", "spk", "nama", "code"];
    for k in KEYS {
        if let Some(t) = item.get(k).and_then(scalar_text).filter(|t| !t.is_empty()) {
            return t;
        }
    }
    match item.get("id").and_then(Value::as_u64) {
        Some(id) => format!("#{id}"),
        None => scalar_text(item).unwrap_or_else(|| item.to_string()),
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
    fn detail_rows_meratakan_relasi_dan_daftar() {
        let v = json!({
            "id": 3,
            "nomor": "K-1",
            "penyedia": { "nama": "CV Air" },
            "pekerjaan": [{ "id": 9, "nama_paket": "Jaringan A" }, { "id": 10 }]
        });
        let rows = detail_rows(&v);
        let get = |k: &str| rows.iter().find(|(l, _)| l == k).map(|(_, v)| v.clone());
        assert_eq!(get("nomor").as_deref(), Some("K-1"));
        assert_eq!(get("penyedia.nama").as_deref(), Some("CV Air"));
        assert_eq!(get("pekerjaan").as_deref(), Some("2 item"));
        assert!(rows
            .iter()
            .any(|(l, v)| l == "  pekerjaan" && v == "Jaringan A"));
        assert!(rows.iter().any(|(l, v)| l == "  pekerjaan" && v == "#10"));
    }

    #[test]
    fn nama_pekerjaan_register_diambil_dari_kontrak() {
        let reg = json!({ "id": 1, "kontrak": { "id": 3, "pekerjaan": { "id": 9, "nama_paket": "Jaringan A" } } });
        assert_eq!(nama_pekerjaan_register(&reg).as_deref(), Some("Jaringan A"));
        assert_eq!(nama_pekerjaan_register(&json!({ "id": 1 })), None);
    }

    #[test]
    fn scalar_text_membuang_objek_dan_array() {
        assert_eq!(scalar_text(&json!("x")), Some("x".to_string()));
        assert_eq!(scalar_text(&json!({ "a": 1 })), None);
        assert_eq!(scalar_text(&json!([1])), None);
    }
}
