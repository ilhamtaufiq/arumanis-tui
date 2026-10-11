//! Tampilan ratatui. Hanya membaca `App`, tidak mengubah state.

use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState, Wrap,
    },
    Frame,
};

use chrono::Datelike;

use crate::api::Module;
use crate::app::{detail_rows, kontrak_utama, register_untuk_jenis, App, Screen};
use crate::form::{Form, Kind};

const ACCENT: Color = Color::Rgb(0x0B, 0x3B, 0x78);
const ACCENT_SOFT: Color = Color::Rgb(0x7F, 0xA7, 0xD9);
const OK: Color = Color::Rgb(0x2E, 0x9E, 0x5B);
const WARN: Color = Color::Rgb(0xD9, 0x8E, 0x04);
const ERR: Color = Color::Rgb(0xC6, 0x28, 0x28);
const MUTED: Color = Color::Rgb(0x6B, 0x72, 0x80);
const SELECT_BG: Color = Color::Rgb(0x1F, 0x4E, 0x8C);

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, body, help, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, app, header);

    let area = body.inner(Margin::new(1, 0));
    match app.screen {
        Screen::Login => draw_login(frame, app, area),
        Screen::Menu => draw_menu(frame, app, area),
        Screen::List => draw_list(frame, app, area),
        Screen::Detail => draw_detail(frame, app, area),
        Screen::Form => draw_form(frame, app, area),
    }

    draw_help(frame, &help_pairs(app), help);
    draw_status(frame, &app.status, status);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let crumb = match app.screen {
        Screen::Login => "Masuk".to_string(),
        Screen::Menu => "Menu".to_string(),
        Screen::List => format!("{} · {}", app.module.title(), app.year),
        Screen::Detail => format!("{} · detail", app.module.title()),
        Screen::Form => app
            .form
            .as_ref()
            .map(|f| f.target.judul())
            .unwrap_or_default(),
    };
    let line = Line::from(vec![
        Span::styled(
            " ◆ ARUMANIS ",
            Style::default()
                .fg(Color::White)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {crumb} "),
            Style::default().fg(Color::White).bg(SELECT_BG),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(ACCENT)),
        area,
    );
}

/// Pasangan (tombol, keterangan) untuk bilah bantuan di bawah layar.
fn help_pairs(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.screen {
        Screen::Login => vec![("Tab", "pindah"), ("Enter", "masuk"), ("Esc", "keluar")],
        Screen::Menu => vec![
            ("↑↓", "pilih"),
            ("Enter", "buka"),
            ("l", "keluar"),
            ("q", "tutup"),
        ],
        Screen::List if app.searching => {
            vec![("ketik", "cari"), ("Enter", "terapkan"), ("Esc", "batal")]
        }
        Screen::List if app.year_menu.is_some() => vec![
            ("↑↓", "pilih tahun"),
            ("Enter", "terapkan"),
            ("Esc", "batal"),
        ],
        Screen::List => {
            let mut v = vec![
                ("↑↓", "pilih"),
                ("Enter", "detail"),
                ("/", "cari"),
                ("t", "tahun"),
                ("←→", "halaman"),
            ];
            match app.module {
                Module::Pekerjaan => v.push(("n", "baru")),
                Module::DocumentRegister => v.push(("S", "sequence")),
                Module::Kontrak => {}
            }
            v.extend([
                ("s", "kirim antrean"),
                ("x", "buang ditolak"),
                ("Esc", "menu"),
            ]);
            v
        }
        Screen::Detail => {
            let mut v = vec![("Esc", "kembali")];
            match app.module {
                Module::Kontrak => v.push(("n", "register baru")),
                Module::DocumentRegister => {
                    v.push(("↑↓", "jenis"));
                    v.push(("Enter", "isi/ubah nomor"));
                }
                Module::Pekerjaan => v.push(("e", "ubah")),
            }
            v
        }
        Screen::Form if app.form.as_ref().is_some_and(|f| f.pemilih.is_some()) => {
            vec![("↑↓", "pilih"), ("Enter", "pakai"), ("Esc", "batal")]
        }
        Screen::Form => vec![
            ("Tab/↑↓", "pindah"),
            ("Enter", "pilih"),
            ("Spasi", "centang"),
            ("Ctrl+S", "simpan"),
            ("Esc", "batal"),
        ],
    }
}

