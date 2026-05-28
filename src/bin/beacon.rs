use beacon::{
    Command, Response,
    executer::response,
    frontend::{
        app::{App, Tab},
        ui::set_layouts,
    },
    types::InterfaceType,
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
    time::Duration,
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    if let Err(e) = main_loop().await {
        eprintln!("Error: {}", e);
    };
}
async fn main_loop() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    /*
     * commands are given to cmdsx, which are then processed and responses transfered to ressx
     * */
    let (ressx, mut resrx) = mpsc::unbounded_channel::<Response>();
    let (cmdsx, mut cmdrx) = mpsc::unbounded_channel::<Command>();

    let ressx_clone = ressx.clone();
    tokio::spawn(async move {
        while let Some(cmd) = cmdrx.recv().await {
            let ressx = ressx_clone.clone();
            tokio::spawn(async move {
                match cmd {
                    Command::Notification(msg) => {
                        let ressx_clone = ressx.clone();
                        tokio::spawn(async move {
                            let delay = 3;
                            let duration = Duration::from_secs(delay);
                            tokio::time::sleep(duration).await;
                            let _ = ressx_clone.send(Response::ClearNotification);
                        });
                        let _ = ressx.send(Response::Notification(msg));
                    }
                    _ => {
                        let response = match response(&cmd).await {
                            Ok(r) => r,
                            Err(e) => Response::Error(e.to_string()),
                        };
                        let _ = ressx.send(response);
                    }
                }
            });
        }
    });
    let cmdsx_clone = cmdsx.clone();
    tokio::spawn(async move {
        loop {
            // Scan for active Interfaces every second or so
            let _ = cmdsx_clone.send(Command::Tick);
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
    });

    let mut last_active_interface = None;
    loop {
        terminal.draw(|f| {
            set_layouts(&mut app, f);
        })?;

        if app.active_tab == Tab::Interface
            && let Some(idx) = app.iface_index.selected()
            && let Some(active_iface) = app.get_ifaces().get(idx)
            && last_active_interface != active_iface.ifname
            && active_iface.iftype == InterfaceType::Wireless
        {
            cmdsx.send(Command::ListActiveConnections(active_iface.clone()))?;
            last_active_interface = active_iface.ifname.clone();
        };

        if let Ok(response) = resrx.try_recv() {
            match response {
                Response::AllInterfaces(ifaces) => {
                    app.set_interfaces(ifaces);
                }
                Response::ActiveHosts(ifname, hosts) => {
                    app.set_hosts(hosts, &ifname);
                }
                Response::Notification(msg) => {
                    app.notification = Some(msg);
                }
                Response::ClearNotification => {
                    app.notification = None;
                }
                Response::Error(err) => {
                    let _ = cmdsx.send(Command::Notification(err));
                }
                Response::CurrentConnection(connection) => {
                    app.current_connection = connection;
                }
                Response::Tick => {
                    let active_iface = app.get_current_interface();
                    let _ = cmdsx.send(Command::ListInterfaces);
                    if let Some(iface) = active_iface.clone() {
                        let _ = cmdsx.send(Command::ListActiveConnections(iface));
                    };
                    let _ = cmdsx.send(Command::CurrentConnection);
                }
                Response::Connected => {
                    let _ = cmdsx.send(Command::Notification("Connected.".into()));
                }
                _ => {}
            }
        }

        if event::poll(Duration::from_millis(10))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_keys(key, &cmdsx);
            if let KeyCode::Char('q') = key.code {
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
