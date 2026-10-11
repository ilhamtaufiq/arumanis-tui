//! Formulir Pekerjaan dan Register Dokumen. Logika murni tanpa I/O: isian, validasi lokal, dan body untuk API.
//! Pilihan dropdown (kegiatan, kecamatan, desa, pengawas, jenis dokumen) diisi `App` dari lookup server.

use chrono::{Datelike, NaiveDate};
use serde_json::{json, Map, Value};

use crate::api::Module;
use crate::app::scalar_text;

/// Pilihan dropdown: `(nilai, label)`. Nilai kosong berarti tidak diisi.
pub type Pilihan = (String, String);

const KOSONG: &str = "— kosong —";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Teks,
    Angka,
    Tanggal,
    Pilih,
    Centang,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub wajib: bool,
    /// Isian yang tidak boleh diubah (mis. jenis dokumen saat mengubah register).
    pub terkunci: bool,
    /// Teks isian. Untuk `Pilih` berisi nilai opsi terpilih, untuk `Centang` "1" atau "0".
    pub nilai: String,
    pub pilihan: Vec<Pilihan>,
    pub galat: Option<String>,
    /// Teks bantu di bawah isian, hanya tampilan (tidak dikirim). Dipakai untuk pratinjau nomor.
    pub hint: Option<String>,
}

impl Field {
    fn new(key: &'static str, label: &'static str, kind: Kind, wajib: bool, nilai: &str) -> Self {
        Self {
            key,
            label,
            kind,
            wajib,
            terkunci: false,
            nilai: nilai.to_string(),
            pilihan: Vec::new(),
            galat: None,
            hint: None,
        }
    }

    fn pilih(key: &'static str, label: &'static str, nilai: &str, pilihan: Vec<Pilihan>) -> Self {
        let mut f = Self::new(key, label, Kind::Pilih, false, nilai);
        f.pilihan = pilihan;
        f
    }

    pub fn label_terpilih(&self) -> String {
        if self.nilai.is_empty() {
            return "—".to_string();
        }
        self.pilihan
            .iter()
            .find(|(v, _)| *v == self.nilai)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| self.nilai.clone())
    }

    /// Nilai untuk body JSON, sesuai jenis isian. Teks kosong dikirim sebagai `null`.
    fn ke_json(&self) -> Value {
        let t = self.nilai.trim();
        match self.kind {
            Kind::Centang => json!(self.nilai == "1"),
            Kind::Pilih if t.is_empty() => Value::Null,
            Kind::Pilih if self.key.ends_with("_id") => {
                t.parse::<i64>().map(Value::from).unwrap_or(Value::Null)
            }
            Kind::Pilih => json!(t),
            Kind::Angka if t.is_empty() => Value::Null,
            Kind::Angka if matches!(self.key, "sequence_number" | "last_number") => {
                t.parse::<i64>().map(Value::from).unwrap_or(Value::Null)
            }
            Kind::Angka => t.parse::<f64>().map(|n| json!(n)).unwrap_or(Value::Null),
            Kind::Tanggal => json!(t),
            Kind::Teks if t.is_empty() => Value::Null,
            Kind::Teks => json!(t),
        }
    }
}

/// Tujuan simpan: membuat atau mengubah satu data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Pekerjaan {
        id: Option<u64>,
    },
    /// Register selalu dibuat di dalam kontrak yang dipilih (`kontrak_id`).
    Register {
        id: Option<u64>,
        kontrak_id: u64,
    },
    /// Nomor urut terakhir per tahun (`/berita-acara/sequence`).
    Sequence {
        tahun: i32,
    },
}

