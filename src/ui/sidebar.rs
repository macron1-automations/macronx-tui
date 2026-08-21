use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use ratatui_image::{protocol::StatefulProtocol, Resize, StatefulImage};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, AssetState, Focus, SidebarTab};
use crate::models::Attachment;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(tab_title(app))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);
    if inner.width < 4 || inner.height < 3 {
        return;
    }

    match app.sidebar_tab {
        SidebarTab::Metadata => render_metadata(f, app, inner),
        SidebarTab::Attachments => render_attachments(f, app, inner),
    }
}

fn tab_title(app: &App) -> Line<'static> {
    let active = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(Color::DarkGray);
    let count = app.attachments().len();

    let (meta_style, att_style) = if app.sidebar_tab == SidebarTab::Metadata {
        (active, inactive)
    } else {
        (inactive, active)
    };

    Line::from(vec![
        Span::styled(" Metadata ", meta_style),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" Attachments ({}) ", count), att_style),
    ])
}

fn render_metadata(f: &mut Frame, app: &App, area: Rect) {
    let Some(inbox) = &app.current_inbox else {
        return;
    };

    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);
    let accent_style = Style::default().fg(Color::Cyan);

    let mut lines = vec![
        kv_line("ID", &inbox.id.to_string(), label_style, accent_style),
        kv_line("Source", inbox.source.as_str(), label_style, accent_style),
        kv_line(
            "Tag",
            inbox.tag.as_deref().unwrap_or("—"),
            label_style,
            value_style,
        ),
        kv_line(
            "Created",
            &format_datetime_short(&inbox.created_at),
            label_style,
            value_style,
        ),
        kv_line(
            "Files",
            &format!("{}", inbox.attachments.len()),
            label_style,
            value_style,
        ),
        Line::from(""),
        Line::from(Span::styled("Summary", label_style)),
    ];

    let summary = inbox.summary.as_deref().unwrap_or("—");
    for piece in wrap_plain(summary, area.width.saturating_sub(2) as usize) {
        lines.push(Line::from(Span::styled(piece, value_style)));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn kv_line<'a>(key: &str, value: &str, label_style: Style, value_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{:<9}", key), label_style),
        Span::styled(value.to_string(), value_style),
    ])
}

fn render_attachments(f: &mut Frame, app: &mut App, area: Rect) {
    let attachments = app.attachments().to_vec();
    if attachments.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " No attachments",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let selected = app.selected_attachment.min(attachments.len() - 1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Percentage(55)])
        .split(area);

    let items: Vec<ListItem> = attachments
        .iter()
        .enumerate()
        .map(|(i, att)| {
            let tag_style = if att.is_image() {
                Style::default().fg(Color::Cyan)
            } else if att.is_audio() {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let name_style = if i == selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                Span::styled(type_tag(att), tag_style),
                Span::raw(" "),
                Span::styled(
                    truncate(&att.filename, chunks[0].width.saturating_sub(12) as usize),
                    name_style,
                ),
                Span::styled(
                    format!(" {}", human_size(att.byte_size)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    let list = List::new(items).highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let att = &attachments[selected];
    if att.is_image() {
        render_image_preview(f, app, att.id, chunks[1]);
    } else if att.is_audio() {
        render_audio_panel(f, app, att, chunks[1]);
    }
}

fn render_image_preview(f: &mut Frame, app: &mut App, id: u64, area: Rect) {
    let state_kind = match app.asset_state(id) {
        Some(AssetState::Ready(_)) => 1,
        Some(AssetState::Loading) => 2,
        Some(AssetState::Failed(_)) => 3,
        Some(AssetState::AudioReady) | None => 0,
    };

    match state_kind {
        1 => {
            if let Some(protocol) = ready_protocol_mut(app, id) {
                let image = StatefulImage::new(None).resize(Resize::Fit(None));
                f.render_stateful_widget(image, area, protocol);
            }
        }
        2 => {
            let spinner = spinner_frame();
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {} Loading…", spinner),
                    Style::default().fg(Color::Yellow),
                ))),
                centered_top(area),
            );
        }
        3 => {
            let err = match app.asset_state(id) {
                Some(AssetState::Failed(e)) => e.clone(),
                _ => String::new(),
            };
            f.render_widget(
                Paragraph::new(wrap_error(&err, area.width as usize))
                    .style(Style::default().fg(Color::Red)),
                centered_top(area),
            );
        }
        _ => {}
    }
}

fn ready_protocol_mut(app: &mut App, id: u64) -> Option<&mut Box<dyn StatefulProtocol>> {
    match app.assets.get_mut(&id) {
        Some(AssetState::Ready(asset)) => Some(&mut asset.protocol),
        _ => None,
    }
}

