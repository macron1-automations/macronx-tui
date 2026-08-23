use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table,
        TableState,
    },
    Frame,
};

use crate::app::App;
use crate::format::date_short;

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
    let title = Paragraph::new(Line::from(Span::styled(
        "Inbox",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(title, chunks[0]);

    let content = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(0)])
        .split(chunks[1]);

    render_sidebar(f, app, content[0]);

    // Table
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from("NAME").style(header_style),
        Cell::from("SUMMARY").style(header_style),
        Cell::from("CREATED").style(header_style),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&idx| {
            let inbox = &app.inboxes[idx];
            Row::new(vec![
                Cell::from(inbox.name.as_str()),
                Cell::from(inbox.summary.as_deref().unwrap_or("")),
                Cell::from(date_short(&inbox.created_at)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(30),
    ];

    let title_text = if app.filtered.len() < app.inboxes.len() {
        format!(" {} / {} inboxes ", app.filtered.len(), app.inboxes.len())
    } else {
        format!(" {} inboxes ", app.inboxes.len())
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title_text),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    let mut state = TableState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(table, content[1], &mut state);

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
            Span::styled("[Shift+J/K] ", Style::default().fg(Color::Cyan)),
            Span::raw("Tags  "),
            Span::styled("[Enter] ", Style::default().fg(Color::Cyan)),
            Span::raw("Open  "),
            Span::styled("[r] ", Style::default().fg(Color::Cyan)),
            Span::raw("Refresh  "),
            Span::styled("[q] ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ])
    };

    let status_bar = Paragraph::new(status_text).style(Style::default().bg(Color::Black));
    f.render_widget(status_bar, chunks[2]);
}

fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::with_capacity(app.tags.len() + 1);

    items.push(ListItem::new(Line::from(Span::styled(
        "All",
        Style::default().fg(Color::Cyan),
    ))));

    for tag in &app.tags {
        items.push(ListItem::new(Line::from(Span::styled(
            tag.name.clone(),
            Style::default().fg(tag_color(tag.color.as_deref())),
        ))));
    }

    let sidebar = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(format!(" {} tags ", app.tags.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");

    let mut state = ListState::default();
    state.select(Some(app.tag_offset()));
    f.render_stateful_widget(sidebar, area, &mut state);
}

fn tag_color(color: Option<&str>) -> Color {
    let color = color.map(|c| c.to_lowercase());
    match color.as_deref() {
        Some(c) if c.contains("purple") || c.contains("violet") => Color::Magenta,
        Some(c) if c.contains("blue") || c.contains("indigo") => Color::Blue,
        Some(c) if c.contains("green") || c.contains("emerald") => Color::Green,
        Some(c) if c.contains("red") || c.contains("rose") => Color::Red,
        Some(c) if c.contains("orange") || c.contains("amber") => Color::Yellow,
        Some(c) if c.contains("pink") => Color::LightMagenta,
        Some(c) if c.contains("cyan") || c.contains("teal") => Color::Cyan,
        Some(c) if c.contains("gray") || c.contains("grey") => Color::DarkGray,
        _ => Color::White,
    }
}
