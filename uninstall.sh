#!/bin/bash

set -e
echo "Uninstalling Binary..."

sudo rm -rf /usr/bin/beacond
sudo rm -rf /usr/bin/beacon

sudo rm -rf /etc/sv/beacond/
sudo rm -rf /var/service/beacond

echo "Uninstall Complete..."
