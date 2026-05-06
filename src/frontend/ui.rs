use std::{
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, Borders, Clear, List, Paragraph},
};

use crate::{
    debug::{self, write},
    frontend::app::{App, Tab},
};

pub fn set_layouts(app: &mut App, rect: &mut Frame) {
    let size = rect.area();
    let outer_main = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" BEACON SURVEILLANCE SYSTEM ");

    let inner_main = outer_main.inner(size);
    rect.render_widget(outer_main, size);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Length(40), Constraint::Min(100)])
        .split(inner_main);

    let iface_vec = app
        .get_ifaces()
        .iter()
        .map(|f| f.ifname.as_ref().unwrap_or(&"---".to_string()).to_string())
        .collect::<Vec<String>>();

    let interfaces_block = List::new(iface_vec)
        .block(Block::default().borders(Borders::ALL).title(" Interface "))
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol(">>");

    let hosts_vec = app
        .get_hosts()
        .iter()
        .map(|f| f.ssid.as_ref().unwrap_or(&"---".to_string()).to_string())
        .collect::<Vec<String>>();
    // let mut wrote = false;
    // if !hosts_vec.is_empty() {
    //     wrote = true;
    //     write(format!("hosts: {:#?}", hosts_vec));
    // };
    // let hostc = hosts_vec.clone();
    // if wrote {
    //     thread::spawn(move || {
    //         loop {
    //             thread::sleep(Duration::from_secs(2));
    //             write(format!("hosts after: {:#?}", hostc));
    //         }
    //     });
    // };
    //
    //
    let hosts_vec = if let Some(target) = app
        .group
        .iter()
        .find(|f| f.iface.ifname == Some("wlo1".to_string()))
    {
        target
            .hosts
            .clone()
            .iter()
            .map(|f| f.ssid.as_ref().unwrap_or(&"---".to_string()).to_string())
            .collect::<Vec<String>>()
    } else {
        vec![]
    };
    let host_count = if hosts_vec.is_empty() {
        "No Hosts".into()
    } else {
        hosts_vec.len().to_string()
    };

    let hosts_block = List::new(hosts_vec)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Hosts ({}) ", host_count)),
        )
        .highlight_style(Style::default().bg(Color::Yellow));

    if let Some(ref msg) = app.notification {
        let block = Block::new()
            .title(" Notification ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .fg(Color::Yellow);

        let area = centered_rect(60, 20, rect.area());

        rect.render_widget(Clear, area);
        rect.render_widget(Paragraph::new(msg.to_string()).block(block), area);
    }
    // rendering active and non-active tab based on condition
    if app.active_tab == Tab::Interface {
        rect.render_stateful_widget(interfaces_block, chunks[0], &mut app.iface_index);
        rect.render_widget(hosts_block, chunks[1]);
    } else {
        rect.render_stateful_widget(hosts_block, chunks[1], &mut app.host_index);
        rect.render_widget(interfaces_block, chunks[0]);
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let notification = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(rect);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(notification[1])[1]
}