impl Target {
    /// Kunci modul pada antrean offline.
    pub fn modul(&self) -> &'static str {
        match self {
            Target::Pekerjaan { .. } => "pekerjaan",
            Target::Register { .. } | Target::Sequence { .. } => "register",
        }
    }

    /// Metode HTTP dan path relatif terhadap base API.
    pub fn tujuan(&self) -> (&'static str, String) {
        match self {
            Target::Pekerjaan { id: None } => ("POST", "pekerjaan".to_string()),
            Target::Pekerjaan { id: Some(id) } => ("PUT", format!("pekerjaan/{id}")),
            Target::Register { id: None, .. } => ("POST", "document-registers".to_string()),
            Target::Register { id: Some(id), .. } => ("PUT", format!("document-registers/{id}")),
            Target::Sequence { .. } => ("POST", "berita-acara/sequence".to_string()),
        }
    }

    pub fn judul(&self) -> String {
        match self {
            Target::Pekerjaan { id: None } => "Pekerjaan · baru".to_string(),
            Target::Pekerjaan { id: Some(id) } => format!("Pekerjaan · ubah #{id}"),
            Target::Register {
                id: None,
                kontrak_id,
            } => format!("Register · baru di kontrak #{kontrak_id}"),
            Target::Register { id: Some(id), .. } => format!("Register · ubah #{id}"),
            Target::Sequence { tahun } => format!("Sequence · {tahun}"),
        }
    }

    /// Judul baris di daftar antrean offline.
    pub fn label(&self, body: &Value) -> String {
        match self {
            Target::Pekerjaan { id: None } => {
                let nama = body.get("nama_paket").and_then(Value::as_str).unwrap_or("");
                format!("Pekerjaan baru: {nama}")
            }
            Target::Pekerjaan { id: Some(id) } => format!("Ubah pekerjaan #{id}"),
            Target::Register {
                id: None,
                kontrak_id,
            } => format!("Register baru di kontrak #{kontrak_id}"),
            Target::Register { id: Some(id), .. } => format!("Ubah register #{id}"),
            Target::Sequence { tahun } => {
                let last = body.get("last_number").and_then(Value::as_i64).unwrap_or(0);
                format!("Sequence {tahun} → {last}")
            }
        }
    }
}

/// Jenis dokumen dengan kode dan template nomor, untuk pratinjau nomor.
#[derive(Debug, Clone)]
pub struct Jenis {
    pub id: String,
    pub code: String,
    pub template: String,
}

/// Template bawaan bila jenis dokumen belum punya `format_template`. Sama dengan frontend.
pub const TEMPLATE_DEFAULT: &str = "{sequence}/{code}-AMIS/{month}/{year}";

pub fn jenis_dari_rows(rows: &[Value]) -> Vec<Jenis> {
    rows.iter()
        .filter_map(|r| {
            let id = r.get("id").and_then(scalar_text)?;
            let code = r.get("code").and_then(scalar_text).unwrap_or_default();
            let template = r
                .get("format_template")
                .and_then(scalar_text)
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| TEMPLATE_DEFAULT.to_string());
            Some(Jenis { id, code, template })
        })
        .collect()
}

/// Nomor dari template dengan token yang sama seperti di frontend:
/// `{sequence}` (3 digit), `{nomor_urut_surat}`, `{code}`, `{year}`/`{tahun}`, `{month}` (romawi), `{day}`.
pub fn pratinjau_nomor(template: &str, code: &str, tanggal: &str, seq: i64) -> Option<String> {
    const ROMAWI: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    let tgl = NaiveDate::parse_from_str(tanggal, "%Y-%m-%d").ok()?;
    let tahun = tgl.format("%Y").to_string();
    Some(
        template
            .replace("{sequence}", &format!("{seq:03}"))
            .replace("{nomor_urut_surat}", &seq.to_string())
            .replace("{code}", code)
            .replace("{year}", &tahun)
            .replace("{tahun}", &tahun)
            .replace("{month}", ROMAWI[tgl.month0() as usize])
            .replace("{day}", &format!("{:02}", tgl.day())),
    )
}

/// Kunci modul antrean untuk daftar aktif. Kontrak tidak punya tulis langsung, jadi tidak ada antrean.
pub fn modul_antrean(m: Module) -> &'static str {
    match m {
        Module::Pekerjaan => "pekerjaan",
        Module::DocumentRegister => "register",
        Module::Kontrak => "kontrak",
    }
}

pub struct Form {
    pub target: Target,
    pub fields: Vec<Field>,
    pub fokus: usize,
    /// Indeks opsi yang disorot saat pemilih dropdown terbuka.
    pub pemilih: Option<usize>,
    /// Jenis dokumen untuk pratinjau nomor (hanya register baru).
    pub jenis: Vec<Jenis>,
    /// Nomor urut terakhir tahun berjalan. Nomor otomatis berikutnya = ini + 1.
    pub last_seq: i64,
}

