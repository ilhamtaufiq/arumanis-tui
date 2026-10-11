# arumanis-tui

Aplikasi terminal (ratatui) untuk Arumanis: Pekerjaan, Kontrak, dan Register Dokumen.

Daftar Register memuat pekerjaan beserta nomor per jenis dokumen, seperti halaman Register di frontend.
Nomor baru dipratinjau dari template jenis dokumen. Sequence mengikuti tahun aktif.

## Menjalankan

```bash
APIAMIS_URL=https://apiamis.cianjur.space/api cargo run --release
```

`APIAMIS_URL` default-nya `https://apiamis.cianjur.space/api`.

## Tombol

Menu: ↑/↓ pilih, Enter buka, l keluar, q tutup.

Daftar: ↑/↓ pilih, Enter detail, / cari, t tahun anggaran, ←/→ halaman, n pekerjaan baru,
s kirim antrean, x buang perubahan yang ditolak server, Esc kembali.

Detail pekerjaan: e ubah, Esc kembali.
Detail register: ↑/↓ pilih jenis dokumen, Enter isi atau ubah nomornya, Esc kembali.
Detail kontrak: n tambah register di dalam kontrak, Esc kembali.
Register (daftar): S ubah nomor urut terakhir tahun aktif (sequence).

Formulir: Tab/↑/↓ pindah isian, Enter buka dropdown atau pindah, Spasi centang, Ctrl+S simpan, Esc batal.
Dropdown: ↑/↓ pilih, Enter pakai, Esc batal.

## Offline

- Daftar dan lookup (kegiatan, kecamatan, desa, pengawas, jenis dokumen) disimpan ke cache di
  `%LOCALAPPDATA%\arumanis-tui` (Windows) atau `~/.cache/arumanis-tui`. Saat tidak ada koneksi, data dibaca dari cache.
- Simpan saat offline masuk ke antrean (`antrean.json` di folder yang sama). Antrean dikirim saat tekan `s`
  atau otomatis setelah login. Penolakan server tetap di antrean dengan pesannya, dan bisa dibuang dengan `x`.

Login offline:

- Setiap login online berhasil, token disimpan di `sesi.bin` (folder cache yang sama) dalam keadaan terenkripsi
  AES-256-GCM. Kuncinya diturunkan dari password dengan PBKDF2, jadi tidak ada kunci yang disimpan terpisah.
- Saat server tidak bisa dihubungi, login memakai sesi itu bila email dan password cocok dan sesi belum kedaluwarsa.
  Masa berlakunya 30 hari. Saat sinkron, server tetap memutuskan: bila token di server sudah habis, masuk online lagi.
  Token di server default-nya 12 jam (`SANCTUM_TOKEN_EXPIRATION`=720 menit), jadi sesi 30 hari hanya berguna
  penuh bila server juga diatur ke 30 hari (43200 menit).
- Login online baru menggantikan sesi yang tersimpan. Keluar (logout) menghapus `sesi.bin`.
- Lookup dan daftar tidak terenkripsi (hanya data baca).

## Pengujian

```bash
cargo test
# Uji alur login ke server sungguhan (opsional):
APIAMIS_TEST_URL=... APIAMIS_TEST_EMAIL=... APIAMIS_TEST_PASSWORD=... cargo test -- --ignored
```

## Batasan

- Di luar cakupan TUI: foto bukti, pengaturan RHK, dan export PPTX. Fitur itu tetap di aplikasi web.
- Panggilan API bersifat blocking. Saat jaringan lambat, layar bisa tertahan sebentar.
