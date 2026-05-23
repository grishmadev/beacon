# Beacon

![App Screenshot](assets/screenshot1.png)

### A Simple WiFi Manager

Hello Everyone, This is Beacon, a lightweight alternative for NetworkManager in the making.

## Architecture

This Project uses the Daemon - Client Achitecture meaning a daemon(beacond) will be running in the background while user can communicate with the daemon using the tui(beacon).

## Installation

Run the Install Script `install.sh`

```sh
./install.sh
```

## How to Run

1. Start the Daemon

```sh
# -b flag for running in background
sudo beacond -b

```

1. Start the Client

```sh
sudo beacon
```

## Crates Used

- dhcp4r - Creates needed structs and matches like Packets, Dhcp Messages etc.
- nl80211 - Convenient for Enums rather than raw u16 for talking to C Kernel.
- rand - Generate Random Tokens for Identification
- socket2 - Used for creating raw sockets
- rtnetlink - For convenience of creating Raw Commands, may be removed later.
- tokio - Async runtime
- serde - Serialization and De-Serialization
- serde_json - Json serialization and deserialization
- bincode - Helps with the actual conversion of Serilization and De-Serialization data.
- neli - Creating raw Netlink Commands
- ratatui - TUI Library
- chrono - Tracking time.

## Prerequisites

- `wpa_supplicant` needs to be installed

That's it!

This was the description pretty much.
A Star would be Appreciated.