fn render_audio_panel(f: &mut Frame, app: &mut App, att: &Attachment, area: Rect) {
    let loaded = app
        .audio
        .as_ref()
        .map(|a| a.attachment_id == att.id)
        .unwrap_or(false);

    let mut lines: Vec<Line> = Vec::new();

    if !loaded {
        let cached = matches!(app.asset_state(att.id), Some(AssetState::AudioReady));
        let status = if cached {
            Span::styled(
                " ⏵ Ready — [space] to play",
                Style::default().fg(Color::Cyan),
            )
        } else {
            match app.asset_state(att.id) {
                Some(AssetState::Loading) => Span::styled(
                    format!(" {} Loading…", spinner_frame()),
                    Style::default().fg(Color::Yellow),
                ),
                Some(AssetState::Failed(e)) => {
                    Span::styled(format!(" ✗ {}", e), Style::default().fg(Color::Red))
                }
                _ => Span::styled(
                    " [space] to load & play",
                    Style::default().fg(Color::DarkGray),
                ),
            }
        };
        lines.push(Line::from(status));
        f.render_widget(Paragraph::new(lines), area);
        return;
    }

    let Some(audio) = &app.audio else {
        return;
    };

    let pos = audio.position();
    let total = audio.duration.unwrap_or(pos);
    let play_symbol = if audio.is_paused() { "⏸" } else { "▶" };

    lines.push(Line::from(Span::styled(
        format!(" {} {}", play_symbol, format_duration(pos)),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    // Waveform overview with playhead
    let width = area.width.saturating_sub(2) as usize;
    let peaks = audio.peaks.clone();
    let progress = if total.as_secs_f64() > 0.0 {
        (pos.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let playhead_col = ((progress * width as f64) as usize).min(width.saturating_sub(1));

    for row in waveform_rows(&peaks, width, 3) {
        let played_cols = playhead_col.min(row.chars().count());
        let played: String = row.chars().take(played_cols).collect();
        let remaining: String = row.chars().skip(played_cols).collect();
        let mut spans = Vec::with_capacity(2);
        if !played.is_empty() {
            spans.push(Span::styled(played, Style::default().fg(Color::Cyan)));
        }
        if !remaining.is_empty() {
            spans.push(Span::styled(
                remaining,
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(spans));
    }

    // Live level visualization
    let levels = audio.viz.snapshot();
    let viz: String = levels
        .iter()
        .rev()
        .take(width)
        .rev()
        .map(|l| level_char(*l))
        .collect();
    if !viz.is_empty() {
        lines.push(Line::from(Span::styled(
            viz,
            Style::default().fg(Color::Magenta),
        )));
    }

    lines.push(Line::from(Span::styled(
        " [space] pause  [←/→] seek",
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines), area);
}

fn waveform_rows(peaks: &[f32], width: usize, height: usize) -> Vec<String> {
    let mut rows = vec![String::new(); height];
    if width == 0 || peaks.is_empty() {
        for row in &mut rows {
            row.push_str(&" ".repeat(width));
        }
        return rows;
    }

    let max_peak = peaks.iter().cloned().fold(0.0f32, f32::max).max(0.05);
    let mid = height / 2;

    for col in 0..width {
        let start = col * peaks.len() / width;
        let end = ((col + 1) * peaks.len() / width).max(start + 1);
        let bucket_peak = peaks[start..end.min(peaks.len())]
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        let amp = (bucket_peak / max_peak).clamp(0.0, 1.0);
        let bar_height = ((amp * height as f32).ceil() as usize).clamp(1, height);
        let top = mid.saturating_sub(bar_height / 2);
        let bottom = (top + bar_height).min(height);

        for (r, row) in rows.iter_mut().enumerate() {
            if r >= top && r < bottom {
                row.push('█');
            } else {
                row.push(' ');
            }
        }
    }
    rows
}

fn level_char(level: f32) -> char {
    const BARS: [char; 8] = [' ', '▁', '▂', '▄', '▅', '▆', '▇', '█'];
    let idx = ((level.clamp(0.0, 1.0)) * (BARS.len() - 1) as f32).round() as usize;
    BARS[idx]
}

fn spinner_frame() -> &'static str {
    let step = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() / 100)
        .unwrap_or(0)) as usize;
    SPINNER[step % SPINNER.len()]
}

fn type_tag(att: &Attachment) -> &'static str {
    if att.is_image() {
        "[IMG]"
    } else if att.is_audio() {
        "[AUD]"
    } else {
        "[FILE]"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max > 1 {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{}…", cut)
    } else {
        "…".to_string()
    }
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1}M", b / MB)
    } else if b >= KB {
        format!("{:.0}K", b / KB)
    } else {
        format!("{}B", bytes)
    }
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn wrap_plain(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for token in text.split_inclusive(char::is_whitespace) {
        if current.width() + token.width() > width && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.push_str(token.trim_end_matches('\n'));
    }
    if !current.is_empty() || out.is_empty() {
        out.push(current);
    }
    out
}

fn wrap_error(text: &str, width: usize) -> String {
    wrap_plain(text, width.max(8)).join("\n")
}

fn centered_top(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y + area.height / 3,
        area.width,
        area.height / 2,
    )
}

fn format_datetime_short(s: &str) -> String {
    if s.len() >= 16 {
        if let (Some(date), Some(time)) = (s.get(..10), s.get(11..16)) {
            return format!("{} {}", date, time);
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_rows_have_one_char_per_column() {
        let peaks: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        for width in [1usize, 2, 7, 20, 55] {
            for row in waveform_rows(&peaks, width, 3) {
                assert_eq!(row.chars().count(), width);
            }
        }
    }

    #[test]
    fn playhead_split_at_every_column_is_char_safe() {
        let peaks: Vec<f32> = vec![0.9, 0.1, 0.5, 1.0, 0.3];
        for width in [1usize, 3, 8, 40] {
            for row in waveform_rows(&peaks, width, 3) {
                for col in 0..=row.chars().count() {
                    let played: String = row.chars().take(col).collect();
                    let remaining: String = row.chars().skip(col).collect();
                    assert_eq!(played.chars().count() + remaining.chars().count(), width);
                }
            }
        }
    }

    #[test]
    fn empty_peaks_produce_blank_rows() {
        for row in waveform_rows(&[], 10, 3) {
            assert_eq!(row.chars().count(), 10);
            assert!(row.chars().all(|c| c == ' '));
        }
    }
}
