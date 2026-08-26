//! 렌더링 — 탭/본문/상태바 레이아웃, 화면 디스패치, 도움말 오버레이, 공용 헬퍼.

mod clusters;
mod detail;
mod failed_tx;
mod overview;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, Screen};

/// 한 프레임 전체를 그린다.
pub fn draw(f: &mut Frame<'_>, app: &App) {
    let full = f.area();
    // amarillo 워드마크 배너 — 터미널이 충분히 크면 탭 박스 위에 표시(작으면 생략).
    let banner_h: u16 = if full.height >= 18 && full.width >= 34 {
        3
    } else {
        0
    };
    let chunks = Layout::vertical([
        Constraint::Length(banner_h),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(full);

    if banner_h > 0 {
        render_banner(f, chunks[0]);
    }
    render_tabs(f, chunks[1], app);
    match app.screen {
        Screen::Overview => overview::render(f, chunks[2], app),
        Screen::FailedTx => failed_tx::render(f, chunks[2], app),
        Screen::Detail => detail::render(f, chunks[2], app),
        Screen::Clusters => clusters::render(f, chunks[2], app),
    }
    render_status(f, chunks[3], app);

    if app.show_help {
        render_help(f, full);
    }
}

/// amarillo 워드마크 배너. `amarillo`는 스페인어로 '노란색' — 앰버 그라데이션으로
/// 카테고리 팔레트(슬리피지 앰버 톤)와 시각적으로 맞춘다.
fn render_banner(f: &mut Frame<'_>, area: Rect) {
    let bright = Color::Rgb(0xF4, 0xBD, 0x50);
    let deep = Color::Rgb(0xE5, 0xA9, 0x3D);
    let lines = vec![
        Line::from(Span::styled(
            " ▄▀█ █▀▄▀█ ▄▀█ █▀█ █ █   █   █▀█",
            Style::default().fg(bright).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " █▀█ █ ▀ █ █▀█ █▀▄ █ █▄▄ █▄▄ █▄█",
            Style::default().fg(deep).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  failure intelligence · in your terminal",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn render_tabs(f: &mut Frame<'_>, area: Rect, app: &App) {
    let titles = vec![
        "[1] Overview",
        "[2] Failed Tx",
        "[3] Detail",
        "[4] Clusters",
    ];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" amarillo "))
        .select(app.screen.index())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");
    f.render_widget(tabs, area);
}

fn render_status(f: &mut Frame<'_>, area: Rect, app: &App) {
    let spinner = if app.is_busy() {
        format!("{} ", spinner_frame(app.tick))
    } else {
        String::new()
    };
    let poll = if app.auto_poll {
        format!("poll on {}s", app.config.refresh_interval_secs)
    } else {
        "poll off".to_string()
    };
    let updated = match app.updated_secs_ago() {
        Some(s) => format!("updated {s}s ago"),
        None => "—".to_string(),
    };
    let hints = match app.screen {
        Screen::Overview => "Tab switch · r refresh · p poll",
        Screen::FailedTx => "j/k move · Enter detail · c cat · t window · n/b page",
        Screen::Detail => "j/k scroll · Esc back",
        Screen::Clusters => "j/k move · r refresh · p poll",
    };
    let line = format!(
        "{spinner}{} · {poll} · {updated} · {hints} · ? help · q quit",
        app.config.api_url
    );
    f.render_widget(
        Paragraph::new(line).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_help(f: &mut Frame<'_>, area: Rect) {
    let rect = centered_rect(62, 70, area);
    f.render_widget(Clear, rect);
    let text = vec![
        Line::from("amarillo-tui — keys"),
        Line::from(""),
        Line::from("Tab / Shift+Tab    switch tab"),
        Line::from("1 / 2 / 3 / 4      Overview / Failed Tx / Detail / Clusters"),
        Line::from("j / k  ↑ / ↓       move selection / scroll"),
        Line::from("Enter              open detail for selected row"),
        Line::from("Esc                back"),
        Line::from("c / t              cycle category / time window"),
        Line::from("n / b              next / previous page"),
        Line::from("r                  refresh now"),
        Line::from("p                  toggle auto-poll"),
        Line::from("? / Esc / q        close this help"),
        Line::from("q / Ctrl+C         quit"),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Help ")),
        rect,
    );
}

/// 진행 중 스피너 프레임 (braille).
fn spinner_frame(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

/// 중앙 정렬된 상태 메시지 (loading/error/empty 공용).
fn banner(f: &mut Frame<'_>, area: Rect, msg: &str, color: Color) {
    f.render_widget(
        Paragraph::new(msg.to_string())
            .style(Style::default().fg(color))
            .alignment(Alignment::Center),
        area,
    );
}

/// 블록을 먼저 그리고 그 안쪽에 배너를 그린다.
fn render_block_banner(f: &mut Frame<'_>, area: Rect, block: Block<'_>, msg: &str, color: Color) {
    let inner = block.inner(area);
    f.render_widget(block, area);
    banner(f, inner, msg, color);
}

/// `area` 중앙에 `percent_x` × `percent_y` 비율의 사각형.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
