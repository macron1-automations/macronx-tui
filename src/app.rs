use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use crossterm::event::KeyCode;
use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use tui_scrollview::ScrollViewState;

use crate::api::ApiClient;
use crate::audio::AudioPlayer;
use crate::config::Config;
use crate::models::{Attachment, Inbox, Tag};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    InboxList,
    InboxShow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarTab {
    Metadata,
    Attachments,
}

impl SidebarTab {
    pub fn next(self) -> Self {
        match self {
            SidebarTab::Metadata => SidebarTab::Attachments,
            SidebarTab::Attachments => SidebarTab::Metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Body,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomLevel {
    Fit,
    X1,
    X2,
    X4,
}

impl ZoomLevel {
    pub fn factor(self) -> f32 {
        match self {
            ZoomLevel::Fit => 1.0,
            ZoomLevel::X1 => 1.0,
            ZoomLevel::X2 => 2.0,
            ZoomLevel::X4 => 4.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ZoomLevel::Fit => "fit",
            ZoomLevel::X1 => "100%",
            ZoomLevel::X2 => "200%",
            ZoomLevel::X4 => "400%",
        }
    }

    pub fn zoom_in(self) -> Self {
        match self {
            ZoomLevel::Fit => ZoomLevel::X1,
            ZoomLevel::X1 => ZoomLevel::X2,
            ZoomLevel::X2 | ZoomLevel::X4 => ZoomLevel::X4,
        }
    }

    pub fn zoom_out(self) -> Self {
        match self {
            ZoomLevel::Fit | ZoomLevel::X1 => ZoomLevel::Fit,
            ZoomLevel::X2 => ZoomLevel::X1,
            ZoomLevel::X4 => ZoomLevel::X2,
        }
    }

    pub fn is_fit(self) -> bool {
        self == ZoomLevel::Fit
    }
}

pub struct ImageAsset {
    /// Protocol sized for the sidebar preview.
    pub protocol: Box<dyn StatefulProtocol>,
    /// Original decoded image, used by the fullscreen viewer for zoom/pan crops.
    pub image: DynamicImage,
}

pub enum AssetState {
    Loading,
    Ready(ImageAsset),
    AudioReady,
    Failed(String),
}

pub struct ImageViewer {
    pub attachment_id: u64,
    pub zoom: ZoomLevel,
    /// Crop center in normalized image coordinates (0.0..=1.0).
    pub pan_u: f32,
    pub pan_v: f32,
}

struct ViewerRender {
    zoom: ZoomLevel,
    pan_u: u16, // quantized for change detection
    pan_v: u16,
    width: u16,
    height: u16,
    protocol: Box<dyn StatefulProtocol>,
}

impl ImageViewer {
    fn new(attachment_id: u64) -> Self {
        Self {
            attachment_id,
            zoom: ZoomLevel::Fit,
            pan_u: 0.5,
            pan_v: 0.5,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub inboxes: Vec<Inbox>,
    pub tags: Vec<Tag>,
    pub selected_tag: Option<usize>, // index into `tags`, None = "All"
    pub filtered: Vec<usize>,        // indices into `inboxes` matching the active tag
    pub selected: usize,
    pub current_inbox: Option<Inbox>,
    pub body_scroll: ScrollViewState,
    pub status: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub client: ApiClient,

    // Sidebar
    pub sidebar_tab: SidebarTab,
    pub focus: Focus,
    pub selected_attachment: usize,

    // Attachments
    pub picker: Picker,
    pub assets: HashMap<u64, AssetState>,
    pub viewer: Option<ImageViewer>,
    pub audio: Option<AudioPlayer>,

    pending_downloads: HashSet<u64>,
    download_tx: std::sync::mpsc::Sender<(u64, Result<Vec<u8>, String>)>,
    download_rx: Receiver<(u64, Result<Vec<u8>, String>)>,
    audio_bytes: HashMap<u64, Vec<u8>>,
    pending_play: Option<u64>,
    viewer_pending: Option<u64>,
    viewer_render: Option<ViewerRender>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let client = ApiClient::new(&config.base_url, &config.api_token);
        let (download_tx, download_rx) = channel();
        Self {
            screen: Screen::InboxList,
            inboxes: Vec::new(),
            tags: Vec::new(),
            selected_tag: None,
            filtered: Vec::new(),
            selected: 0,
            current_inbox: None,
            body_scroll: ScrollViewState::new(),
            status: None,
            should_quit: false,
            client,
            sidebar_tab: SidebarTab::Metadata,
            focus: Focus::Body,
            selected_attachment: 0,
            picker: Picker::new((10, 20)),
            assets: HashMap::new(),
            viewer: None,
            audio: None,
            pending_downloads: HashSet::new(),
            download_tx,
            download_rx,
            audio_bytes: HashMap::new(),
            pending_play: None,
            viewer_pending: None,
            viewer_render: None,
        }
    }

    pub fn set_picker(&mut self, picker: Picker) {
        self.picker = picker;
    }

    /// Temporarily hand out the picker (protocol construction needs `&mut`).
    /// Always pair with `return_picker`.
    pub fn take_picker(&mut self) -> Picker {
        std::mem::replace(&mut self.picker, Picker::new((10, 20)))
    }

    pub fn return_picker(&mut self, picker: Picker) {
        self.picker = picker;
    }

    pub fn attachments(&self) -> &[Attachment] {
        match &self.current_inbox {
            Some(inbox) => &inbox.attachments,
            None => &[],
        }
    }

    pub fn asset_state(&self, id: u64) -> Option<&AssetState> {
        self.assets.get(&id)
    }

    pub fn selected_attachment(&self) -> Option<&Attachment> {
        let attachments = self.attachments();
        if attachments.is_empty() {
            return None;
        }
        Some(&attachments[self.selected_attachment.min(attachments.len() - 1)])
    }

    pub fn load_inboxes(&mut self) {
        match self.client.list_inboxes() {
            Ok(inboxes) => {
                self.inboxes = inboxes;
                self.refresh_tags();
                self.status = None;
            }
            Err(e) => {
                self.status = Some((format!("Error loading inboxes: {}", e), true));
            }
        }
    }

    pub fn open_selected_inbox(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let id = self.inboxes[self.filtered[self.selected]].id;
        match self.client.get_inbox(id) {
            Ok(inbox) => {
                self.audio = None;
                self.viewer = None;
                self.viewer_render = None;
                self.viewer_pending = None;
                self.pending_play = None;
                self.current_inbox = Some(inbox);
                self.body_scroll.scroll_to_top();
                self.sidebar_tab = SidebarTab::Metadata;
                self.focus = Focus::Body;
                self.selected_attachment = 0;
                self.screen = Screen::InboxShow;
                self.status = None;
            }
            Err(e) => {
                self.status = Some((format!("Error loading inbox: {}", e), true));
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        // Clear non-error status on any keypress
        if let Some((_, false)) = &self.status {
            self.status = None;
        }

        match self.screen {
            Screen::InboxList => self.handle_list_key(key),
            Screen::InboxShow => self.handle_show_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.filtered.is_empty() {
                    self.selected = (self.selected + 1).min(self.filtered.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.filtered.is_empty() {
                    self.selected = self.filtered.len() - 1;
                }
            }
            KeyCode::Char('J') => {
                self.cycle_tag(true);
            }
            KeyCode::Char('K') => {
                self.cycle_tag(false);
            }
            KeyCode::Enter => {
                self.open_selected_inbox();
            }
            KeyCode::Char('r') => {
                self.load_inboxes();
            }
            _ => {}
        }
    }

    fn handle_show_key(&mut self, key: KeyCode) {
        // The fullscreen viewer captures all keys while open.
        if self.viewer.is_some() {
            match key {
                KeyCode::Esc | KeyCode::Char('f') | KeyCode::Char('q') | KeyCode::Backspace => {
                    self.close_viewer();
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.viewer_zoom(true);
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.viewer_zoom(false);
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.viewer_pan(-0.15, 0.0);
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.viewer_pan(0.15, 0.0);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.viewer_pan(0.0, -0.15);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.viewer_pan(0.0, 0.15);
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.audio = None;
                self.screen = Screen::InboxList;
                self.status = None;
            }
            KeyCode::Char('f') => {
                self.open_image_viewer();
            }
            KeyCode::Char(' ') => {
                self.toggle_audio();
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(audio) = &self.audio {
                    audio.set_volume(audio.volume() + 0.1);
                }
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(audio) = &self.audio {
                    audio.set_volume(audio.volume() - 0.1);
                }
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Body => Focus::Sidebar,
                    Focus::Sidebar => Focus::Body,
                };
                self.on_tab_entered();
            }
            KeyCode::BackTab => {
                self.sidebar_tab = self.sidebar_tab.next();
                self.on_tab_entered();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.focus == Focus::Sidebar && self.sidebar_tab == SidebarTab::Attachments {
                    self.select_next_attachment();
                } else {
                    self.body_scroll.scroll_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.focus == Focus::Sidebar && self.sidebar_tab == SidebarTab::Attachments {
                    self.select_prev_attachment();
                } else {
                    self.body_scroll.scroll_up();
                }
            }
            KeyCode::Left => {
                if let Some(audio) = &self.audio {
                    audio.seek_by(std::time::Duration::from_secs(5));
                } else if self.focus == Focus::Sidebar {
                    self.sidebar_tab = self.sidebar_tab.next();
                    self.on_tab_entered();
                }
            }
            KeyCode::Right => {
                if let Some(audio) = &self.audio {
                    audio.seek_by(std::time::Duration::from_secs(5));
                } else if self.focus == Focus::Sidebar {
                    self.sidebar_tab = self.sidebar_tab.next();
                    self.on_tab_entered();
                }
            }
            KeyCode::Char('h') => {
                if self.focus == Focus::Sidebar {
                    self.sidebar_tab = self.sidebar_tab.next();
                    self.on_tab_entered();
                }
            }
            KeyCode::Char('l') => {
                if self.focus == Focus::Sidebar {
                    self.sidebar_tab = self.sidebar_tab.next();
                    self.on_tab_entered();
                }
            }
            KeyCode::PageDown => {
                self.body_scroll.scroll_page_down();
            }
            KeyCode::PageUp => {
                self.body_scroll.scroll_page_up();
            }
            KeyCode::Char('g') => {
                self.body_scroll.scroll_to_top();
            }
            KeyCode::Char('G') => {
                self.body_scroll.scroll_to_bottom();
            }
            _ => {}
        }
    }

    fn select_next_attachment(&mut self) {
        let count = self.attachments().len();
        if count == 0 {
            return;
        }
        self.selected_attachment = (self.selected_attachment + 1).min(count - 1);
        self.on_attachment_selected();
    }

    fn select_prev_attachment(&mut self) {
        self.selected_attachment = self.selected_attachment.saturating_sub(1);
        self.on_attachment_selected();
    }

    fn on_attachment_selected(&mut self) {
        if let Some(att) = self.selected_attachment() {
            self.request_attachment(att.id);
        }
    }

    /// Kick off lazy downloads when the Attachments tab becomes visible.
    fn on_tab_entered(&mut self) {
        if self.sidebar_tab == SidebarTab::Attachments {
            self.on_attachment_selected();
        }
    }

    pub fn request_attachment(&mut self, id: u64) {
        if self.assets.contains_key(&id) || self.pending_downloads.contains(&id) {
            return;
        }
        let url = match self.attachments().iter().find(|a| a.id == id) {
            Some(att) if !att.url.is_empty() => att.url.clone(),
            _ => return,
        };

        self.assets.insert(id, AssetState::Loading);
        self.pending_downloads.insert(id);

        let client = self.client.clone();
        let tx = self.download_tx.clone();
        std::thread::spawn(move || {
            let result = client.fetch_bytes(&url).map_err(|e| e.to_string());
            let _ = tx.send((id, result));
        });
    }

    pub fn poll_downloads(&mut self) {
        loop {
            match self.download_rx.try_recv() {
                Ok((id, result)) => {
                    self.pending_downloads.remove(&id);
                    match result {
                        Ok(bytes) => self.on_download_complete(id, bytes),
                        Err(err) => {
                            self.assets.insert(id, AssetState::Failed(err));
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn on_download_complete(&mut self, id: u64, bytes: Vec<u8>) {
        let kind = self
            .attachments()
            .iter()
            .find(|a| a.id == id)
            .map(|a| (a.is_image(), a.is_audio()));

        match kind {
            Some((true, _)) => match image::load_from_memory(&bytes) {
                Ok(decoded) => {
                    let protocol = self.picker.new_resize_protocol(decoded.clone());
                    self.assets.insert(
                        id,
                        AssetState::Ready(ImageAsset {
                            protocol,
                            image: decoded,
                        }),
                    );
                }
                Err(e) => {
                    self.assets
                        .insert(id, AssetState::Failed(format!("Decode failed: {}", e)));
                }
            },
            Some((false, true)) => {
                self.audio_bytes.insert(id, bytes);
                self.assets.insert(id, AssetState::AudioReady);
                if self.pending_play == Some(id) && self.audio.is_none() {
                    self.start_audio(id);
                }
            }
            _ => {}
        }

        if self.viewer_pending == Some(id) {
            if matches!(self.assets.get(&id), Some(AssetState::Ready(_))) {
                self.viewer = Some(ImageViewer::new(id));
                self.viewer_render = None;
            }
            self.viewer_pending = None;
        }
    }

    pub fn open_image_viewer(&mut self) {
        let target = match self.selected_attachment() {
            Some(att) if att.is_image() => Some(att.id),
            Some(_) => self.first_image_id(),
            None => self.first_image_id(),
        };

        let Some(id) = target else {
            self.status = Some(("No image attachments".into(), false));
            return;
        };

        match self.assets.get(&id) {
            Some(AssetState::Ready(_)) => {
                self.viewer = Some(ImageViewer::new(id));
                self.viewer_render = None;
            }
            Some(AssetState::Failed(err)) => {
                self.status = Some((err.clone(), true));
            }
            _ => {
                self.request_attachment(id);
                self.viewer_pending = Some(id);
                self.status = Some(("Loading image…".into(), false));
            }
        }
    }

    pub fn close_viewer(&mut self) {
        self.viewer = None;
        self.viewer_render = None;
        self.viewer_pending = None;
    }

    fn first_image_id(&self) -> Option<u64> {
        self.attachments()
            .iter()
            .find(|a| a.is_image())
            .map(|a| a.id)
    }

    fn first_audio_id(&self) -> Option<u64> {
        self.attachments()
            .iter()
            .find(|a| a.is_audio())
            .map(|a| a.id)
    }

    fn viewer_zoom(&mut self, inward: bool) {
        if let Some(viewer) = &mut self.viewer {
            viewer.zoom = if inward {
                viewer.zoom.zoom_in()
            } else {
                viewer.zoom.zoom_out()
            };
            self.viewer_render = None;
        }
    }

    fn viewer_pan(&mut self, du: f32, dv: f32) {
        if let Some(viewer) = &mut self.viewer {
            if viewer.zoom.is_fit() {
                return;
            }
            viewer.pan_u = (viewer.pan_u + du).clamp(0.0, 1.0);
            viewer.pan_v = (viewer.pan_v + dv).clamp(0.0, 1.0);
            self.viewer_render = None;
        }
    }

    pub fn toggle_audio(&mut self) {
        if let Some(audio) = &self.audio {
            audio.toggle();
            return;
        }
        self.begin_audio_playback();
    }

    fn begin_audio_playback(&mut self) {
        let target = match self.selected_attachment() {
            Some(att) if att.is_audio() => Some(att.id),
            Some(_) => self.first_audio_id(),
            None => self.first_audio_id(),
        };

        let Some(id) = target else {
            self.status = Some(("No audio attachments".into(), false));
            return;
        };

        if self.audio_bytes.contains_key(&id) {
            self.start_audio(id);
        } else {
            self.request_attachment(id);
            self.pending_play = Some(id);
            self.status = Some(("Loading audio…".into(), false));
        }
    }

    fn start_audio(&mut self, id: u64) {
        let Some(bytes) = self.audio_bytes.get(&id).cloned() else {
            return;
        };
        match AudioPlayer::load(id, bytes) {
            Ok(player) => {
                self.audio = Some(player);
                self.pending_play = None;
            }
            Err(e) => {
                self.pending_play = None;
                self.status = Some((format!("Audio playback failed: {}", e), true));
            }
        }
    }

    pub fn needs_fast_poll(&self) -> bool {
        if !self.pending_downloads.is_empty() || self.pending_play.is_some()
            || self.viewer_pending.is_some()
        {
            return true;
        }
        if let Some(audio) = &self.audio {
            return !audio.is_paused();
        }
        false
    }

    pub fn viewer_needs_rebuild(
        &mut self,
        zoom: ZoomLevel,
        pan_u: f32,
        pan_v: f32,
        width: u16,
        height: u16,
    ) -> bool {
        let stale = match &self.viewer_render {
            Some(r) => {
                r.zoom != zoom
                    || (r.pan_u as f32 / 1000.0 - pan_u).abs() > 0.001
                    || (r.pan_v as f32 / 1000.0 - pan_v).abs() > 0.001
                    || r.width != width
                    || r.height != height
            }
            None => true,
        };
        if stale {
            self.viewer_render = None;
        }
        stale
    }

    pub fn store_viewer_protocol(
        &mut self,
        zoom: ZoomLevel,
        pan_u: f32,
        pan_v: f32,
        width: u16,
        height: u16,
        protocol: Box<dyn StatefulProtocol>,
    ) {
        self.viewer_render = Some(ViewerRender {
            zoom,
            pan_u: (pan_u * 1000.0) as u16,
            pan_v: (pan_v * 1000.0) as u16,
            width,
            height,
            protocol,
        });
    }

    pub fn viewer_protocol_mut(&mut self) -> Option<&mut Box<dyn StatefulProtocol>> {
        self.viewer_render.as_mut().map(|r| &mut r.protocol)
    }

    fn refresh_tags(&mut self) {
        let selected = self.selected_tag_name().map(|s| s.to_string());

        let mut tags = self.client.list_tags().unwrap_or_default();

        // Union in any tags seen on inboxes that are not yet listed.
        let mut names: HashSet<String> = tags.iter().map(|t| t.name.to_lowercase()).collect();
        for inbox in &self.inboxes {
            if let Some(name) = inbox.tag.as_deref() {
                if !name.trim().is_empty() && names.insert(name.to_lowercase()) {
                    tags.push(Tag {
                        name: name.to_string(),
                        color: None,
                    });
                }
            }
        }

        tags.sort_by_key(|t| t.name.to_lowercase());
        self.tags = tags;

        self.selected_tag = selected.as_deref().and_then(|s| {
            self.tags
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(s))
        });
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let tag = self.selected_tag_name();
        self.filtered = self
            .inboxes
            .iter()
            .enumerate()
            .filter(|(_, inbox)| match tag {
                None => true,
                Some(t) => inbox
                    .tag
                    .as_deref()
                    .map(|name| name.eq_ignore_ascii_case(t))
                    .unwrap_or(false),
            })
            .map(|(idx, _)| idx)
            .collect();

        if !self.filtered.is_empty() && self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    fn cycle_tag(&mut self, forward: bool) {
        let total = self.tags.len() + 1; // "All" plus each tag
        let current = self.tag_offset();
        let next = if forward {
            (current + 1) % total
        } else {
            (current + total - 1) % total
        };

        if next == 0 {
            self.selected_tag = None;
        } else {
            self.selected_tag = Some(next - 1);
        }
        self.selected = 0;
        self.apply_filter();
    }

    pub fn tag_offset(&self) -> usize {
        self.selected_tag.map_or(0, |i| i + 1)
    }

    pub fn selected_tag_name(&self) -> Option<&str> {
        self.selected_tag
            .and_then(|i| self.tags.get(i))
            .map(|t| t.name.as_str())
    }
}
