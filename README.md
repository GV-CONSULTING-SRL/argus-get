# argus-get

Installer pubblico di **Argus**, il pannello di controllo delle macchine GV Consulting.

```sh
wget -qO- https://get.gvconsulting.eu | sudo sh
```

(funziona anche `curl -fsSL https://get.gvconsulting.eu | sudo sh`)

1. `get.sh` scarica `argus-get` dall'ultima release di questo repository e ne verifica lo SHA-256.
2. `argus-get` chiede l'accesso GitHub con il *device flow* (codice da inserire su
   `github.com/login/device`) tramite la GitHub App **Argus Installer**.
3. Se l'account ha accesso al repository privato `GV-CONSULTING-SRL/Argus`, scarica
   l'ultima release del pannello, ne verifica lo SHA-256, la installa in
   `/opt/argus/bin/argus` (link in `/usr/local/bin/argus`) e avvia `argus install`.

Il token GitHub resta in memoria e non viene salvato sulla macchina.
Questo repository non contiene segreti né logica del gestionale.

Opzioni: `--dir <cartella>` cartella del `docker-compose.yml` (default `/opt/gv/box`).

Requisiti: Debian/Ubuntu x86_64, root.

## Configurazione (una tantum)

- GitHub App *Argus Installer* nell'organizzazione: **Enable Device Flow**, permesso
  repository `Contents: Read-only`, installata **solo** su `Argus`.
- Variabile di repository `ARGUS_GH_CLIENT_ID` = Client ID dell'App.
- `get.gvconsulting.eu` serve `get.sh` (o redirect a
  `https://github.com/GV-CONSULTING-SRL/argus-get/releases/latest/download/get.sh`).

## Release

`git tag v0.1.0 && git push --tags` → la CI compila `argus-get-x86_64-linux-musl`
e pubblica binario, `get.sh` e relativi `.sha256`.

Per aggiornare **solo `get.sh`** senza nuova versione: Actions → *update-installer* →
*Run workflow* (oppure `gh workflow run update-installer.yml -R GV-CONSULTING-SRL/argus-get`).
Sostituisce `get.sh` e `get.sh.sha256` nell'ultima release (o in quella indicata).
