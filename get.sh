#!/bin/sh
# Installer Argus · GV Consulting
#
#   curl -fsSL https://get.gvconsulting.eu | sudo sh
#   curl -fsSL https://get.gvconsulting.eu | sudo sh -s -- --dir /opt/gv/box
#
# Scarica argus-get (pubblico), ne verifica lo SHA-256 e lo avvia.
# argus-get chiede l'accesso GitHub e scarica il pannello Argus (privato).
set -eu

BASE="${ARGUS_GET_BASE:-https://github.com/GV-CONSULTING-SRL/argus-get/releases/latest/download}"
BIN="argus-get-x86_64-linux-musl"

if [ "$(id -u)" -ne 0 ]; then
    echo "Serve root:  curl -fsSL https://get.gvconsulting.eu | sudo sh" >&2
    exit 1
fi
if [ "$(uname -m)" != "x86_64" ]; then
    echo "Architettura non supportata: $(uname -m)" >&2
    exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -fsSL -o "$TMP/argus-get" "$BASE/$BIN"
curl -fsSL -o "$TMP/argus-get.sha256" "$BASE/$BIN.sha256"
(cd "$TMP" && echo "$(cut -d' ' -f1 argus-get.sha256)  argus-get" | sha256sum -c --quiet -)
chmod +x "$TMP/argus-get"

# stdin e' la pipe di curl: il pannello ha bisogno del terminale
"$TMP/argus-get" "$@" </dev/tty
