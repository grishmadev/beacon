use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List},
};

use crate::frontend::app::{App, Tab};

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
        .interfaces
        .iter()
        .map(|f| f.ifname.as_ref().unwrap_or(&"---".to_string()).to_string())
        .collect::<Vec<String>>();

    let interfaces_block = List::new(iface_vec)
        .block(Block::default().borders(Borders::ALL).title(" Interface "))
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol(">>");

    let hosts_vec = app
        .hosts
        .iter()
        .map(|f| f.ssid.as_ref().unwrap_or(&"---".to_string()).to_string())
        .collect::<Vec<String>>();

    let hosts_block = List::new(hosts_vec)
        .block(Block::default().borders(Borders::ALL).title(" Hosts "))
        .highlight_style(Style::default().bg(Color::Yellow));

    // rendering active and non-active tab based on condition
    if app.active_tab == Tab::Interface {
        rect.render_stateful_widget(interfaces_block, chunks[0], &mut app.active_index);
        rect.render_widget(hosts_block, chunks[1]);
    } else {
        rect.render_stateful_widget(hosts_block, chunks[1], &mut app.active_index);
        rect.render_widget(interfaces_block, chunks[0]);
    }
}
