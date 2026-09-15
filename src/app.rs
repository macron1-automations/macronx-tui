use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

use crossterm::event::KeyCode;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
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
pub enum IndexView {
    Processed,
    Archived,
}

impl IndexView {
    pub fn toggle(self) -> Self {
        match self {
            IndexView::Processed => IndexView::Archived,
            IndexView::Archived => IndexView::Processed,
        }
    }

    pub fn matches(&self, inbox: &Inbox) -> bool {
        match self {
            IndexView::Processed => inbox.processed && !inbox.archived,
            IndexView::Archived => inbox.archived,
        }
    }
}

pub struct ImageAsset {
    /// Protocol sized for the sidebar preview.
    pub protocol: Box<dyn StatefulProtocol>,
    /// Original decoded image, rendered fit-to-screen by the fullscreen viewer.
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
}

struct ViewerRender {
    width: u16,
    height: u16,
    protocol: Box<dyn StatefulProtocol>,
}

/// Parameters of an in-flight viewer encode job on the worker thread.
struct ViewerEncode {
    gen: u64,
    width: u16,
    height: u16,
}

struct ViewerEncodeJob {
    gen: u64,
    protocol: Box<dyn StatefulProtocol>,
    area: Rect,
}

impl ImageViewer {
    fn new(attachment_id: u64) -> Self {
        Self { attachment_id }
    }
}

pub struct App {
    pub screen: Screen,
    pub inboxes: Vec<Inbox>,
    pub tags: Vec<Tag>,
    pub selected_tag: Option<usize>, // index into `tags`, None = "All"
    pub index_view: IndexView,
    api_tags: Vec<Tag>,
    pub filtered: Vec<usize>,        // indices into `inboxes` matching the active view + tag
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
    raw_bytes: HashMap<u64, Vec<u8>>,
    pending_play: Option<u64>,
    viewer_pending: Option<u64>,
    viewer_render: Option<ViewerRender>,
    viewer_encode: Option<ViewerEncode>,
    viewer_gen: u64,
    encode_tx: Sender<ViewerEncodeJob>,
    encode_rx: Receiver<(u64, Box<dyn StatefulProtocol>)>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let client = ApiClient::new(&config.base_url, &config.api_token);
        let (download_tx, download_rx) = channel();
        let (encode_tx, job_rx) = channel::<ViewerEncodeJob>();
        let (done_tx, encode_rx) = channel();

        // Background worker: resize+encode viewer images off the UI thread.
        std::thread::spawn(move || {
            while let Ok(mut job) = job_rx.recv() {
                job.protocol
                    .resize_encode(&Resize::Fit(None), None, job.area);
                let _ = done_tx.send((job.gen, job.protocol));
            }
        });

