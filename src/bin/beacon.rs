use beacon::frontend::app::App;
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
    widgets::{Block, BorderType, Borders, List, Padding, Paragraph, canvas::Line},
};
use std::{
    error::Error,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut count = 0;
    let mut app = App::default();
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

            let list_items = [
                "lo      [UP]    127.0.0.1",
                "eth0    [DOWN]  NO_CARRIER",
                "wlo1    [UP]    192.168.1.5",
            ];
            let interfaces_block = List::new(list_items)
                .block(Block::default().borders(Borders::ALL).title(" Interface "))
                .highlight_style(Style::default().bg(Color::Blue))
                .highlight_symbol(">>");
            f.render_widget(interfaces_block, chunks[0]);
            let hosts = [
                "Redmi X11",
                "One Kafka",
                "NASS Surveilence",
                "Redmi X11",
                "One Kafka",
                "NASS Surveilence",
            ];

            let hosts_block = List::new(hosts)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" Hosts {}", count)),
                )
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
