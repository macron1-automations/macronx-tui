use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use tui_scrollview::{ScrollView, ScrollViewState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::App;
use crate::models::Inbox;

pub fn render(f: &mut Frame, app: &mut App) {
    let inbox = match &app.current_inbox {
        Some(i) => i,
        None => return,
    };

    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Breadcrumb header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("← Inbox", Style::default().fg(Color::DarkGray)),
        Span::raw(" / "),
        Span::styled(
            inbox.name.as_str(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(header, chunks[0]);

    // Details block
    render_details(f, inbox, chunks[1]);

    render_body(f, inbox, chunks[2], &mut app.body_scroll);

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
            Span::raw("Scroll  "),
            Span::styled("[PgUp/PgDn] ", Style::default().fg(Color::Cyan)),
            Span::raw("Page  "),
            Span::styled("[Esc/q] ", Style::default().fg(Color::Cyan)),
            Span::raw("Back to list"),
        ])
    };

    let status_bar = Paragraph::new(status_text).style(Style::default().bg(Color::Black));
    f.render_widget(status_bar, chunks[3]);
}

fn render_details(f: &mut Frame, inbox: &Inbox, area: Rect) {
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Source     ", label_style),
            Span::styled(inbox.source.as_str(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Summary    ", label_style),
            Span::styled(inbox.summary.as_deref().unwrap_or("—"), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Created    ", label_style),
            Span::styled(format_datetime(&inbox.created_at), value_style),
        ]),
    ];

    let details = Paragraph::new(lines).block(
        Block::default()
            .title(" Details ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(details, area);
}

fn render_body(
    f: &mut Frame,
    inbox: &Inbox,
    area: Rect,
    state: &mut ScrollViewState,
) {
    let block = Block::default()
        .title(" Body ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.width < 2 || inner.height < 1 {
        return;
    }

    let width = (inner.width.saturating_sub(1)).max(1);
    let body = inbox.body.as_deref().unwrap_or("");
    let lines = style_markdown(body, width as usize);

    let content_height = (lines.len() as u16).max(1);
    let mut scroll_view = ScrollView::new(Size::new(width, content_height));
    scroll_view.render_widget(
        Paragraph::new(lines),
        Rect::new(0, 0, width, content_height),
    );
    f.render_stateful_widget(scroll_view, inner, state);
}

fn style_markdown(text: &str, width: usize) -> Vec<Line<'static>> {
    let heading_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let code_style = Style::default().fg(Color::Green);
    let quote_style = Style::default().fg(Color::Blue);
    let plain_style = Style::default().fg(Color::White);

    let mut out = Vec::new();
    let mut in_code = false;

    for raw in text.lines() {
        let trimmed = raw.trim_start();
        if is_code_fence(trimmed) {
            in_code = !in_code;
            push_wrapped(&mut out, raw, width, &code_style);
            continue;
        }

        let style = if in_code {
            &code_style
        } else if is_heading(trimmed) {
            &heading_style
        } else if trimmed.starts_with('>') {
            &quote_style
        } else {
            &plain_style
        };

        push_wrapped(&mut out, raw, width, style);
    }

    out
}

fn push_wrapped(out: &mut Vec<Line<'static>>, raw: &str, width: usize, style: &Style) {
    for piece in wrap_text(raw, width) {
        out.push(Line::styled(piece, *style));
    }
}

fn is_code_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    hashes > 0
        && (line.chars().nth(hashes) == Some(' ') || hashes == line.chars().count())
}

fn wrap_text(line: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    if width == 0 {
        out.push(line.to_string());
        return out;
    }
    if line.is_empty() {
        out.push(String::new());
        return out;
    }

    let mut current = String::new();
    let mut current_width = 0usize;

    for token in line.split_inclusive(char::is_whitespace) {
        let token_width = token.width();
        if current_width + token_width > width && current_width > 0 {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if token_width > width && current_width == 0 {
            let mut chunk = String::new();
            let mut chunk_width = 0usize;
            for ch in token.chars() {
                let ch_width = ch.width().unwrap_or(0);
                if chunk_width + ch_width > width && chunk_width > 0 {
                    out.push(std::mem::take(&mut chunk));
                    chunk_width = 0;
                }
                chunk.push(ch);
                chunk_width += ch_width;
            }
            current = chunk;
            current_width = chunk_width;
        } else {
            current.push_str(token);
            current_width += token_width;
        }
    }

    if current_width > 0 {
        out.push(current);
    }
    out
}

fn format_datetime(s: &str) -> String {
    // "2026-06-12T20:40:00.000Z" → "June 12, 2026 at 20:40 UTC"
    if s.len() >= 16 {
        let date = &s[..10];
        let time = &s[11..16];
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "January",
                "02" => "February",
                "03" => "March",
                "04" => "April",
                "05" => "May",
                "06" => "June",
                "07" => "July",
                "08" => "August",
                "09" => "September",
                "10" => "October",
                "11" => "November",
                "12" => "December",
                _ => parts[1],
            };
            let day = parts[2].trim_start_matches('0');
            return format!("{} {}, {} at {} UTC", month, day, parts[0], time);
        }
    }
    s.to_string()
}
