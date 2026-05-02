use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    widgets::ListState,
};

use crate::types::{Host, Interface};

#[derive(Default, Debug, PartialEq)]
pub enum Tab {
    #[default]
    Interface,
    Hosts,
}

#[derive(Default)]
pub struct App {
    pub interfaces: Vec<Interface>,
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
    fn next(&mut self) {
        if self.active_tab == Tab::Interface {
            let mut i = match self.active_index.selected() {
                Some(s) => {
                    if s == self.interfaces.len() - 1 {
                        Some(0)
                    } else {
                        Some(s + 1)
                    }
                }
                None => Some(0),
            };
            if i >= Some(self.interfaces.len()) {
                i = None;
            };
            self.active_index.select(i);
        } else {
            let mut i = match self.active_index.selected() {
                Some(s) => {
                    if s == self.hosts.len() - 1 {
                        Some(0)
                    } else {
                        Some(s + 1)
                    }
                }
                None => Some(0),
            };
            if i >= Some(self.hosts.len()) {
                i = None;
            };
            self.active_index.select(i);
        }
    }

    fn previous(&mut self) {
        if self.active_tab == Tab::Interface {
            let mut i = match self.active_index.selected() {
                Some(s) => {
                    if s == 0 {
                        Some(self.interfaces.len() - 1)
                    } else {
                        Some(s - 1)
                    }
                }
                None => Some(0),
            };
            if i > Some(self.interfaces.len()) {
                i = None;
            };
            self.active_index.select(i);
        } else {
            let mut i = match self.active_index.selected() {
                Some(s) => {
                    if s == 0 {
                        Some(self.hosts.len() - 1)
                    } else {
                        Some(s - 1)
                    }
                }
                None => Some(0),
            };
            if i > Some(self.hosts.len()) {
                i = None;
            };
            self.active_index.select(i);
        }
    }
    fn toggle_tab(&mut self) {
        let target_tab = match self.active_tab {
            Tab::Interface => Tab::Hosts,
            Tab::Hosts => Tab::Interface,
        };
        self.active_tab = target_tab;
    }

    pub fn handle_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::Tab => {
                self.toggle_tab();
            }
            _ => {}
        }
    }

    pub fn set_interfaces(&mut self, ifaces: Vec<Interface>) {
        self.interfaces = ifaces;
    }

    pub fn set_hosts(&mut self, hosts: Vec<Host>) {
        self.hosts = hosts;
    }
}
