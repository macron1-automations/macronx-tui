use crossterm::event::KeyCode;

use crate::api::ApiClient;
use crate::config::Config;
use crate::models::Inbox;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    InboxList,
    InboxShow,
}

pub struct App {
    pub screen: Screen,
    pub inboxes: Vec<Inbox>,
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
                if self.selected >= self.inboxes.len() && !self.inboxes.is_empty() {
                    self.selected = self.inboxes.len() - 1;
                }
                self.status = None;
            }
            Err(e) => {
                self.status = Some((format!("Error loading inboxes: {}", e), true));
            }
        }
    }

    pub fn open_selected_inbox(&mut self) {
        if self.inboxes.is_empty() {
            return;
        }
        let id = self.inboxes[self.selected].id;
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
                if !self.inboxes.is_empty() {
                    self.selected = (self.selected + 1).min(self.inboxes.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::Char('G') => {
                if !self.inboxes.is_empty() {
                    self.selected = self.inboxes.len() - 1;
                }
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
}
