#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Refresh Ubuntu's package indexes before installing a runner's build dependencies.
set -euo pipefail

# The hosted image can retain indexes for an Azure mirror that no longer resolves.
# Keep Ubuntu's official archive as the source; arm64's ports archive stays unchanged.
for source in /etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources; do
  [ -f "$source" ] || continue
  sudo sed -i 's|http://azure.archive.ubuntu.com/ubuntu|https://archive.ubuntu.com/ubuntu|g; s|https://azure.archive.ubuntu.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' "$source"
done
sudo apt-get -o Acquire::Retries=3 -o APT::Update::Error-Mode=any update
sudo apt-get -o Acquire::Retries=3 install -y --no-install-recommends "$@"
