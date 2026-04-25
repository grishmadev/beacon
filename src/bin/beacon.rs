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
    widgets::{Block, Borders, List, Paragraph, canvas::Line},
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
    loop {
        terminal.draw(|f| {
            let size = f.area();
            // let block = Block::default().title("Beacon").borders(Borders::ALL);
            // let hello = Paragraph::new("Hello World!").block(block);
            // f.render_widget(hello, size);
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(size);

            let header = Paragraph::new("BEACON")
                .style(Style::default().fg(ratatui::style::Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(header, chunks[0]);
            let list_items = [
                "lo      [UP]    127.0.0.1",
                "eth0    [DOWN]  NO_CARRIER",
                "wlo1    [UP]    192.168.1.5",
            ];

            let list = List::new(list_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Network Interface"),
                )
                .highlight_style(Style::default().bg(Color::Blue));

            f.render_widget(list, chunks[1]);
            let paragraph = Paragraph::new("Thanks for using beacon.")
                .block(Block::default().borders(Borders::ALL));

            let para2 =
                Paragraph::new("Paragraph 0.").block(Block::default().borders(Borders::ALL));

            let sec_chunk = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .spacing(1)
                .constraints([Constraint::Percentage(50), Constraint::Min(10)])
                .split(chunks[2]);
            f.render_widget(paragraph, sec_chunk[1]);
            f.render_widget(para2, sec_chunk[0]);
        })?;

        if let Event::Key(key) = event::read()?
            && let KeyCode::Char('q') = key.code
        {
            break;
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
