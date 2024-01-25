#!/bin/bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
echo
echo "$HOME/.cargo/env"
source "/root/.cargo/env"