        Self {
            screen: Screen::InboxList,
            inboxes: Vec::new(),
            tags: Vec::new(),
            selected_tag: None,
            index_view: IndexView::Processed,
            api_tags: Vec::new(),
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
            raw_bytes: HashMap::new(),
            pending_play: None,
            viewer_pending: None,
            viewer_render: None,
            viewer_encode: None,
            viewer_gen: 0,
            encode_tx,
            encode_rx,
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
            KeyCode::Char('t') => {
                self.toggle_index_view();
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
                KeyCode::Char('o') => {
                    let id = self.viewer.as_ref().map(|v| v.attachment_id);
                    if let Some(id) = id {
                        self.open_externally(id);
                    }
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
            KeyCode::Char('o') => {
                if let Some(att) = self.selected_attachment() {
                    let id = att.id;
                    self.open_externally(id);
                }
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
                    audio.seek_back(std::time::Duration::from_secs(5));
                } else if self.focus == Focus::Sidebar {
                    self.sidebar_tab = self.sidebar_tab.next();
                    self.on_tab_entered();
                }
            }
            KeyCode::Right => {
                if let Some(audio) = &self.audio {
                    audio.seek_forward(std::time::Duration::from_secs(5));
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
                    self.raw_bytes.insert(id, bytes);
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
                self.raw_bytes.insert(id, bytes);
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
                self.viewer_encode = None;
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
                self.viewer_encode = None;
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
        self.viewer_encode = None;
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

        if self.raw_bytes.contains_key(&id) {
            self.start_audio(id);
        } else {
            self.request_attachment(id);
            self.pending_play = Some(id);
            self.status = Some(("Loading audio…".into(), false));
        }
    }

    fn start_audio(&mut self, id: u64) {
        let Some(bytes) = self.raw_bytes.get(&id).cloned() else {
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

    /// Write the attachment's original bytes to a temp file and open it with
    /// the system's default application for that file type.
    fn open_externally(&mut self, id: u64) {
        let Some(att) = self.attachments().iter().find(|a| a.id == id) else {
            return;
        };
        let filename = att.filename.clone();
        let Some(bytes) = self.raw_bytes.get(&id) else {
            self.status = Some(("Attachment not downloaded yet".into(), true));
            return;
        };

        let name = sanitize_filename(&filename);
        let dir = std::env::temp_dir().join("macronx-tui");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = Some((format!("Open failed: {}", e), true));
            return;
        }
        let path = dir.join(format!("{}-{}", id, name));
        if let Err(e) = std::fs::write(&path, bytes) {
            self.status = Some((format!("Open failed: {}", e), true));
            return;
        }

        #[cfg(target_os = "macos")]
        let mut cmd = {
            let mut c = std::process::Command::new("open");
            c.arg(&path);
            c
        };
        #[cfg(not(target_os = "macos"))]
        let mut cmd = {
            let mut c = std::process::Command::new("xdg-open");
            c.arg(&path);
            c
        };
        match cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => self.status = Some((format!("Opened {}", filename), false)),
            Err(e) => self.status = Some((format!("Open failed: {}", e), true)),
        }
    }

    pub fn needs_fast_poll(&self) -> bool {
        if !self.pending_downloads.is_empty()
            || self.pending_play.is_some()
            || self.viewer_pending.is_some()
            || self.viewer_encode.is_some()
        {
            return true;
        }
        if let Some(audio) = &self.audio {
            return !audio.is_paused();
        }
        false
    }

    pub fn viewer_needs_rebuild(&mut self, width: u16, height: u16) -> bool {
        if let Some(r) = &self.viewer_render {
            if r.width == width && r.height == height {
                return false;
            }
        }
        if let Some(e) = &self.viewer_encode {
            if e.width == width && e.height == height {
                return false;
            }
        }
        self.viewer_render = None;
        true
    }

    /// Hand a freshly built protocol to the worker thread for resize+encode.
    pub fn begin_viewer_encode(&mut self, width: u16, height: u16, image: DynamicImage) {
        self.viewer_gen += 1;
        let gen = self.viewer_gen;
        let mut picker = self.take_picker();
        let protocol = picker.new_resize_protocol(image);
        self.return_picker(picker);
        let _ = self.encode_tx.send(ViewerEncodeJob {
            gen,
            protocol,
            area: Rect::new(0, 0, width, height),
        });
        self.viewer_encode = Some(ViewerEncode { gen, width, height });
    }

    /// Collect finished encodes from the worker, dropping stale generations.
    pub fn poll_viewer_encode(&mut self) {
        while let Ok((gen, protocol)) = self.encode_rx.try_recv() {
            let current = self.viewer_encode.as_ref().is_some_and(|e| e.gen == gen);
            if current {
                if let Some(e) = self.viewer_encode.take() {
                    self.viewer_render = Some(ViewerRender {
                        width: e.width,
                        height: e.height,
                        protocol,
                    });
                }
            }
        }
    }

    pub fn viewer_protocol_mut(&mut self) -> Option<&mut Box<dyn StatefulProtocol>> {
        self.viewer_render.as_mut().map(|r| &mut r.protocol)
    }

    fn refresh_tags(&mut self) {
        self.api_tags = self.client.list_tags().unwrap_or_default();
        self.rebuild_tags();
    }

    fn rebuild_tags(&mut self) {
        let selected = self.selected_tag_name().map(|s| s.to_string());

        let mut tags = self.api_tags.clone();

        // Union in any tags seen on inboxes that are not yet listed.
        let mut names: HashSet<String> = tags.iter().map(|t| t.name.to_lowercase()).collect();
        for inbox in &self.inboxes {
            if !self.index_view.matches(inbox) {
                continue;
            }
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

    fn toggle_index_view(&mut self) {
        self.index_view = self.index_view.toggle();
        self.selected = 0;
        self.rebuild_tags();
    }

    fn apply_filter(&mut self) {
        let tag = self.selected_tag_name();
        self.filtered = self
            .inboxes
            .iter()
            .enumerate()
            .filter(|(_, inbox)| {
                self.index_view.matches(inbox)
                    && match tag {
                        None => true,
                        Some(t) => inbox
                            .tag
                            .as_deref()
                            .map(|name| name.eq_ignore_ascii_case(t))
                            .unwrap_or(false),
                    }
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

/// Reduce an attachment filename to a safe single path component.
fn sanitize_filename(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim()
        .to_string();
    if base.is_empty() {
        "attachment".to_string()
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbox(id: u64, processed: bool, archived: bool, tag: Option<&str>) -> Inbox {
        Inbox {
            id,
            name: format!("inbox-{}", id),
            source: "test".into(),
            tag: tag.map(|s| s.to_string()),
            summary: None,
            body: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            attachments: Vec::new(),
            processed,
            archived,
        }
    }

    fn test_app() -> App {
        App::new(Config {
            base_url: "http://localhost:5000".into(),
            api_token: "test-token".into(),
        })
    }

    #[test]
    fn default_view_is_processed() {
        let app = test_app();
        assert_eq!(app.index_view, IndexView::Processed);
    }

    #[test]
    fn processed_view_filters_processed_unarchived() {
        let mut app = test_app();
        app.inboxes = vec![
            inbox(1, true, false, None),
            inbox(2, false, false, None),
            inbox(3, true, true, None),
            inbox(4, false, true, None),
        ];
        app.api_tags = Vec::new();
        app.rebuild_tags();

        assert_eq!(app.filtered, vec![0]);
    }

    #[test]
    fn archived_view_filters_archived_items() {
        let mut app = test_app();
        app.inboxes = vec![
            inbox(1, true, false, None),
            inbox(2, false, false, None),
            inbox(3, true, true, None),
            inbox(4, false, true, None),
        ];
        app.index_view = IndexView::Archived;
        app.api_tags = Vec::new();
        app.rebuild_tags();

        assert_eq!(app.filtered, vec![2, 3]);
    }

    #[test]
    fn t_key_toggles_view() {
        let mut app = test_app();
        app.inboxes = vec![
            inbox(1, true, false, None),
            inbox(2, true, true, None),
        ];
        app.api_tags = Vec::new();
        app.rebuild_tags();
        assert_eq!(app.filtered, vec![0]);

        app.handle_list_key(KeyCode::Char('t'));
        assert_eq!(app.index_view, IndexView::Archived);
        assert_eq!(app.filtered, vec![1]);

        app.handle_list_key(KeyCode::Char('t'));
        assert_eq!(app.index_view, IndexView::Processed);
        assert_eq!(app.filtered, vec![0]);
    }

    #[test]
    fn tag_filter_combines_with_view() {
        let mut app = test_app();
        app.inboxes = vec![
            inbox(1, true, false, Some("Research")),
            inbox(2, true, false, Some("News")),
            inbox(3, true, true, Some("Research")),
        ];
        app.api_tags = vec![
            Tag { name: "Research".into(), color: None },
            Tag { name: "News".into(), color: None },
        ];
        app.rebuild_tags();

        app.selected_tag = app
            .tags
            .iter()
            .position(|t| t.name == "Research");
        app.apply_filter();
        assert_eq!(app.filtered, vec![0]);

        app.handle_list_key(KeyCode::Char('t'));
        assert_eq!(app.filtered, vec![2]);
    }
}
