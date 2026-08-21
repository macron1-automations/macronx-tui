use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::{App, AssetState, ZoomLevel};

/// Minimum terminal area (in cells) below which the viewer refuses to render.
const MIN_AREA: u16 = 8;

pub fn render(f: &mut Frame, app: &mut App) {
    let Some(viewer) = app.viewer.as_ref() else {
        return;
    };
    let attachment_id = viewer.attachment_id;
    let zoom = viewer.zoom;
    let pan_u = viewer.pan_u;
    let pan_v = viewer.pan_v;

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

    let needs_rebuild = app.viewer_needs_rebuild(zoom, pan_u, pan_v, inner.width, inner.height);
    if needs_rebuild {
        rebuild_protocol(app, zoom, pan_u, pan_v, inner);
    }

    if let Some(protocol) = app.viewer_protocol_mut() {
        let image = StatefulImage::new(None).resize(Resize::Fit(None));
        f.render_stateful_widget(image, inner, protocol);
    }

    // Footer hints drawn on top of the image area.
    let footer = Line::from(vec![
        Span::styled(" [+/-] ", Style::default().fg(Color::Cyan)),
        Span::raw("Zoom  "),
        Span::styled("[hjkl] ", Style::default().fg(Color::Cyan)),
        Span::raw("Pan  "),
        Span::styled("[f/Esc] ", Style::default().fg(Color::Cyan)),
        Span::raw("Close"),
    ]);
    let footer_area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    f.render_widget(
        Paragraph::new(footer).style(Style::default().add_modifier(Modifier::BOLD)),
        footer_area,
    );

    // Zoom label in the top-right corner.
    let label = format!(" {} ", zoom_label(zoom));
    let label_area_w = label.len() as u16 + 2;
    let label_area = Rect::new(
        area.right().saturating_sub(label_area_w).saturating_sub(1),
        area.y + 1,
        label_area_w.min(area.width),
        1,
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))),
        label_area,
    );
}

fn zoom_label(zoom: ZoomLevel) -> &'static str {
    zoom.label()
}

fn rebuild_protocol(app: &mut App, zoom: ZoomLevel, pan_u: f32, pan_v: f32, inner: Rect) {
    let Some(id) = app.viewer.as_ref().map(|v| v.attachment_id) else {
        return;
    };

    // Grab what we need from the asset under an immutable borrow.
    let cropped = {
        let Some(AssetState::Ready(asset)) = app.assets.get(&id) else {
            return;
        };
        crop_for_area(
            &asset.image,
            zoom,
            pan_u,
            pan_v,
            inner,
            app.picker.font_size,
        )
    };

    let mut picker = app.take_picker();
    let protocol = picker.new_resize_protocol(cropped);
    app.return_picker(picker);

    app.store_viewer_protocol(zoom, pan_u, pan_v, inner.width, inner.height, protocol);
}

/// Crop the source image to the region that should fill `area` at the given
/// zoom level. The cropped image's aspect ratio matches the area's pixel
/// aspect ratio, so rendering it with `Resize::Fit` fills the area exactly.
fn crop_for_area(
    image: &image::DynamicImage,
    zoom: ZoomLevel,
    pan_u: f32,
    pan_v: f32,
    area: Rect,
    font_size: (u16, u16),
) -> image::DynamicImage {
    let img_w = image.width() as f32;
    let img_h = image.height() as f32;

    let cell_px_w = font_size.0.max(1) as f32;
    let cell_px_h = font_size.1.max(1) as f32;
    let area_px_w = (area.width.max(1) as f32) * cell_px_w;
    let area_px_h = (area.height.max(1) as f32) * cell_px_h;

    let fit_scale = (area_px_w / img_w).min(area_px_h / img_h);
    let scale = (fit_scale * zoom.factor()).max(f32::EPSILON);

    let vis_w = (area_px_w / scale).min(img_w);
    let vis_h = (area_px_h / scale).min(img_h);

    let center_x = pan_u.clamp(0.0, 1.0) * img_w;
    let center_y = pan_v.clamp(0.0, 1.0) * img_h;

    let x0 = ((center_x - vis_w / 2.0).round() as u32).clamp(0, image.width() - 1);
    let y0 = ((center_y - vis_h / 2.0).round() as u32).clamp(0, image.height() - 1);
    let w = (vis_w as u32).clamp(1, image.width() - x0);
    let h = (vis_h as u32).clamp(1, image.height() - y0);

    image.crop_imm(x0, y0, w, h)
}
