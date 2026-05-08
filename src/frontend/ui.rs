use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, BorderType, Borders, Clear, List, Paragraph, Row, Table},
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

    let left_block = Block::default();

    let left_inner = left_block.inner(chunks[0]);

    rect.render_widget(left_block, chunks[0]);

    let left_inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(15)])
        .split(left_inner);

    let iface_vec = app
        .get_ifaces()
        .iter()
        .map(|f| f.ifname.as_ref().unwrap_or(&"---".to_string()).to_string())
        .collect::<Vec<String>>();

    let iface_count = if iface_vec.is_empty() {
        "No Interface".into()
    } else {
        iface_vec.len().to_string()
    };

    let interfaces_block = List::new(iface_vec)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Interface ({}) ", iface_count)),
        )
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol(">> ");

    if let Some(curcon) = app.current_connection.clone() {
        let mut current_connection_list = vec![];
        let mut add_attr = |l: &str, r: &str| {
            current_connection_list.push(format!("{:<20}: {:>15}", l, r));
        };
        if let Some(ip) = curcon.ip_addr {
            add_attr("IP", &ip.to_string());
        }
        if let Some(dns) = curcon.dns_servers.first() {
            add_attr("DNS", &dns.to_string());
        }
        if let Some(server_id) = curcon.server_id {
            add_attr("Server ID", &server_id.to_string());
        }
        if let Some(subnet) = curcon.subnet_mask {
            add_attr("Subnet", &subnet.to_string());
        }
        if let Some(gateway) = curcon.gateway {
            add_attr("Gateway", &gateway.to_string());
        }
        if let Some(freq) = curcon.frequency {
            add_attr("Frequency", &freq.to_string());
        }
        add_attr("Lease Duration", &curcon.lease_duration.to_string());
        add_attr("Renewal Time", &curcon.renewal_time.to_string());
        add_attr("Rebinding Time", &curcon.rebinding_time.to_string());

        let current_connection = List::new(current_connection_list).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Current Connection "),
        );
        rect.render_widget(current_connection, left_inner_chunks[1]);
    }

    let hosts_vec = app.get_hosts();
    let host_header = Row::new(vec!["SSID", "BSSID", "FREQUENCY", "SIGRATE", "CONNECTED"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let hosts = hosts_vec.iter().map(|f| {
        let ssid = f.ssid.clone().unwrap_or("--".to_string());
        let connected = if f.is_connected { "YES" } else { "NO" }.to_string();
        let bssid = f.bssid.clone().unwrap_or("--".to_string());
        let freq = f.frequency.unwrap_or_default().to_string();
        let signal = f.signal.unwrap_or_default().to_string();

        Row::new(vec![ssid, bssid, freq, signal, connected])
    });

    let host_count = if hosts_vec.is_empty() {
        "No Hosts".into()
    } else {
        hosts_vec.len().to_string()
    };

    let hosts_block = Table::new(
        hosts,
        [
            Constraint::Length(20), // SSID
            Constraint::Length(15), // BSSID
            Constraint::Length(15), //FREQ
            Constraint::Length(15), // SIGNAL
            Constraint::Length(15), // CONNECTED
        ],
    )
    .header(host_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Hosts ({}) ", host_count)),
    )
    .row_highlight_style(Style::default().bg(Color::Yellow));

    if let Some(ref msg) = app.notification {
        let block = Block::new()
            .title(" Notification ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .fg(Color::Yellow);

        let area = centered_rect(60, 40, rect.area());

        rect.render_widget(Clear, area);
        rect.render_widget(Paragraph::new(msg.to_string()).block(block), area);
    }
    // rendering active and non-active tab based on condition
    if app.active_tab == Tab::Interface {
        rect.render_stateful_widget(interfaces_block, left_inner_chunks[0], &mut app.iface_index);
        rect.render_widget(hosts_block, chunks[1]);
    } else {
        rect.render_stateful_widget(hosts_block, chunks[1], &mut app.host_index);
        rect.render_widget(interfaces_block, left_inner_chunks[0]);
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
