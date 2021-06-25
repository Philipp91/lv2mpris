#!/bin/bash
OUTPUT_DIR="${OUTPUT_DIR:-./lv2mpris.lv2}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.lv2}"
set -e

# BUILD
./build.sh

# INSTALL OUTPUT
mkdir -p "${INSTALL_DIR?}"
rm -rf "${INSTALL_DIR:?}/$(basename "${OUTPUT_DIR?}")"
cp -v -R "${OUTPUT_DIR?}" "${INSTALL_DIR?}"

# START TEST APPLICATION
carla ./carlatest.carxp
