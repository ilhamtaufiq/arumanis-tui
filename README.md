# arumanis-tui

Aplikasi terminal (ratatui) untuk Jurnal Kegiatan SKP di Arumanis.

## Menjalankan

```bash
APIAMIS_URL=https://apiamis.cianjur.space/api cargo run --release
```

`APIAMIS_URL` default-nya `https://apiamis.cianjur.space/api`.

## Tombol

Login: Tab pindah kolom, Enter masuk, Esc keluar.

Jurnal: ←/→ ganti bulan, ↑/↓ pilih kegiatan, n tambah, e ubah, d hapus, r muat ulang, q keluar.

Form: Tab/Shift+Tab pindah kolom, Enter simpan, Esc batal.

Token login hanya disimpan di memori. Tidak ada yang ditulis ke disk.

## Batasan versi ini

- Belum ada foto bukti, pengaturan RHK, atau export PPTX.
- Panggilan API bersifat blocking. Saat jaringan lambat, layar bisa tertahan sebentar.
