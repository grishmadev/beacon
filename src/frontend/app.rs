use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    widgets::ListState,
};

use crate::types::Host;

#[derive(Default, Debug, PartialEq)]
pub enum Tab {
    #[default]
    Interface,
    Hosts,
}

#[derive(Default)]
pub struct App {
    pub interfaces: Vec<String>,
    pub hosts: Vec<Host>,
    pub active_tab: Tab,
    pub active_index: ListState, // starts from 0
    pub is_running: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            interfaces: vec![],
            hosts: vec![],
            active_tab: Tab::Interface,
            active_index: ListState::default(),
            is_running: true,
        }
    }
    pub fn next(&mut self) {
        let i = match self.active_index.selected() {
            Some(s) => {
                if s == self.interfaces.len() - 1 {
                    0
                } else {
                    s + 1
                }
            }
            None => 0,
        };
        self.active_index.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.active_index.selected() {
            Some(s) => {
                if s == 0 {
                    self.interfaces.len() - 1
                } else {
                    s - 1
                }
            }
            None => 0,
        };
        self.active_index.select(Some(i));
    }

    pub fn handle_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Char('k') => {
                self.previous();
            }
            _ => {}
        }
    }
}
