//! Clusters 화면 — UNKNOWN revert 클러스터 테이블.
//!
//! `GET /v1/analytics/failed-tx/unknown-clusters`의 행을 뷰 정렬 그대로
//! (`occurrences DESC, total_gas_wasted DESC`) 나열한다. 상위 클러스터가
//! classifier 신규 룰 후보 — 이 화면이 곧 룰 백로그다.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, Loadable};
use crate::format;

/// Clusters 본문을 그린다.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    render_table(f, rows[0], app);
    render_footer(f, rows[1], app);
}

fn render_table(f: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Unknown revert clusters ");
    match &app.clusters {
        Loadable::Loaded(rows) if !rows.is_empty() => {
            let header = Row::new(vec![
                "template",
                "kind",
                "count",
                "% unk",
                "gas total",
                "gas avg",
                "last seen",
            ])
            .style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            );
            let body: Vec<Row> = rows
                .iter()
                .map(|c| {
                    Row::new(vec![
                        Cell::from(c.template.clone()),
                        Cell::from(ratatui::text::Span::styled(
                            c.cluster_kind.clone(),
                            Style::default().fg(format::cluster_kind_color(&c.cluster_kind)),
                        )),
                        Cell::from(format::group_thousands(c.occurrences)),
                        Cell::from(format::format_pct_str(&c.pct_of_unknown)),
                        Cell::from(format::format_compact(format::to_number(
                            &c.total_gas_wasted,
                        ))),
                        Cell::from(format::format_compact(format::to_number(&c.avg_gas_wasted))),
                        Cell::from(format::time_ago(&c.last_seen)),
                    ])
                })
                .collect();
            let widths = [
                Constraint::Min(20),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(11),
            ];
            let table = Table::new(body, widths)
                .header(header)
                .block(block)
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("▶ ");
            let mut state = app.cluster_state.clone();
            f.render_stateful_widget(table, area, &mut state);
        }
        Loadable::Loaded(_) => super::render_block_banner(
            f,
            area,
            block,
            "No UNKNOWN failures — every revert is classified.",
            Color::DarkGray,
        ),
        Loadable::Failed(e) => {
            super::render_block_banner(f, area, block, &format!("Error: {e}"), Color::Red)
        }
        _ => super::render_block_banner(f, area, block, "Loading…", Color::Yellow),
    }
}

fn render_footer(f: &mut Frame<'_>, area: Rect, app: &App) {
    let text = match &app.clusters {
        Loadable::Loaded(rows) => {
            let senders: i64 = rows.iter().map(|c| c.distinct_senders).sum();
            format!(
                " {} cluster(s) · {} distinct sender(s) · ordered by occurrences desc",
                rows.len(),
                senders
            )
        }
        _ => " —".to_string(),
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
