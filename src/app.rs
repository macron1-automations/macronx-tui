use std::collections::HashSet;

use crossterm::event::KeyCode;

use crate::api::ApiClient;
use crate::config::Config;
use crate::models::{Inbox, Tag};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    InboxList,
    InboxShow,
}

pub struct App {
    pub screen: Screen,
    pub inboxes: Vec<Inbox>,
    pub tags: Vec<Tag>,
    pub selected_tag: Option<usize>, // index into `tags`, None = "All"
    pub filtered: Vec<usize>,        // indices into `inboxes` matching the active tag
    pub selected: usize,
    pub current_inbox: Option<Inbox>,
    pub status: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub client: ApiClient,
}

impl App {
    pub fn new(config: Config) -> Self {
        let client = ApiClient::new(&config.base_url, &config.api_token);
        Self {
            screen: Screen::InboxList,
            inboxes: Vec::new(),
            tags: Vec::new(),
            selected_tag: None,
            filtered: Vec::new(),
            selected: 0,
            current_inbox: None,
            status: None,
            should_quit: false,
            client,
        }
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
                self.current_inbox = Some(inbox);
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
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.screen = Screen::InboxList;
                self.status = None;
            }
            _ => {}
        }
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

        self.selected_tag = selected
            .as_deref()
            .and_then(|s| self.tags.iter().position(|t| t.name.eq_ignore_ascii_case(s)));
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
