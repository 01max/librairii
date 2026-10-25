#!/usr/bin/env bash
# Runs only inside the owned disposable cloud-image guest, before app launch.
set -euo pipefail
mkdir -p /var/lib/qualification /opt/qualification
trap 'printf failed > /var/lib/qualification/provision-status' ERR
cp /mnt/qualification/guest.py /mnt/qualification/drive.py /mnt/qualification/check-wayland.py /mnt/qualification/inspect-runtime.py /opt/qualification/
cp /mnt/qualification/package.tar.gz /mnt/qualification/identity.json /var/lib/qualification/
chmod -R a+rX /opt/qualification
mkdir /var/lib/qualification/evidence
chown qualification:qualification /var/lib/qualification/evidence
# No network is needed after provisioning, including on the next boot.
systemctl disable systemd-networkd-wait-online.service
touch /etc/cloud/cloud-init.disabled
systemctl enable --now qemu-guest-agent
printf ready > /var/lib/qualification/provision-status
