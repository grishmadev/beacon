# Beacon

### A simple wifi manager

Hello Everyone, This is Beacon, a lightweight alternative for NetworkManager in the making.

## Architecture

This Project uses the Daemon - Client Achitecture meaning a daemon(beacond) will be running in the background while user can communicate with the daemon using the tui(beacon).

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
- neli - Used for creating raw Netlink Commands
- ratatui - TUI Library
- chrono - For Tracking time.

## Prerequisites

- `wpa_supplication` needs to be installed

Thats it!

This was the description pretty much.
A Star would be Appreciated.
