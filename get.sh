#!/bin/sh
# Installer Argus · GV Consulting
#
#   wget -qO- https://get.gvconsulting.eu | sudo sh
#   wget -qO- https://get.gvconsulting.eu | sudo sh -s -- --dir /opt/gv/box
#
# Usa wget (presente nelle installazioni Debian di base); se manca ripiega su curl.
#
# Scarica argus-get (pubblico), ne verifica lo SHA-256 e lo avvia.
# argus-get chiede l'accesso GitHub e scarica il pannello Argus (privato).
set -eu

BASE="${ARGUS_GET_BASE:-https://github.com/GV-CONSULTING-SRL/argus-get/releases/latest/download}"
BIN="argus-get-x86_64-linux-musl"

if [ "$(id -u)" -ne 0 ]; then
    echo "Serve root:  wget -qO- https://get.gvconsulting.eu | sudo sh" >&2
    exit 1
fi
if [ "$(uname -m)" != "x86_64" ]; then
    echo "Architettura non supportata: $(uname -m)" >&2
    exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# scarica $1 in $2
scarica() {
    if command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1"
    elif command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    else
        echo "Servono wget o curl:  apt-get install -y wget" >&2
        exit 1
    fi
}

scarica "$BASE/$BIN" "$TMP/argus-get"
scarica "$BASE/$BIN.sha256" "$TMP/argus-get.sha256"
(cd "$TMP" && echo "$(cut -d' ' -f1 argus-get.sha256)  argus-get" | sha256sum -c --quiet -)
chmod +x "$TMP/argus-get"

# stdin e' la pipe di wget: il pannello ha bisogno del terminale
"$TMP/argus-get" "$@" </dev/tty
