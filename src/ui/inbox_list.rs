use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Macronx", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" / "),
        Span::styled("Inboxes", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(title, chunks[0]);

    // Table
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from("NAME").style(header_style),
        Cell::from("SOURCE").style(header_style),
        Cell::from("SUMMARY").style(header_style),
        Cell::from("CREATED").style(header_style),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .inboxes
        .iter()
        .map(|inbox| {
            Row::new(vec![
                Cell::from(inbox.name.as_str()),
                Cell::from(inbox.source.as_str()),
                Cell::from(inbox.summary.as_deref().unwrap_or("")),
                Cell::from(format_date(&inbox.created_at)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(35),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" {} inboxes ", app.inboxes.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    let mut state = TableState::default();
    if !app.inboxes.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(table, chunks[1], &mut state);

    // Status bar
    let status_text = if let Some((msg, is_error)) = &app.status {
        let style = if *is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        Line::from(Span::styled(format!(" {}", msg), style))
    } else {
        Line::from(vec![
            Span::styled(" [j/k] ", Style::default().fg(Color::Cyan)),
            Span::raw("Navigate  "),
            Span::styled("[Enter] ", Style::default().fg(Color::Cyan)),
            Span::raw("Open  "),
            Span::styled("[r] ", Style::default().fg(Color::Cyan)),
            Span::raw("Refresh  "),
            Span::styled("[q] ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ])
    };

    let status_bar = Paragraph::new(status_text)
        .style(Style::default().bg(Color::Black));
    f.render_widget(status_bar, chunks[2]);
}

fn format_date(s: &str) -> String {
    // "2026-06-12T20:40:00.000Z" → "Jun 12, 2026"
    if s.len() >= 10 {
        let date = &s[..10];
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[1],
            };
            let day = parts[2].trim_start_matches('0');
            return format!("{} {}, {}", month, day, parts[0]);
        }
    }
    s.to_string()
}
