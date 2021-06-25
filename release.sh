#!/bin/bash
OUTPUT_DIR="${OUTPUT_DIR:-./lv2mpris.lv2}"
RELEASE_FILE="${RELEASE_FILE:-lv2mpris.zip}"
set -e

read -p "If you want, bump the version number in Cargo.toml. Then press enter."

./build.sh

zip "${RELEASE_FILE?}" -r "${OUTPUT_DIR?}"
echo "Done, see ${RELEASE_FILE?}"
