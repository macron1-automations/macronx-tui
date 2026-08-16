mod inbox_list;
mod inbox_show;

use ratatui::Frame;

use crate::app::{App, Screen};

pub fn render(f: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::InboxList => inbox_list::render(f, app),
        Screen::InboxShow => inbox_show::render(f, app),
    }
}