impl Form {
    pub fn pekerjaan(
        id: Option<u64>,
        dasar: Option<&Value>,
        kegiatan: Vec<Pilihan>,
        kecamatan: Vec<Pilihan>,
        desa: Vec<Pilihan>,
        pengawas: Vec<Pilihan>,
    ) -> Self {
        let teks = |k: &str| {
            dasar
                .and_then(|d| d.get(k))
                .and_then(scalar_text)
                .unwrap_or_default()
        };
        let konsultan = dasar
            .and_then(|d| d.get("is_konsultan"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = if teks("status") == "canceled" {
            "canceled"
        } else {
            "active"
        };
        let fields = vec![
            Field::new(
                "nama_paket",
                "Nama paket",
                Kind::Teks,
                true,
                &teks("nama_paket"),
            ),
            Field::new(
                "kode_rekening",
                "Kode rekening",
                Kind::Teks,
                false,
                &teks("kode_rekening"),
            ),
            Field::new(
                "pagu",
                "Pagu (Rp)",
                Kind::Angka,
                true,
                &angka_teks(dasar.and_then(|d| d.get("pagu"))),
            ),
            Field::new(
                "is_konsultan",
                "Paket konsultan",
                Kind::Centang,
                false,
                if konsultan { "1" } else { "0" },
            ),
            Field::pilih(
                "kegiatan_id",
                "Kegiatan",
                &id_teks(dasar, "kegiatan_id", "kegiatan"),
                kegiatan,
            ),
            Field::pilih(
                "kecamatan_id",
                "Kecamatan",
                &id_teks(dasar, "kecamatan_id", "kecamatan"),
                kecamatan,
            ),
            Field::pilih("desa_id", "Desa", &id_teks(dasar, "desa_id", "desa"), desa),
            Field::pilih(
                "pengawas_id",
                "Pengawas",
                &id_teks(dasar, "pengawas_id", "pengawas"),
                pengawas.clone(),
            ),
            Field::pilih(
                "pendamping_id",
                "Pendamping",
                &id_teks(dasar, "pendamping_id", "pendamping"),
                pengawas,
            ),
            Field::pilih(
                "status",
                "Status",
                status,
                vec![
                    ("active".to_string(), "Aktif".to_string()),
                    ("canceled".to_string(), "Dibatalkan".to_string()),
                ],
            ),
            Field::new("catatan", "Catatan", Kind::Teks, false, &teks("catatan")),
        ];
        Self {
            target: Target::Pekerjaan { id },
            fields,
            fokus: 0,
            pemilih: None,
            jenis: Vec::new(),
            last_seq: 0,
        }
    }

    /// Register dokumen. `hari_ini` mengisi tanggal bila data dasar belum punya tanggal.
    /// `jenis_rows` dari `/document-types`; `jenis_pref` memilih jenis awal saat membuat register.
    /// `last_seq` adalah nomor urut terakhir tahun berjalan, untuk pratinjau nomor otomatis.
    pub fn register(
        id: Option<u64>,
        kontrak_id: u64,
        dasar: Option<&Value>,
        jenis_rows: &[Value],
        jenis_pref: Option<&str>,
        hari_ini: &str,
        last_seq: i64,
    ) -> Self {
        let teks = |k: &str| {
            dasar
                .and_then(|d| d.get(k))
                .and_then(scalar_text)
                .unwrap_or_default()
        };
        let tanggal = teks("tanggal")
            .get(..10)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| hari_ini.to_string());

        let jenis_awal = match id_teks(dasar, "type_id", "type") {
            t if t.is_empty() => jenis_pref.unwrap_or("").to_string(),
            t => t,
        };
        let mut type_field = Field::pilih(
            "type_id",
            "Jenis dokumen",
            &jenis_awal,
            pilihan(jenis_rows, &["name", "code"]),
        );
        type_field.wajib = true;
        type_field.terkunci = id.is_some();

        let nomor_label = if id.is_some() {
            "Nomor"
        } else {
            "Nomor (kosong = otomatis)"
        };
        let mut fields = vec![
            type_field,
            Field::new("tanggal", "Tanggal", Kind::Tanggal, true, &tanggal),
            // Saat mengubah, nomor wajib diisi (aturan server).
            Field::new(
                "nomor",
                nomor_label,
                Kind::Teks,
                id.is_some(),
                &teks("nomor"),
            ),
            Field::new(
                "description",
                "Keterangan",
                Kind::Teks,
                false,
                &teks("description"),
            ),
        ];
        if id.is_none() {
            fields.push(Field::new(
                "sequence_number",
                "No. urut (opsional)",
                Kind::Angka,
                false,
                "",
            ));
        }
        let mut form = Self {
            target: Target::Register { id, kontrak_id },
            fields,
            fokus: 0,
            pemilih: None,
            jenis: jenis_dari_rows(jenis_rows),
            last_seq,
        };
        form.perbarui_pratinjau();
        form
    }

    /// Formulir nomor urut terakhir untuk tahun tertentu.
    pub fn sequence(tahun: i32, terakhir: i64) -> Self {
        Self {
            target: Target::Sequence { tahun },
            fields: vec![Field::new(
                "last_number",
                "Nomor urut terakhir",
                Kind::Angka,
                true,
                &terakhir.to_string(),
            )],
            fokus: 0,
            pemilih: None,
            jenis: Vec::new(),
            last_seq: terakhir,
        }
    }

    /// Pratinjau nomor di bawah isian nomor, saat membuat register. Mengikuti jenis, tanggal, dan sequence.
    pub fn perbarui_pratinjau(&mut self) {
        if !matches!(self.target, Target::Register { id: None, .. }) {
            return;
        }
        let jenis_id = self.nilai("type_id").to_string();
        let tanggal = self.nilai("tanggal").to_string();
        let manual = self.nilai("sequence_number").trim().parse::<i64>().ok();
        let seq = manual.unwrap_or(self.last_seq + 1);
        let hint = self
            .jenis
            .iter()
            .find(|j| j.id == jenis_id)
            .and_then(|j| pratinjau_nomor(&j.template, &j.code, &tanggal, seq))
            .map(|n| format!("Pratinjau: {n}"));
        if let Some(f) = self.fields.iter_mut().find(|f| f.key == "nomor") {
            f.hint = hint;
        }
    }

    pub fn fokus_field(&self) -> &Field {
        &self.fields[self.fokus]
    }

    pub fn nilai(&self, key: &str) -> &str {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.nilai.as_str())
            .unwrap_or("")
    }

