#!/bin/bash

set -e

# Ensure Script in sudo
if [ "$EUID" -ne 0 ]; then
  echo "Please run the installation script using sudo."
  exit 1
fi

DAEMON_NAME="beacond"
CLIENT_NAME="beacon"
SYSTEMD_PATH="/etc/systemd/system"
RUNIT_PATH="/etc/sv/"

echo "Make sure you ran \"cargo build --release\""
echo "Starting installation in 1 secs"
sleep 1

echo "Installing Binary..."
cp -f ./target/release/$DAEMON_NAME /usr/bin/$DAEMON_NAME
cp -f ./target/release/$CLIENT_NAME /usr/bin/$CLIENT_NAME
chmod 755 /usr/bin/$DAEMON_NAME

echo "Detecting Service Manager"

if [ -d "$SYSTEMD_PATH" ] && command -v systemctl >/dev/null 2>&1; then
  echo "Detected systemd, installing systemd service"

  cat <<EOF >"$SYSTEMD_PATH/$DAEMON_NAME.service"
[Unit]
Description=Beacond Network Manager
After=network.target

[Service]
ExecStart=/usr/bin/$DAEMON_NAME
Type=simple
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable "$DAEMON_NAME.service"
  systemctl start "$DAEMON_NAME.service"
elif [ -d "$RUNIT_PATH" ] && command -v sv >/dev/null 2>&1; then
  echo "Detected Runit, installing runit service"
  mkdir -p "$RUNIT_PATH/$DAEMON_NAME/log"
  mkdir -p "/var/log/$DAEMON_NAME"

  cat <<EOF >"$RUNIT_PATH/$DAEMON_NAME/run"
#!/bin/bash
exec /usr/bin/$DAEMON_NAME 2>&1
EOF

  cat <<EOF >"$RUNIT_PATH/$DAEMON_NAME/log/run"
#!/bin/bash
exec svlogd -tt /var/log/$DAEMON_NAME
EOF

  chmod +x "$RUNIT_PATH/$DAEMON_NAME/run"
  chmod +x "$RUNIT_PATH/$DAEMON_NAME/log/run"

  if [ -d "/var/service" ]; then
    ln -sf "$RUNIT_PATH/$DAEMON_NAME" "/var/service/"
  elif [ -d "/etc/service" ]; then
    ln -sf "$RUNIT_PATH/$DAEMON_NAME" "/etc/service/"
  fi
  echo "$DAEMON_NAME registered in runit Successfully."
else
  echo "Warning: No Supported Service Manager Detected."
  echo "The binary has been installed to /usr/local/bin/$DAEMON_NAME but must be run manually."
  echo "Try running beacond -b to run in background."
fi

echo "Installation Complete."
