#!/bin/bash
OUTPUT_DIR="${OUTPUT_DIR:-./lv2mpris.lv2}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.lv2}"
set -e

# BUILD
cargo build --release

# ASSEMBLE OUTPUT
cp -v ./target/release/liblv2mpris.so "${OUTPUT_DIR?}"
