use std::sync::mpsc::Sender;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    widgets::ListState,
};

use crate::{
    Command,
    backend::functions::disconnect_connection,
    types::{Host, Interface},
};

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
    pub iface_index: ListState, // starts from 0
    pub host_index: ListState,
    pub notification: Option<String>,
    pub is_running: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            interfaces: vec![],
            hosts: vec![],
            active_tab: Tab::Interface,
            iface_index: ListState::default(),
            host_index: ListState::default(),
            notification: None,
            is_running: true,
        }
    }
    fn next(&mut self) {
        if self.active_tab == Tab::Interface {
            let mut i = match self.iface_index.selected() {
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
            self.iface_index.select(i);
        } else if self.active_tab == Tab::Hosts {
            let mut i = match self.host_index.selected() {
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
            self.host_index.select(i);
        }
    }

    fn previous(&mut self) {
        if self.active_tab == Tab::Interface {
            let mut i = match self.iface_index.selected() {
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
            self.iface_index.select(i);
        } else if self.active_tab == Tab::Hosts {
            let mut i = match self.host_index.selected() {
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
            self.host_index.select(i);
        }
    }
    fn toggle_tab(&mut self) {
        // both blocks are empty, going further is useless
        if self.interfaces.is_empty() && self.hosts.is_empty() {
            return;
        };

        // check which tab the app is already pointing to and choose the opposite one
        if self.active_tab == Tab::Interface {
            // switch only if the other party is not empty
            if !self.hosts.is_empty() {
                self.active_tab = Tab::Hosts;
            }
        } else {
            // same logic from above
            if !self.interfaces.is_empty() {
                self.active_tab = Tab::Interface;
            }
        };
    }

    pub fn connect(&mut self, sx: &Sender<Command>, password: Option<String>) {
        if self.active_tab != Tab::Hosts {
            self.notification = Some("No Host Selected.".to_string());
            return;
        }
        if let Some(idx) = self.host_index.selected()
            && let Some(target_host) = self.hosts.get(idx)
            && let Some(bssid) = target_host.bssid.clone()
            && let Some(iface_idx) = self.iface_index.selected()
            && let Some(iface) = self.interfaces.get(iface_idx)
        {
            {
                let _ = sx.send(Command::Connect {
                    bssid,
                    password,
                    iface: iface.clone(),
                });
            }
        }
    }

    pub fn handle_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.next();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.previous();
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
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
