use std::sync::mpsc::Sender;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    widgets::{ListState, TableState},
};
use serde::{Deserialize, Serialize};

use crate::{
    Command,
    debug::write,
    types::{CurrentConnection, Host, Interface},
};

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Interface,
    Hosts,
    Input,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceList {
    pub iface: Interface,
    pub hosts: Vec<Host>,
}
#[derive(Debug, Clone, Default)]
pub struct App {
    pub group: Vec<InterfaceList>,
    pub active_tab: Tab,
    pub iface_index: ListState, // starts from 0
    pub host_index: TableState,
    pub notification: Option<String>,
    pub current_connection: Option<CurrentConnection>,
    pub input_text: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Interface,
            ..Default::default()
        }
    }
    pub fn get_ifaces(&mut self) -> Vec<Interface> {
        self.group
            .iter()
            .map(|i| i.iface.clone())
            .collect::<Vec<Interface>>()
    }
    pub fn get_current_interface(&mut self) -> Option<Interface> {
        if let Some(idx) = self.iface_index.selected()
            && let Some(ifl) = self.group.get(idx)
        {
            let iface = ifl.iface.clone();
            Some(iface)
        } else {
            None
        }
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

        if self.active_tab == Tab::Input {
            return;
        }
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

    fn get_current_host(&mut self, respect_tab: bool) -> Option<Host> {
        let hosts = self.get_hosts();
        if let Some(idx) = self.host_index.selected() {
            if respect_tab {
                if self.active_tab == Tab::Hosts
                    && let Some(host) = hosts.get(idx)
                {
                    return Some(host.clone());
                } else {
                    return None;
                }
            }
            if let Some(host) = hosts.get(idx) {
                return Some(host.clone());
            } else {
                return None;
            }
        }
        None
    }

    pub fn connect(&mut self, sx: &Sender<Command>, host: Host, password: Option<String>) {
        let cmd = if let Some(iface) = self.get_current_interface() {
            Command::Connect {
                host,
                password,
                iface: iface.clone(),
            }
        } else {
            Command::Notification("Cannot Connect, Interface not set.".into())
        };
        let _ = sx.send(cmd);
    }

    fn delete_char(&mut self) {
        self.input_text.pop();
    }

    pub fn handle_keys(&mut self, key: KeyEvent, sx: &Sender<Command>) {
        match self.active_tab.clone() {
            Tab::Input => match key.code {
                KeyCode::Char(ch) => {
                    self.input_text.push(ch);
                }
                KeyCode::Backspace => {
                    self.delete_char();
                }
                KeyCode::Esc => {
                    self.active_tab = Default::default();
                }
                KeyCode::Enter => {
                    if let Some(cur_host) = self.get_current_host(false) {
                        self.connect(sx, cur_host, Some(self.input_text.clone()));
                        self.active_tab = Tab::Interface;
                        self.input_text = String::new();
                    } else {
                        let _ = sx.send(Command::Notification("Error in app".to_string()));
                    }
                }
                _ => {}
            },
            s => {
                if s == Tab::Hosts
                    && let KeyCode::Enter = key.code
                {
                    let hosts = self.get_hosts();
                    if hosts.iter().find(|h| h.is_connected).is_some() {
                        if let Some(iface) = self.get_current_interface() {
                            let _ = sx.send(Command::Disconnect(iface.ifname.unwrap()));
                        }
                    } else {
                        self.active_tab = Tab::Input;
                    }
                };
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.next();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.previous();
                    }
                    KeyCode::Tab
                    | KeyCode::Right
                    | KeyCode::Left
                    | KeyCode::Char('h')
                    | KeyCode::Char('l') => {
                        self.toggle_tab();
                    }
                    _ => {}
                }
            }
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
