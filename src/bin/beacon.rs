use beacon::{
    Command, Response,
    backend::executer::execute,
    frontend::app::{App, Tab},
    types::Interface,
    wifi::helper::get_interfaces,
};
use ratatui::{
    Terminal,
    crossterm::{
        event::{self, DisableMouseCapture, Event, KeyCode},
        execute,
        terminal::{LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout},
    prelude::CrosstermBackend,
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List},
};
use std::{
    error::Error,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
    thread,
    time::Duration,
};
use tokio::time::sleep;

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::default();
    let hosts = [
        "Redmi X11",
        "One Kafka",
        "NASS Surveilence",
        "Redmi X11",
        "One Kafka",
        "NASS Surveilence",
    ]
    .to_vec()
    .iter()
    .map(|f| f.to_string())
    .collect::<Vec<String>>();
    app.interfaces.extend(hosts.clone());
    let (ressx, resrx) = mpsc::channel::<Response>();
    let (cmdsx, cmdrx) = mpsc::channel::<Command>();
    // let mut active_iface: Interface;

    tokio::spawn(async move {
        loop {
            match cmdrx.try_recv() {
                Ok(cmd) => {
                    let response = match execute(&cmd).await {
                        Ok(r) => r,
                        Err(e) => Response::Error(e.to_string()),
                    };
                    let _ = ressx.send(response);
                }
                _ => break,
            };
        }
    });
    loop {
        terminal.draw(|f| {
            let size = f.area();

            let outer_main = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" BEACON SURVEILLANCE SYSTEM ");

            let inner_main = outer_main.inner(size);
            f.render_widget(outer_main, size);

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([Constraint::Length(40), Constraint::Min(100)])
                .split(inner_main);

            // let mut list_items = vec![];
            if let Ok(Response::Pong) = resrx.try_recv() {
                // list_items = interfaces
                //     .iter()
                //     .map(|iface| iface.ifname.clone().unwrap_or("UNKNOWN".to_string()))
                //     .collect::<Vec<String>>();
            }
            let interfaces_block = List::new(hosts.clone())
                .block(Block::default().borders(Borders::ALL).title(" Interface "))
                .highlight_style(Style::default().bg(Color::Blue))
                .highlight_symbol(">>");
            f.render_widget(interfaces_block, chunks[0]);

            let hosts_block = List::new(app.interfaces.clone())
                .block(Block::default().borders(Borders::ALL).title(" Hosts "))
                .highlight_style(Style::default().bg(Color::Yellow));
            f.render_stateful_widget(hosts_block, chunks[1], &mut app.active_index);
        })?;

        if let Event::Key(key) = event::read()? {
            app.handle_keys(key);
            if key.code == KeyCode::Char('q') {
                break;
            }
        }
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}