    pub fn geser_fokus(&mut self, arah: isize) {
        let n = self.fields.len() as isize;
        self.fokus = (self.fokus as isize + arah).rem_euclid(n) as usize;
    }

    pub fn ketik(&mut self, c: char) {
        let f = &mut self.fields[self.fokus];
        if f.terkunci {
            return;
        }
        let boleh = match f.kind {
            Kind::Teks => !c.is_control(),
            Kind::Angka => c.is_ascii_digit() || (c == '.' && !f.nilai.contains('.')),
            Kind::Tanggal => (c.is_ascii_digit() || c == '-') && f.nilai.len() < 10,
            Kind::Pilih | Kind::Centang => false,
        };
        if boleh {
            f.nilai.push(c);
            f.galat = None;
        }
        self.perbarui_pratinjau();
    }

    pub fn hapus(&mut self) {
        let f = &mut self.fields[self.fokus];
        if f.terkunci || matches!(f.kind, Kind::Pilih | Kind::Centang) {
            return;
        }
        f.nilai.pop();
        f.galat = None;
        self.perbarui_pratinjau();
    }

    pub fn centang(&mut self) {
        let f = &mut self.fields[self.fokus];
        if f.kind == Kind::Centang && !f.terkunci {
            f.nilai = if f.nilai == "1" { "0" } else { "1" }.to_string();
        }
    }

    /// Membuka pemilih dropdown pada isian aktif, dengan opsi terpilih disorot.
    pub fn buka_pemilih(&mut self) {
        let f = &self.fields[self.fokus];
        if f.kind != Kind::Pilih || f.terkunci {
            return;
        }
        let idx = f
            .pilihan
            .iter()
            .position(|(v, _)| *v == f.nilai)
            .unwrap_or(0);
        self.pemilih = Some(idx);
    }