fn draw_help(frame: &mut Frame, pairs: &[(&str, &str)], area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (tombol, ket) in pairs {
        spans.push(Span::styled(
            format!(" {tombol} "),
            Style::default()
                .fg(Color::White)
                .bg(ACCENT_SOFT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {ket}   "),
            Style::default().fg(MUTED),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Status bar dengan ikon dan warna sesuai jenis pesan.
fn draw_status(frame: &mut Frame, pesan: &str, area: Rect) {
    let (ikon, warna) =
        if pesan.contains("Gagal") || pesan.contains("gagal") || pesan.contains("Periksa") {
            ("✖", ERR)
        } else if pesan.contains("Offline")
            || pesan.contains("Tidak ada koneksi")
            || pesan.contains("cache")
            || pesan.contains("antrean")
            || pesan.contains("Tidak ada perubahan")
            || pesan.contains("Sesi habis")
        {
            ("⚠", WARN)
        } else if pesan.contains("Tersimpan")
            || pesan.contains("terkirim")
            || pesan.contains("Masuk sebagai")
            || pesan.contains("sudah keluar")
        {
            ("✔", OK)
        } else {
            ("ℹ", MUTED)
        };
    let line = Line::from(vec![
        Span::styled(
            format!(" {ikon} "),
            Style::default().fg(warna).add_modifier(Modifier::BOLD),
        ),
        Span::styled(pesan.to_string(), Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn panel(judul: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT_SOFT))
        .title(Span::styled(
            format!(" {judul} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
}

fn sorot() -> Style {
    Style::default()
        .bg(SELECT_BG)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn draw_login(frame: &mut Frame, app: &App, area: Rect) {
    let kotak = centered(area, 56, 13);
    let masked = "•".repeat(app.password.chars().count());
    let field = |label: &str, value: String, active: bool| {
        let (marker, gaya) = if active {
            (
                "▌ ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(MUTED))
        };
        let kursor = if active { "▏" } else { "" };
        Line::from(vec![
            Span::styled(marker, gaya),
            Span::styled(format!("{label:<10}"), gaya),
            Span::raw(format!("{value}{kursor}")),
        ])
    };
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Selamat datang di Arumanis",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Masuk dengan akun Anda",
            Style::default().fg(MUTED),
        )),
        Line::from(""),
        field("Email", app.email.clone(), app.login_field == 0),
        Line::from(""),
        field("Password", masked, app.login_field == 1),
    ];
    frame.render_widget(Paragraph::new(lines).block(panel("Masuk")), kotak);
}

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let kotak = centered(area, 48, 14);
    let mut items: Vec<ListItem> = Module::ALL
        .iter()
        .map(|m| ListItem::new(format!("  {}", m.title())))
        .collect();
    items.push(ListItem::new("  Keluar").style(Style::default().fg(ERR)));
    let [daftar, info] = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(kotak);
    let list = List::new(items)
        .block(panel("Menu"))
        .highlight_style(sorot())
        .highlight_symbol("▶ ");
    let mut state = ListState::default().with_selected(Some(app.menu_idx));
    frame.render_stateful_widget(list, daftar, &mut state);
    let info_line = Line::from(vec![
        Span::styled(format!(" Tahun {}", app.year), Style::default().fg(ACCENT)),
        Span::styled(
            format!("   Antrean {}", app.antrean.len()),
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(info_line), info);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let [cari_area, rows_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    let teks_cari = if app.searching {
        Line::from(vec![
            Span::raw(app.search.clone()),
            Span::styled("▏", Style::default().fg(ACCENT)),
        ])
    } else if app.search.is_empty() {
        Line::from(Span::styled(
            "tekan / untuk mencari",
            Style::default().fg(MUTED),
        ))
    } else {
        Line::from(vec![
            Span::styled("cari: ", Style::default().fg(MUTED)),
            Span::raw(app.search.clone()),
        ])
    };
    let antrean = app.antrean_modul();
    let mut judul = format!("Tahun {} · halaman {}", app.year, app.page);
    if app.cached_at.is_some() {
        judul.push_str(" · offline");
    }
    if app.module == Module::DocumentRegister {
        if let Some(seq) = app.sequence {
            judul.push_str(&format!(" · sequence {seq}"));
        }
    }
    if !antrean.is_empty() {
        judul.push_str(&format!(" · {} menunggu", antrean.len()));
    }
    frame.render_widget(Paragraph::new(teks_cari).block(panel(&judul)), cari_area);

    let (headers, widths) = kolom_modul(app.module);
    let mut rows: Vec<Row> = antrean
        .iter()
        .map(|p| {
            let (ikon, warna, status) = match &p.galat {
                Some(g) => ("✖", ERR, format!("ditolak: {g}")),
                None => ("⧗", WARN, "menunggu kirim".to_string()),
            };
            let mut cells: Vec<Cell> = vec![Cell::from(format!("{ikon} {}", p.label))];
            cells.extend(std::iter::repeat_with(|| Cell::from("")).take(headers.len() - 2));
            cells.push(Cell::from(status));
            Row::new(cells).style(Style::default().fg(warna))
        })
        .collect();
    let offset = rows.len();
    if app.rows.is_empty() {
        rows.push(Row::new(vec![Cell::from(Span::styled(
            "Tidak ada data di halaman ini.",
            Style::default().fg(MUTED),
        ))]));
    } else {
        rows.extend(app.rows.iter().map(|r| Row::new(sel_kolom(app.module, r))));
    }
    let header = Row::new(headers.iter().map(|h| Cell::from(*h)).collect::<Vec<_>>())
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));
    let table = Table::new(rows, widths)
        .header(header)
        .block(panel("Daftar"))
        .row_highlight_style(sorot())
        .highlight_symbol("▶ ")
        .column_spacing(2);
    let selected = if app.rows.is_empty() {
        None
    } else {
        Some(offset + app.selected)
    };
    let mut state = TableState::default().with_selected(selected);
    frame.render_stateful_widget(table, rows_area, &mut state);

    if let Some(idx) = app.year_menu {
        draw_year_menu(frame, rows_area, idx);
    }
}

/// Judul dan lebar kolom daftar per modul.
fn kolom_modul(m: Module) -> (Vec<&'static str>, Vec<Constraint>) {
    match m {
        Module::Pekerjaan => (
            vec!["Nama paket", "Kode rekening", "Pagu", "Status"],
            vec![
                Constraint::Min(20),
                Constraint::Length(18),
                Constraint::Length(16),
                Constraint::Length(16),
            ],
        ),
        Module::Kontrak => (
            vec!["SPK", "Kode paket", "Nilai kontrak", "Tgl SPK"],
            vec![
                Constraint::Min(20),
                Constraint::Length(18),
                Constraint::Length(18),
                Constraint::Length(12),
            ],
        ),
        Module::DocumentRegister => (
            vec!["Nama paket", "Kode rekening", "Kontrak", "Register"],
            vec![
                Constraint::Min(20),
                Constraint::Length(18),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        ),
    }
}

/// Sel untuk satu baris data, sesuai kolom modul.
fn sel_kolom(m: Module, row: &serde_json::Value) -> Vec<Cell<'static>> {
    let teks = |k: &str| {
        row.get(k)
            .and_then(crate::app::scalar_text)
            .unwrap_or_else(|| "-".to_string())
    };
    let tanggal = |k: &str| teks(k).chars().take(10).collect::<String>();
    let jumlah_kontrak = row
        .get("kontrak")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let jumlah_register: usize = row
        .get("kontrak")
        .and_then(serde_json::Value::as_array)
        .map_or(0, |ks| {
            ks.iter()
                .map(|k| {
                    k.get("registers")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len)
                })
                .sum()
        });
    match m {
        Module::Pekerjaan => vec![
            Cell::from(teks("nama_paket")),
            Cell::from(teks("kode_rekening")),
            Cell::from(rupiah(row.get("pagu"))).style(Style::default().fg(Color::White)),
            Cell::from(status_teks(&teks("status"))),
        ],
        Module::Kontrak => vec![
            Cell::from(teks("spk")),
            Cell::from(teks("kode_paket")),
            Cell::from(rupiah(row.get("nilai_kontrak"))),
            Cell::from(tanggal("tgl_spk")),
        ],
        Module::DocumentRegister => vec![
            Cell::from(teks("nama_paket")),
            Cell::from(teks("kode_rekening")),
            Cell::from(jumlah_kontrak.to_string()),
            Cell::from(Span::styled(
                format!("{jumlah_register} nomor"),
                Style::default().fg(if jumlah_register > 0 { OK } else { MUTED }),
            )),
        ],
    }
}

fn status_teks(s: &str) -> String {
    match s {
        "active" => "aktif".to_string(),
        "canceled" => "batal".to_string(),
        "-" => "-".to_string(),
        other => other.to_string(),
    }
}

/// Angka sebagai rupiah dengan pemisah ribuan titik, mis. `1.500.000`. `-` bila kosong.
fn rupiah(v: Option<&serde_json::Value>) -> String {
    let n = v.and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()));
    let Some(n) = n else {
        return "-".to_string();
    };
    let angka = format!("{:.0}", n.abs());
    let mut out = String::new();
    for (i, c) in angka.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    let out: String = out.chars().rev().collect();
    if n < 0.0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Dropdown tahun anggaran, digambar di atas daftar.
fn draw_year_menu(frame: &mut Frame, area: Rect, idx: usize) {
    let options = crate::cache::year_options(chrono::Local::now().year());
    let width = 18u16.min(area.width);
    let height = (options.len() as u16 + 2).min(area.height);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width),
        area.y,
        width,
        height,
    );
    let items: Vec<ListItem> = options
        .iter()
        .map(|y| ListItem::new(format!(" {y}")))
        .collect();
    let list = List::new(items)
        .block(panel("Tahun"))
        .highlight_style(sorot())
        .highlight_symbol("▶");
    let mut state = ListState::default().with_selected(Some(idx));
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let Some(value) = &app.detail else {
        frame.render_widget(Paragraph::new("Tidak ada detail."), area);
        return;
    };
    if app.module == Module::DocumentRegister {
        draw_detail_register(frame, app, value, area);
        return;
    }
    let lines: Vec<Line> = detail_rows(value)
        .into_iter()
        .map(|(label, text)| {
            if label.starts_with(' ') {
                Line::from(vec![
                    Span::styled(
                        format!("{:<28}", format!("   {}", label.trim())),
                        Style::default().fg(MUTED),
                    ),
                    Span::styled(text, Style::default().fg(Color::Gray)),
                ])
            } else {
                let teks_nilai = if text == "-" {
                    Span::styled(text, Style::default().fg(MUTED))
                } else {
                    Span::styled(text, Style::default().fg(Color::White))
                };
                Line::from(vec![
                    Span::styled(
                        format!("{label:<28}"),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    teks_nilai,
                ])
            }
        })
        .collect();
    let paragraph = Paragraph::new(Text::from(lines))
        .block(panel("Detail"))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Lebar kolom tabel jenis dokumen di detail register. Header dan baris memakai angka yang sama.
const NAMA_W: usize = 30;
const NOMOR_W: usize = 24;

/// Detail register: header pekerjaan dan kontrak, lalu satu baris per jenis dokumen.
/// Baris yang disorot adalah jenis yang dipilih. Enter mengisi atau mengubah nomornya.
fn draw_detail_register(frame: &mut Frame, app: &App, value: &serde_json::Value, area: Rect) {
    let teks = |k: &str| {
        value
            .get(k)
            .and_then(crate::app::scalar_text)
            .unwrap_or_else(|| "-".to_string())
    };
    let kontrak = kontrak_utama(value).and_then(|id| {
        value
            .get("kontrak")?
            .as_array()?
            .iter()
            .find(|k| k.get("id").and_then(serde_json::Value::as_u64) == Some(id))
    });
    let spk = kontrak
        .and_then(|k| k.get("spk"))
        .and_then(crate::app::scalar_text)
        .unwrap_or_else(|| "belum ada kontrak".to_string());
    let penyedia = kontrak
        .and_then(|k| k.pointer("/penyedia/nama"))
        .and_then(crate::app::scalar_text)
        .unwrap_or_else(|| "-".to_string());

    let label = |t: &str| {
        Span::styled(
            format!("{t:<12}"),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    };
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            label("Pekerjaan"),
            Span::styled(
                teks("nama_paket"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![label("Kode rek."), Span::raw(teks("kode_rekening"))]),
        Line::from(vec![label("Kontrak"), Span::raw(spk)]),
        Line::from(vec![label("Penyedia"), Span::raw(penyedia)]),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "  {:<NAMA_W$}{:<NOMOR_W$}{}",
                "Jenis dokumen", "Nomor", "Tanggal"
            ),
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
    ];

    if app.jenis.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Jenis dokumen belum dimuat.",
            Style::default().fg(WARN),
        )));
    }
    for (i, j) in app.jenis.iter().enumerate() {
        let nama = format!(
            "{} ({})",
            j.get("name")
                .and_then(crate::app::scalar_text)
                .unwrap_or_default(),
            j.get("code")
                .and_then(crate::app::scalar_text)
                .unwrap_or_default()
        );
        let type_id = j.get("id").and_then(serde_json::Value::as_u64);
        let reg = type_id.and_then(|t| register_untuk_jenis(value, t));
        let (nomor, tanggal, nomor_gaya) = match &reg {
            Some(r) => (
                r.get("nomor")
                    .and_then(crate::app::scalar_text)
                    .unwrap_or_else(|| "-".to_string()),
                r.get("tanggal")
                    .and_then(crate::app::scalar_text)
                    .map(|t| t.chars().take(10).collect::<String>())
                    .unwrap_or_else(|| "-".to_string()),
                Style::default().fg(Color::White),
            ),
            None => (
                "Belum ada".to_string(),
                "-".to_string(),
                Style::default().fg(WARN),
            ),
        };
        let aktif = i == app.jenis_cursor;
        let marker = if aktif { "▶ " } else { "  " };
        let baris = Line::from(vec![
            Span::raw(format!("{marker}{:<NAMA_W$}", truncate(&nama, NAMA_W))),
            Span::styled(
                format!("{:<NOMOR_W$}", truncate(&nomor, NOMOR_W)),
                nomor_gaya,
            ),
            Span::styled(tanggal, Style::default().fg(MUTED)),
        ]);
        lines.push(if aktif { baris.style(sorot()) } else { baris });
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(panel("Register"))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Potong teks ke lebar `n` karakter, dengan elipsis bila terpotong.
fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn draw_form(frame: &mut Frame, app: &App, area: Rect) {
    let Some(form) = &app.form else {
        return;
    };
    let lebar_label = 24;
    let mut lines: Vec<Line> = Vec::new();
    for (i, f) in form.fields.iter().enumerate() {
        let aktif = i == form.fokus;
        let gaya = if aktif {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED)
        };
        let marker = if aktif { "▌ " } else { "  " };
        let wajib = if f.wajib {
            Span::styled(" *", Style::default().fg(ERR))
        } else {
            Span::raw("")
        };

        let (nilai, nilai_gaya) = match f.kind {
            Kind::Pilih => {
                let teks = f.label_terpilih();
                let gaya_nilai = if f.nilai.is_empty() {
                    Style::default().fg(MUTED)
                } else {
                    Style::default().fg(Color::White)
                };
                (
                    format!("{teks}{}", if f.terkunci { "" } else { "  ▾" }),
                    gaya_nilai,
                )
            }
            Kind::Centang => {
                if f.nilai == "1" {
                    ("◉ ya".to_string(), Style::default().fg(OK))
                } else {
                    ("○ tidak".to_string(), Style::default().fg(MUTED))
                }
            }
            _ if f.nilai.is_empty() => {
                let kursor = if aktif && !f.terkunci { "▏" } else { "" };
                (format!("—{kursor}"), Style::default().fg(MUTED))
            }
            _ => {
                let kursor = if aktif && !f.terkunci { "▏" } else { "" };
                (
                    format!("{}{kursor}", f.nilai),
                    Style::default().fg(Color::White),
                )
            }
        };
        let mut spans = vec![
            Span::styled(marker, gaya),
            Span::styled(format!("{:<lebar_label$}", f.label), gaya),
            Span::styled(nilai, nilai_gaya),
        ];
        if f.terkunci {
            spans.push(Span::styled(
                "  (tidak bisa diubah)",
                Style::default().fg(MUTED),
            ));
        }
        spans.push(wajib);
        lines.push(Line::from(spans));
        if let Some(h) = &f.hint {
            lines.push(Line::from(Span::styled(
                format!("{:<w$}{h}", "", w = lebar_label + 2),
                Style::default().fg(ACCENT_SOFT),
            )));
        }
        if let Some(g) = &f.galat {
            lines.push(Line::from(Span::styled(
                format!("{:<w$}✖ {g}", "", w = lebar_label + 2),
                Style::default().fg(ERR),
            )));
        }
        lines.push(Line::from(""));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(panel("Isian"))
            .wrap(Wrap { trim: false }),
        area,
    );

    if form.pemilih.is_some() {
        draw_pemilih(frame, area, form);
    }
}

/// Pemilih dropdown untuk isian aktif, di tengah area formulir.
fn draw_pemilih(frame: &mut Frame, area: Rect, form: &Form) {
    let f = form.fokus_field();
    let Some(idx) = form.pemilih else {
        return;
    };
    let width = 56u16.min(area.width.saturating_sub(4));
    let height = (f.pilihan.len() as u16 + 2)
        .min(area.height.saturating_sub(2))
        .max(3);
    let popup = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + 1,
        width,
        height,
    );
    let items: Vec<ListItem> = f
        .pilihan
        .iter()
        .map(|(_, l)| ListItem::new(format!(" {l}")))
        .collect();
    let list = List::new(items)
        .block(panel(f.label))
        .highlight_style(sorot())
        .highlight_symbol("▶");
    let mut state = ListState::default().with_selected(Some(idx));
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut state);
}

/// Kotak berukuran tetap di tengah `area`, dibatasi ukuran layar.
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Client, DEFAULT_BASE};
    use crossterm::event::{KeyCode, KeyEvent};
    use ratatui::{backend::TestBackend, Terminal};
    use serde_json::json;

    fn teks(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app() -> App {
        App::new(Client::new(DEFAULT_BASE.to_string()).unwrap())
    }

    #[test]
    fn setiap_layar_tergambar_tanpa_panik() {
        let mut a = app();
        assert!(teks(&a).contains("ARUMANIS"));
        assert!(teks(&a).contains("Masuk"));

        a.screen = Screen::Menu;
        assert!(teks(&a).contains("Antrean 0"));

        a.screen = Screen::List;
        a.rows = vec![json!({ "id": 1, "nama_paket": "Jaringan A" })];
        assert!(teks(&a).contains("Jaringan A"));

        a.screen = Screen::Detail;
        a.detail = Some(json!({ "id": 1, "nama_paket": "Jaringan A" }));
        assert!(teks(&a).contains("nama_paket"));

        a.screen = Screen::Form;
        a.form = Some(Form::pekerjaan(None, None, vec![], vec![], vec![], vec![]));
        let layar = teks(&a);
        assert!(layar.contains("Nama paket"));
        assert!(layar.contains("Ctrl+S"));
    }

    #[test]
    fn tampilan_menampilkan_pesan_galat_dan_pemilih() {
        let mut a = app();
        a.screen = Screen::Form;
        let kegiatan = crate::form::pilihan(
            &[json!({ "id": 4, "nama_kegiatan": "Air" })],
            &["nama_kegiatan"],
        );
        a.form = Some(Form::pekerjaan(
            None,
            None,
            kegiatan,
            vec![],
            vec![],
            vec![],
        ));
        a.handle_key(KeyEvent::from(KeyCode::Down));
        a.form.as_mut().unwrap().fokus = 4; // kegiatan
        a.form.as_mut().unwrap().buka_pemilih();
        assert!(teks(&a).contains("kosong"));
    }
}
