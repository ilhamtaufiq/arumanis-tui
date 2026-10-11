# arumanis-tui

Aplikasi terminal (ratatui) untuk Arumanis: Pekerjaan, Kontrak, dan Register Dokumen.

## Menjalankan

```bash
APIAMIS_URL=https://apiamis.cianjur.space/api cargo run --release
```

`APIAMIS_URL` default-nya `https://apiamis.cianjur.space/api`.

## Tombol

Menu: ↑/↓ pilih, Enter buka, l keluar, q tutup.

Daftar: ↑/↓ pilih, Enter detail, / cari, t tahun anggaran, ←/→ halaman, n pekerjaan baru,
s kirim antrean, x buang perubahan yang ditolak server, Esc kembali.

Detail: e ubah (Pekerjaan dan Register), n tambah register di dalam kontrak (Kontrak), Esc kembali.

Formulir: Tab/↑/↓ pindah isian, Enter buka dropdown atau pindah, Spasi centang, Ctrl+S simpan, Esc batal.
Dropdown: ↑/↓ pilih, Enter pakai, Esc batal.

## Offline

- Daftar dan lookup (kegiatan, kecamatan, desa, pengawas, jenis dokumen) disimpan ke cache di
  `%LOCALAPPDATA%\arumanis-tui` (Windows) atau `~/.cache/arumanis-tui`. Saat tidak ada koneksi, data dibaca dari cache.
- Simpan saat offline masuk ke antrean (`antrean.json` di folder yang sama). Antrean dikirim saat tekan `s`
  atau otomatis setelah login. Penolakan server tetap di antrean dengan pesannya, dan bisa dibuang dengan `x`.

Token login hanya disimpan di memori. Tidak ada token yang ditulis ke disk.

## Pengujian

```bash
cargo test
# Uji alur login ke server sungguhan (opsional):
APIAMIS_TEST_URL=... APIAMIS_TEST_EMAIL=... APIAMIS_TEST_PASSWORD=... cargo test -- --ignored
```

## Batasan

- Belum ada foto bukti, pengaturan RHK, atau export PPTX.
- Panggilan API bersifat blocking. Saat jaringan lambat, layar bisa tertahan sebentar.
