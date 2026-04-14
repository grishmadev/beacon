#!/bin/bash
# this is a program to setup test environment

sudo systemctl stop NetworkManager

# setting up wifi carf manually
sudo ip link set wlo1 up

# # creating config for wpa_supplicant
# sudo echo 'ctrl_interface=/var/run/wpa_supplicant/
# update_config=1' >/tmp/wpa_supplicant.conf

# kill wpa_supplicant
sudo killall wpa_supplicant
# turning wpa_supplicant manually
sudo wpa_supplicant -B -i wlo1 -c /etc/wpa_supplicant/wpa_supplicant.conf

echo "Setup Complete"
