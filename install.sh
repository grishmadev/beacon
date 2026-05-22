#!/bin/bash

echo "Building Beacon Client"
cargo build --bin beacon --release
echo "Build Complete"
echo "Building Beacon Daemon"
cargo build --bin beacond --release
echo "Build Complete"

sudo cp ./target/release/beacon /usr/bin/beacon
sudo cp ./target/release/beacond /usr/bin/beacond

echo "Beacon Installation Complete"
echo "Start Beacon Daemon by"
echo "sudo beacond -b"
