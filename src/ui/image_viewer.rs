use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::{App, AssetState};

/// Minimum terminal area (in cells) below which the viewer refuses to render.
const MIN_AREA: u16 = 8;

pub fn render(f: &mut Frame, app: &mut App) {
    let Some(viewer) = app.viewer.as_ref() else {
        return;
    };
    let attachment_id = viewer.attachment_id;

    let area = f.area();
    if area.width < MIN_AREA || area.height < MIN_AREA {
        return;
    }

    f.render_widget(Clear, area);

    let filename = app
        .attachments()
        .iter()
        .find(|a| a.id == attachment_id)
        .map(|a| a.filename.clone())
        .unwrap_or_default();

    let block = Block::default()
        .title(format!(" {} ", filename))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    if app.viewer_needs_rebuild(inner.width, inner.height) {
        request_encode(app, inner);
    }

    if let Some(protocol) = app.viewer_protocol_mut() {
        let image = StatefulImage::new(None).resize(Resize::Fit(None));
        f.render_stateful_widget(image, inner, protocol);
    }

    // Footer hints drawn on top of the image area.
    let footer = Line::from(vec![
        Span::styled(" [o] ", Style::default().fg(Color::Cyan)),
        Span::raw("Open  "),
        Span::styled("[f/Esc] ", Style::default().fg(Color::Cyan)),
        Span::raw("Close"),
    ]);
    let footer_area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    f.render_widget(
        Paragraph::new(footer).style(Style::default().add_modifier(Modifier::BOLD)),
        footer_area,
    );
}

fn request_encode(app: &mut App, inner: Rect) {
    let Some(id) = app.viewer.as_ref().map(|v| v.attachment_id) else {
        return;
    };

    let prepared = {
        let Some(AssetState::Ready(asset)) = app.assets.get(&id) else {
            return;
        };
        shrink_for_display(asset.image.clone(), inner, app.picker.font_size)
    };

    app.begin_viewer_encode(inner.width, inner.height, prepared);
}

/// Pre-scale the image to the display pixel size before encoding. The protocol
/// rescales to cell dimensions anyway; feeding it a full-resolution photo only
/// wastes resize/encode time and degrades quality (Nearest filter).
fn shrink_for_display(
    img: image::DynamicImage,
    area: Rect,
    font_size: (u16, u16),
) -> image::DynamicImage {
    let max_w = ((area.width.max(1) as f32) * font_size.0.max(1) as f32) as u32;
    let max_h = ((area.height.max(1) as f32) * font_size.1.max(1) as f32) as u32;
    if img.width() <= max_w && img.height() <= max_h {
        return img;
    }
    img.resize(
        max_w.max(1),
        max_h.max(1),
        image::imageops::FilterType::Triangle,
    )
}
