//! Tampilan ratatui. Hanya membaca `App`, tidak mengubah state.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use chrono::Datelike;

use crate::api::Module;
use crate::app::{scalar_text, App, Screen};

const ACCENT: Color = Color::Rgb(0x0B, 0x3B, 0x78);

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, body, help, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let title = match app.screen {
        Screen::Login => "Arumanis · Masuk".to_string(),
        Screen::Menu => "Arumanis · Menu".to_string(),
        Screen::List => format!("Arumanis · {}", app.module.title()),
        Screen::Detail => format!("Arumanis · {} · detail", app.module.title()),
    };
    frame.render_widget(
        Paragraph::new(title).style(Style::default().fg(Color::White).bg(ACCENT).add_modifier(Modifier::BOLD)),
        header,
    );

    match app.screen {
        Screen::Login => draw_login(frame, app, body),
        Screen::Menu => draw_menu(frame, app, body),
        Screen::List => draw_list(frame, app, body),
        Screen::Detail => draw_detail(frame, app, body),
    }

    frame.render_widget(
        Paragraph::new(format!(" {}", help_line(app))).style(Style::default().fg(Color::DarkGray)),
        help,
    );
    frame.render_widget(
        Paragraph::new(format!(" {}", app.status)).style(Style::default().fg(Color::Gray)),
        status,
    );
}

fn help_line(app: &App) -> &'static str {
    match app.screen {
        Screen::Login => "Tab ganti kolom · Enter masuk · Esc keluar",
        Screen::Menu => "↑/↓ pilih · Enter buka · l keluar · q keluar aplikasi",
        Screen::List if app.searching => "Ketik kata kunci · Enter cari · Esc batal",
        Screen::List if app.year_menu.is_some() => "↑/↓ pilih tahun · Enter terapkan · Esc batal",
        Screen::List => "↑/↓ pilih · Enter detail · / cari · t tahun · ←/→ halaman · r muat ulang · Esc kembali",
        Screen::Detail => "Esc atau Backspace kembali",
    }
}

fn draw_login(frame: &mut Frame, app: &App, area: Rect) {
    let masked = "•".repeat(app.password.chars().count());
    let field = |label: &str, value: &str, active: bool| {
        let style = if active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(vec![Span::styled(format!("{label:<10}"), style), Span::raw(value.to_string())])
    };
    let lines = vec![
        Line::from(""),
        field("Email", &app.email, app.login_field == 0),
        Line::from(""),
        field("Password", &masked, app.login_field == 1),
        Line::from(""),
    ];
    let block = Block::bordered().title(" Masuk ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Module::ALL
        .iter()
        .map(|m| ListItem::new(m.title()))
        .collect();
    items.push(ListItem::new("Keluar"));
    let list = List::new(items)
        .block(Block::bordered().title(" Menu "))
        .highlight_style(Style::default().bg(ACCENT).fg(Color::White))
        .highlight_symbol("▶ ");
    let mut state = ListState::default().with_selected(Some(app.menu_idx));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let [search_area, rows_area] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    let search_text = if app.searching {
        format!("{}▏", app.search)
    } else if app.search.is_empty() {
        "tekan / untuk mencari".to_string()
    } else {
        format!("cari: {}", app.search)
    };
    let title = if app.cached_at.is_some() {
        format!(" Tahun {} · halaman {} · offline ", app.year, app.page)
    } else {
        format!(" Tahun {} · halaman {} ", app.year, app.page)
    };
    frame.render_widget(
        Paragraph::new(search_text).block(Block::bordered().title(title)),
        search_area,
    );

    let items: Vec<ListItem> = if app.rows.is_empty() {
        vec![ListItem::new("Tidak ada data di halaman ini.")]
    } else {
        app.rows.iter().map(|r| ListItem::new(app.row_label(r))).collect()
    };
    let list = List::new(items)
        .block(Block::bordered())
        .highlight_style(Style::default().bg(ACCENT).fg(Color::White))
        .highlight_symbol("▶ ");
    let selected = if app.rows.is_empty() { None } else { Some(app.selected) };
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(list, rows_area, &mut state);

    if let Some(idx) = app.year_menu {
        draw_year_menu(frame, rows_area, idx);
    }
}

/// Dropdown tahun anggaran, digambar di atas daftar.
fn draw_year_menu(frame: &mut Frame, area: Rect, idx: usize) {
    let options = crate::cache::year_options(chrono::Local::now().year());
    let width = 16u16.min(area.width);
    let height = (options.len() as u16 + 2).min(area.height);
    let popup = Rect::new(area.x + area.width.saturating_sub(width), area.y, width, height);
    let items: Vec<ListItem> = options.iter().map(|y| ListItem::new(y.to_string())).collect();
    let list = List::new(items)
        .block(Block::bordered().title(" Tahun "))
        .highlight_style(Style::default().bg(ACCENT).fg(Color::White))
        .highlight_symbol("▶ ");
    let mut state = ListState::default().with_selected(Some(idx));
    frame.render_widget(Clear, popup);
    frame.render_stateful_widget(list, popup, &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let Some(value) = &app.detail else {
        frame.render_widget(Paragraph::new("Tidak ada detail."), area);
        return;
    };
    let mut lines: Vec<Line> = Vec::new();
    if let Some(obj) = value.as_object() {
        let mut keys: Vec<&String> = obj.keys().collect();
        keys.sort();
        for key in keys {
            let text = scalar_text(&obj[key]).unwrap_or_else(|| {
                let raw = obj[key].to_string();
                if raw.chars().count() > 120 {
                    format!("{}…", raw.chars().take(120).collect::<String>())
                } else {
                    raw
                }
            });
            lines.push(Line::from(vec![
                Span::styled(format!("{key:<28}"), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                Span::raw(text),
            ]));
        }
    } else {
        lines.push(Line::from(value.to_string()));
    }
    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::bordered())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