    pub fn pilih_nilai(&mut self, nilai: String) {
        let f = &mut self.fields[self.fokus];
        f.nilai = nilai;
        f.galat = None;
        self.pemilih = None;
        self.perbarui_pratinjau();
    }

    /// Mengganti opsi dropdown. Nilai terpilih dikosongkan bila tidak ada lagi di daftar baru.
    pub fn set_pilihan(&mut self, key: &str, pilihan: Vec<Pilihan>) {
        if let Some(f) = self.fields.iter_mut().find(|f| f.key == key) {
            if !pilihan.iter().any(|(v, _)| *v == f.nilai) {
                f.nilai.clear();
            }
            f.pilihan = pilihan;
        }
    }

    /// Validasi lokal, mengikuti aturan di frontend. Mengisi `galat` per isian dan mengembalikan `true` bila lolos.
    pub fn validasi(&mut self) -> bool {
        let wajib_lokasi =
            matches!(self.target, Target::Pekerjaan { .. }) && self.nilai("is_konsultan") != "1";
        for f in &mut self.fields {
            let t = f.nilai.trim();
            f.galat = if t.is_empty() {
                if f.wajib {
                    Some("Wajib diisi.".to_string())
                } else if wajib_lokasi && matches!(f.key, "kecamatan_id" | "desa_id") {
                    Some("Wajib untuk paket non-konsultan.".to_string())
                } else {
                    None
                }
            } else {
                match f.kind {
                    Kind::Angka => match t.parse::<f64>() {
                        Ok(n) if n >= 0.0 && n.is_finite() => None,
                        _ => Some("Harus angka, minimal 0.".to_string()),
                    },
                    Kind::Tanggal => NaiveDate::parse_from_str(t, "%Y-%m-%d")
                        .err()
                        .map(|_| "Format tanggal YYYY-MM-DD.".to_string()),
                    _ => None,
                }
            };
        }
        self.fields.iter().all(|f| f.galat.is_none())
    }

    /// Body JSON untuk POST atau PUT.
    pub fn ke_body(&self) -> Value {
        let mut m = Map::new();
        for f in &self.fields {
            m.insert(f.key.to_string(), f.ke_json());
        }
        if let Target::Sequence { tahun } = &self.target {
            m.insert("year".to_string(), json!(tahun));
        }
        if let Target::Register { id, kontrak_id } = &self.target {
            match id {
                None => {
                    m.insert("kontrak_id".to_string(), json!(kontrak_id));
                }
                // Saat mengubah, jenis dokumen dan nomor urut tidak ikut dikirim.
                Some(_) => {
                    m.remove("type_id");
                    m.remove("sequence_number");
                }
            }
            // Nomor kosong saat membuat berarti otomatis. Saat mengubah, nomor tidak dikosongkan.
            if m.get("nomor").is_some_and(Value::is_null) {
                m.remove("nomor");
            }
        }
        Value::Object(m)
    }

    /// Menempelkan galat dari server ke isian yang sesuai. Galat tanpa isian dikembalikan sebagai teks.
    pub fn terapkan_galat(&mut self, errors: &[(String, String)]) -> String {
        let mut sisa = Vec::new();
        for (kunci, pesan) in errors {
            match self.fields.iter_mut().find(|f| f.key == kunci) {
                Some(f) => f.galat = Some(pesan.clone()),
                None => sisa.push(pesan.clone()),
            }
        }
        sisa.join(" ")
    }
}

/// Mengubah daftar lookup menjadi pilihan. Label dari kunci-kunci yang tersedia, digabung dengan " · ".
pub fn pilihan(rows: &[Value], kunci: &[&str]) -> Vec<Pilihan> {
    let mut out = vec![(String::new(), KOSONG.to_string())];
    for r in rows {
        let Some(id) = r.get("id").and_then(scalar_text) else {
            continue;
        };
        let bagian: Vec<String> = kunci
            .iter()
            .filter_map(|k| r.get(*k).and_then(scalar_text))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let label = if bagian.is_empty() {
            format!("#{id}")
        } else {
            bagian.join(" · ")
        };
        out.push((id, label));
    }
    out
}

