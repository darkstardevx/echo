use crate::app::{self, App, Mode};
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState, Wrap};

pub fn draw(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    draw_table(frame, app, theme, chunks[0]);
    draw_status(frame, app, theme, chunks[1]);
    draw_footer(frame, app, theme, chunks[2]);

    match app.mode {
        Mode::Detail => draw_detail_popup(frame, area, app, theme),
        Mode::Help => draw_help_popup(frame, area, theme),
        _ => {}
    }
}

fn one_line_preview(rendered: &str, max_len: usize) -> String {
    let first_line = rendered.lines().next().unwrap_or("");
    if first_line.chars().count() > max_len {
        let truncated: String = first_line.chars().take(max_len.saturating_sub(1)).collect();
        format!("{truncated}…")
    } else {
        first_line.to_string()
    }
}

fn draw_table(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let header = Row::new(vec!["", "When", "Pipeline", "Dir", "Format", "Preview"]).style(
        Style::default()
            .fg(theme.orange)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&i| {
            let f = &app.flows[i];
            let (dir_label, dir_color) = if f.direction.eq_ignore_ascii_case("outbound") {
                ("OUT", theme.acid_green)
            } else {
                ("IN", theme.cyan)
            };

            Row::new(vec![
                Line::from(Span::styled("●", Style::default().fg(theme.purple))),
                Line::from(Span::styled(
                    f.at.format("%H:%M:%S%.3f").to_string(),
                    Style::default().fg(theme.muted),
                )),
                Line::from(Span::styled(
                    f.pipeline.clone(),
                    Style::default().fg(theme.white),
                )),
                Line::from(Span::styled(dir_label, Style::default().fg(dir_color))),
                Line::from(Span::styled(
                    f.format.clone(),
                    Style::default().fg(theme.muted),
                )),
                Line::from(Span::styled(
                    one_line_preview(&f.rendered, 80),
                    Style::default().fg(theme.white),
                )),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(13),
        Constraint::Percentage(20),
        Constraint::Length(4),
        Constraint::Length(9),
        Constraint::Min(20),
    ];

    let follow_indicator = if app.follow_live {
        " [following]"
    } else {
        " [browsing]"
    };
    let title = format!(" echo — {} flows{} ", app.flows.len(), follow_indicator);
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(theme.hot_pink)
                .fg(theme.white)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.orange))
                .title(Span::styled(title, Style::default().fg(theme.orange))),
        );

    let mut state = TableState::default();
    state.select(if app.filtered.is_empty() {
        None
    } else {
        Some(app.selected)
    });
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let (text, color) = match &app.mode {
        Mode::Filter => (format!("filter: {}_", app.filter_text), theme.cyan),
        _ if app.flows.is_empty() => (
            "no flows yet — waiting for traffic through a WraithFlow pipeline with capture_log enabled"
                .to_string(),
            theme.red,
        ),
        _ => (app.status.clone().unwrap_or_default(), theme.cyan),
    };
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(color)), area);
}

fn draw_footer(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let follow_key = if app.follow_live {
        "f pause"
    } else {
        "f follow"
    };
    let text =
        format!("j/k nav  enter/l details  /  filter  {follow_key}  R reload  ? help  q quit");
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.muted)),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_detail_popup(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let popup = centered_rect(85, 75, area);
    frame.render_widget(Clear, popup);
    let text = app
        .selected_flow()
        .map(app::detail_lines)
        .unwrap_or_default();
    let block = Block::default()
        .style(Style::default().bg(theme.panel).fg(theme.white))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.orange))
        .title(Span::styled(
            " flow detail (Esc to close) ",
            Style::default().fg(theme.orange),
        ));
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(block),
        popup,
    );
}

fn draw_help_popup(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 55, area);
    frame.render_widget(Clear, popup);
    let lines = [
        "j/k, ↑/↓   move",
        "/          filter by pipeline name or content",
        "enter, l   show full flow detail",
        "f          toggle follow-live / browse-history",
        "R          reload the capture log from disk",
        "?          toggle this help",
        "q, Esc     quit (Esc closes a popup first)",
    ]
    .join("\n");
    let block = Block::default()
        .style(Style::default().bg(theme.panel).fg(theme.white))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.line))
        .title(Span::styled(
            " echo — keys ",
            Style::default().fg(theme.line),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}
