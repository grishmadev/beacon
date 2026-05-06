use beacon::{
    Command, Response,
    backend::executer::execute,
    debug::write,
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
    let mut app = App::new();
    /*
     * commands are given to cmdsx, which are then processed and responses transfered to ressx
     * */
    let (ressx, resrx) = mpsc::channel::<Response>();
    let (cmdsx, cmdrx) = mpsc::channel::<Command>();

    let cmdsx_clone = cmdsx.clone();
    tokio::spawn(async move {
        while let Ok(cmd) = cmdrx.recv() {
            if let Command::Notification(msg) = cmd {
                let cmdsx_clone = cmdsx_clone.clone();
                if msg.is_some() {
                    tokio::spawn(async move {
                        let delay = 3;
                        let duration = Duration::from_secs(delay);
                        let _ = write("disable notifcation in 3 secs".to_string());
                        tokio::time::sleep(duration).await;
                        let _ = cmdsx_clone.send(Command::Notification(None));
                    });
                }
                let response = Response::Notification(msg);
                let _ = ressx.send(response);
                return;
            }
            let response = match execute(&cmd).await {
                Ok(r) => r,
                Err(e) => Response::Error(e.to_string()),
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
                    app.notification = msg;
                }
                Response::Error(err) => {
                    let _ = cmdsx.send(Command::Notification(Some(err)));
                }
                Response::Connected => {
                    let _ = cmdsx.send(Command::Notification(Some("Connected.".into())));
                }
                _ => {}
            }
        }

        if event::poll(Duration::from_millis(10))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_keys(key);
            match key.code {
                KeyCode::Char('q') => {
                    break;
                }
                KeyCode::Enter => {
                    if app
                        .get_hosts()
                        .iter()
                        .find(|host| host.is_connected)
                        .is_some()
                    {
                        // disconnect if connected
                        let _ = cmdsx.send(Command::Disconnect);
                    } else {
                        // connect if disconnected
                        app.connect(&cmdsx, Some("kakakakaka".into()));
                    }
                }
                _ => {}
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