/// Id relasi dari data dasar: kolom `kunci` (mis. `kecamatan_id`), atau `relasi.id` bila hanya objek relasi yang ada.
pub fn id_teks(dasar: Option<&Value>, kunci: &str, relasi: &str) -> String {
    let Some(v) = dasar else {
        return String::new();
    };
    v.get(kunci)
        .and_then(scalar_text)
        .or_else(|| {
            v.get(relasi)
                .and_then(|r| r.get("id"))
                .and_then(scalar_text)
        })
        .unwrap_or_default()
}

/// Pagu dari data dasar sebagai teks isian. Tanpa data, "0".
fn angka_teks(v: Option<&Value>) -> String {
    let n = v.and_then(|v| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()));
    match n {
        Some(n) if n.fract() == 0.0 => format!("{}", n as i64),
        Some(n) => n.to_string(),
        None => "0".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isi(f: &mut Form, key: &str, nilai: &str) {
        let field = f
            .fields
            .iter_mut()
            .find(|x| x.key == key)
            .expect("kunci ada");
        field.nilai = nilai.to_string();
    }

    fn pekerjaan_baru() -> Form {
        Form::pekerjaan(None, None, vec![], vec![], vec![], vec![])
    }

    #[test]
    fn pekerjaan_wajib_nama_dan_lokasi_kecuali_konsultan() {
        let mut f = pekerjaan_baru();
        assert!(!f.validasi());
        isi(&mut f, "nama_paket", "Jaringan A");
        assert!(
            !f.validasi(),
            "kecamatan dan desa wajib untuk non-konsultan"
        );
        isi(&mut f, "kecamatan_id", "3");
        isi(&mut f, "desa_id", "9");
        assert!(f.validasi());
        isi(&mut f, "is_konsultan", "1");
        isi(&mut f, "kecamatan_id", "");
        isi(&mut f, "desa_id", "");
        assert!(f.validasi(), "konsultan tidak butuh lokasi");
    }

    #[test]
    fn ke_body_pekerjaan_mengirim_angka_dan_null() {
        let mut f = pekerjaan_baru();
        isi(&mut f, "nama_paket", "Jaringan");
        isi(&mut f, "pagu", "1500000.5");
        isi(&mut f, "kecamatan_id", "3");
        isi(&mut f, "desa_id", "9");
        isi(&mut f, "catatan", "   ");
        let b = f.ke_body();
        assert_eq!(b["nama_paket"], "Jaringan");
        assert_eq!(b["pagu"], 1500000.5);
        assert_eq!(b["kecamatan_id"], 3);
        assert!(b["pengawas_id"].is_null());
        assert!(b["catatan"].is_null());
        assert_eq!(b["is_konsultan"], false);
        assert_eq!(b["status"], "active");
    }

    #[test]
    fn register_edit_tidak_mengirim_jenis_dan_kontrak() {
        let dasar = json!({
            "id": 5,
            "type_id": 2,
            "tanggal": "2026-03-04T00:00:00.000000Z",
            "nomor": "001/X",
            "description": null,
            "kontrak_id": 7
        });
        let f = Form::register(Some(5), 7, Some(&dasar), &[], None, "2026-01-01", 0);
        let b = f.ke_body();
        assert!(b.get("type_id").is_none());
        assert!(b.get("kontrak_id").is_none());
        assert_eq!(b["tanggal"], "2026-03-04");
        assert_eq!(b["nomor"], "001/X");
        assert!(b["description"].is_null());
        assert_eq!(
            f.target.tujuan(),
            ("PUT", "document-registers/5".to_string())
        );
    }

    #[test]
    fn register_baru_menyertakan_kontrak_dan_membuang_nomor_kosong() {
        let jenis = [json!({ "id": 2, "name": "Surat", "code": "SRT" })];
        let mut f = Form::register(None, 7, None, &jenis, None, "2026-01-01", 0);
        isi(&mut f, "type_id", "2");
        let b = f.ke_body();
        assert_eq!(b["kontrak_id"], 7);
        assert_eq!(b["type_id"], 2);
        assert!(b.get("nomor").is_none());
        assert_eq!(b["tanggal"], "2026-01-01");
        assert_eq!(
            f.target.tujuan(),
            ("POST", "document-registers".to_string())
        );
        assert_eq!(f.target.label(&b), "Register baru di kontrak #7");
    }

    #[test]
    fn validasi_tanggal_dan_galat_server_ditempel_ke_isian() {
        let mut f = Form::register(None, 7, None, &[], None, "2026-01-01", 0);
        isi(&mut f, "type_id", "2");
        isi(&mut f, "tanggal", "01-02-2026");
        assert!(!f.validasi());
        let sisa = f.terapkan_galat(&[
            ("nomor".to_string(), "Nomor sudah dipakai.".to_string()),
            ("apa".to_string(), "Lainnya".to_string()),
        ]);
        assert_eq!(sisa, "Lainnya");
        let nomor = f.fields.iter().find(|x| x.key == "nomor").unwrap();
        assert_eq!(nomor.galat.as_deref(), Some("Nomor sudah dipakai."));
    }

    #[test]
    fn ketik_menolak_karakter_sesuai_jenis_isian() {
        let mut f = Form::register(None, 7, None, &[], None, "2026-01-01", 0);
        f.fokus = 1; // tanggal
        f.ketik('a');
        assert_eq!(f.fields[1].nilai, "2026-01-01", "huruf ditolak");
        f.fokus = 0; // pilihan tidak diketik
        f.ketik('5');
        assert_eq!(f.fields[0].nilai, "");
    }

    #[test]
    fn fokus_berputar_dan_pilihan_punya_opsi_kosong() {
        let mut f = pekerjaan_baru();
        f.geser_fokus(-1);
        assert_eq!(f.fokus, f.fields.len() - 1);
        f.geser_fokus(1);
        assert_eq!(f.fokus, 0);
        let p = pilihan(
            &[json!({ "id": 4, "nama_kecamatan": "Cianjur" })],
            &["nama_kecamatan"],
        );
        assert_eq!(p[0], (String::new(), KOSONG.to_string()));
        assert_eq!(p[1], ("4".to_string(), "Cianjur".to_string()));
    }

    #[test]
    fn pratinjau_nomor_mengikuti_token_frontend() {
        let t = "{sequence}/{code}-AMIS/{month}/{year}";
        assert_eq!(
            pratinjau_nomor(t, "SRT", "2026-05-03", 7).as_deref(),
            Some("007/SRT-AMIS/V/2026")
        );
        assert_eq!(
            pratinjau_nomor("{nomor_urut_surat}-{day}-{tahun}", "X", "2026-12-09", 12).as_deref(),
            Some("12-09-2026")
        );
        assert_eq!(pratinjau_nomor(t, "SRT", "tanggal-salah", 1), None);
    }

    #[test]
    fn pratinjau_mengikuti_jenis_dan_sequence_manual() {
        let jenis = [
            json!({ "id": 2, "name": "Surat", "code": "SRT", "format_template": "{sequence}/{code}/{year}" }),
        ];
        let mut f = Form::register(None, 7, None, &jenis, Some("2"), "2026-05-03", 41);
        let hint = |f: &Form| {
            f.fields
                .iter()
                .find(|x| x.key == "nomor")
                .unwrap()
                .hint
                .clone()
        };
        assert_eq!(hint(&f).as_deref(), Some("Pratinjau: 042/SRT/2026"));
        isi(&mut f, "sequence_number", "9");
        f.perbarui_pratinjau();
        assert_eq!(hint(&f).as_deref(), Some("Pratinjau: 009/SRT/2026"));
    }

    #[test]
    fn nomor_wajib_saat_mengubah_register() {
        let dasar = json!({ "id": 5, "type_id": 2, "tanggal": "2026-03-04", "nomor": "" });
        let mut f = Form::register(Some(5), 7, Some(&dasar), &[], None, "2026-01-01", 0);
        assert!(!f.validasi());
        let nomor = f.fields.iter().find(|x| x.key == "nomor").unwrap();
        assert_eq!(nomor.galat.as_deref(), Some("Wajib diisi."));
    }

    #[test]
    fn body_sequence_mengirim_tahun_dan_nomor_bulat() {
        let mut f = Form::sequence(2026, 41);
        isi(&mut f, "last_number", "45");
        assert!(f.validasi());
        let b = f.ke_body();
        assert_eq!(b["year"], 2026);
        assert_eq!(b["last_number"], 45);
        assert_eq!(
            f.target.tujuan(),
            ("POST", "berita-acara/sequence".to_string())
        );
    }
}
