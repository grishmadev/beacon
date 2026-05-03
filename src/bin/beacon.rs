use beacon::{
    Command, Response,
    backend::executer::execute,
    debug::write,
    frontend::{
        app::{App, Tab},
        ui::set_layouts,
    },
};
use ratatui::{
    Terminal,
    crossterm::{
        event::{self, DisableMouseCapture, Event, KeyCode},
        execute,
        terminal::{LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    prelude::CrosstermBackend,
};
use std::{
    error::Error,
    io::{self},
    sync::mpsc,
    thread::{self, spawn},
    time::Duration,
};

#[tokio::main]
async fn main() {
    if let Err(e) = main_loop().await {
        let _ = write(e.to_string());
    };
}
async fn main_loop() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::default();
    /*
     * commands are given to cmdsx, which are then processed and responses transfered to ressx
     * */
    let (ressx, resrx) = mpsc::channel::<Response>();
    let (cmdsx, cmdrx) = mpsc::channel::<Command>();

    tokio::spawn(async move {
        while let Ok(cmd) = cmdrx.recv() {
            let response = match cmd {
                Command::Notification(msg) => Response::Notification(msg),
                _ => match execute(&cmd).await {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Error: {:?}", e.to_string());
                        Response::Error(e.to_string())
                    }
                },
            };
            let _ = ressx.send(response);
        }
    });
    let cmdsx_clone = cmdsx.clone();
    spawn(move || {
        loop {
            let _ = cmdsx_clone.send(Command::ListInterfaces);
            thread::sleep(Duration::from_secs(1));
        }
    });

    let mut last_active_interface = None;
    loop {
        terminal.draw(|f| {
            set_layouts(&mut app, f);
        })?;

        if app.active_tab == Tab::Interface
            && let Some(idx) = app.iface_index.selected()
            && let Some(active_iface) = app.interfaces.get(idx)
            && last_active_interface != active_iface.ifname
        {
            cmdsx.send(Command::ListActiveConnections(active_iface.clone()))?;
            last_active_interface = active_iface.ifname.clone();
        };

        if let Ok(response) = resrx.try_recv() {
            match response {
                Response::AllInterfaces(ifaces) => {
                    app.interfaces = ifaces;
                }
                Response::ActiveHosts(hosts) => {
                    app.hosts = hosts;
                }
                Response::Notification(msg) => app.notification = Some(msg),
                _ => {}
            }
        }

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                app.handle_keys(key);
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Enter => {
                        if let Some(_) = app.hosts.iter().find(|host| host.is_connected) {
                            app.connect(&cmdsx, Some("kakakaka".into()));
                        } else {
                            let _ = cmdsx.send(Command::Disconnect);
                        }
                    }
                    _ => {}
                }
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
