use std::sync::mpsc::Sender;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    widgets::ListState,
};

use crate::{
    Command,
    debug::write,
    types::{Host, Interface},
};

#[derive(Default, Debug, PartialEq, Clone)]
pub enum Tab {
    #[default]
    Interface,
    Hosts,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct InterfaceList {
    pub iface: Interface,
    pub hosts: Vec<Host>,
}
#[derive(Debug, Clone)]
pub struct App {
    pub group: Vec<InterfaceList>,
    pub active_tab: Tab,
    pub iface_index: ListState, // starts from 0
    pub host_index: ListState,
    pub notification: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            group: vec![],
            active_tab: Tab::Interface,
            iface_index: ListState::default(),
            host_index: ListState::default(),
            notification: None,
        }
    }
    pub fn get_ifaces(&mut self) -> Vec<Interface> {
        self.group
            .iter()
            .map(|i| i.iface.clone())
            .collect::<Vec<Interface>>()
    }
    pub fn get_hosts(&mut self) -> Vec<Host> {
        if let Some(idx) = self.iface_index.selected()
            && let Some(ifls) = self.group.get(idx)
        {
            ifls.hosts.clone()
        } else {
            vec![]
        }
    }
    fn next(&mut self) {
        let ifaces = self.get_ifaces();
        let hosts = self.get_hosts();
        if self.active_tab == Tab::Interface {
            let mut i = match self.iface_index.selected() {
                Some(s) => {
                    if s == ifaces.len() - 1 {
                        Some(0)
                    } else {
                        Some(s + 1)
                    }
                }
                None => Some(0),
            };
            if i >= Some(ifaces.len()) {
                i = None;
            };
            self.iface_index.select(i);
        } else if self.active_tab == Tab::Hosts {
            let mut i = match self.host_index.selected() {
                Some(s) => {
                    if s == hosts.len() - 1 {
                        Some(0)
                    } else {
                        Some(s + 1)
                    }
                }
                None => Some(0),
            };
            if i >= Some(hosts.len()) {
                i = None;
            };
            self.host_index.select(i);
        }
    }

    fn previous(&mut self) {
        let ifaces = self.get_ifaces();
        let hosts = self.get_hosts();
        if self.active_tab == Tab::Interface {
            let mut i = match self.iface_index.selected() {
                Some(s) => {
                    if s == 0 {
                        Some(ifaces.len() - 1)
                    } else {
                        Some(s - 1)
                    }
                }
                None => Some(0),
            };
            if i > Some(ifaces.len()) {
                i = None;
            };
            self.iface_index.select(i);
        } else if self.active_tab == Tab::Hosts {
            let mut i = match self.host_index.selected() {
                Some(s) => {
                    if s == 0 {
                        Some(hosts.len() - 1)
                    } else {
                        Some(s - 1)
                    }
                }
                None => Some(0),
            };
            if i > Some(hosts.len()) {
                i = None;
            };
            self.host_index.select(i);
        }
    }
    fn toggle_tab(&mut self) {
        // both blocks are empty, going further is useless

        // checks for iface blocks first
        if self.group.is_empty() {
            return;
        };
        let hosts = self.get_hosts();

        // check which tab the app is already pointing to and choose the opposite one
        if self.active_tab == Tab::Interface {
            // switch only if the other party is not empty
            if !hosts.is_empty() {
                self.active_tab = Tab::Hosts;
            }
        } else {
            // same logic from above
            if !self.group.is_empty() {
                self.active_tab = Tab::Interface;
            }
        };
    }

    pub fn connect(&mut self, sx: &Sender<Command>, password: Option<String>) {
        if self.active_tab != Tab::Hosts {
            let _ = sx.send(Command::Notification(Some("No Host Selected.".to_string())));
            return;
        }
        let hosts = self.get_hosts();
        let interfaces = self.get_ifaces();
        if let Some(idx) = self.host_index.selected()
            && let Some(target_host) = hosts.get(idx)
            && let Some(bssid) = target_host.bssid.clone()
            && let Some(iface_idx) = self.iface_index.selected()
            && let Some(iface) = interfaces.get(iface_idx)
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
        let mut result = vec![];
        for iface in ifaces {
            if self.get_ifaces().contains(&iface) {
                let hosts = self
                    .group
                    .iter()
                    .find(|i| i.iface.ifname == iface.ifname)
                    .map(|i| i.hosts.clone())
                    .unwrap_or(vec![]);
                result.push(InterfaceList { iface, hosts });
            } else {
                result.push(InterfaceList {
                    iface,
                    hosts: vec![],
                });
            }
        }
        self.group = result;
    }

    pub fn set_hosts(&mut self, hosts: Vec<Host>, ifname: &str) {
        let _ = write(format!("hosts: {:#?}, ifname: {}", hosts, ifname));
        if let Some(target) = self
            .group
            .iter_mut()
            .find(|f| f.iface.ifname == Some(ifname.to_string()))
        {
            target.hosts = hosts;
        };
    }
}
