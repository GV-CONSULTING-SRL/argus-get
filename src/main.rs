//! argus-get: installer pubblico di Argus.
//!
//! Non contiene segreti ne' logica del gestionale. Fa autenticare il tecnico
//! su GitHub (device flow di una GitHub App con accesso in sola lettura al
//! repository privato Argus), scarica l'ultima release del pannello,
//! ne verifica lo SHA-256 e lo avvia.

use std::fmt::Display;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "GV-CONSULTING-SRL/Argus";
const ASSET: &str = "argus-x86_64-linux-musl.tar.gz";
const SUMS: &str = "SHA256SUMS";
/// Client ID della GitHub App "Argus Installer": pubblico per definizione,
/// impostato in build con ARGUS_GH_CLIENT_ID.
const CLIENT_ID: &str = match option_env!("ARGUS_GH_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
const BIN_DIR: &str = "/opt/argus/bin";
const LINK: &str = "/usr/local/bin/argus";
const DEFAULT_DIR: &str = "/opt/gv/box";
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

type Result<T> = std::result::Result<T, String>;

fn err<E: Display>(contesto: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{contesto}: {e}")
}

fn main() -> ExitCode {
    let mut dir = DEFAULT_DIR.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dir" => dir = args.next().unwrap_or(dir),
            "-h" | "--help" => {
                println!("argus-get [--dir CARTELLA]\n\nInstalla Argus e avvia l'installazione nella cartella indicata (default {DEFAULT_DIR}).");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("argus-get {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            altro => {
                eprintln!("argomento sconosciuto: {altro}");
                return ExitCode::FAILURE;
            }
        }
    }
    match run(&dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\n❌ {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &str) -> Result<()> {
    println!("\n  Argus · GV Consulting — installer\n");
    check_system()?;
    if CLIENT_ID.is_empty() {
        return Err("build senza ARGUS_GH_CLIENT_ID: usa la release ufficiale da get.gvconsulting.eu".into());
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .user_agent(concat!("argus-get/", env!("CARGO_PKG_VERSION")))
        // il token GitHub non deve seguire i redirect verso lo storage
        .redirect_auth_headers(ureq::RedirectAuthHeaders::Never)
        .build();

    let token = device_flow(&agent)?;
    let gh = GitHub { agent, token };

    println!("🔎 Verifico l'accesso a {REPO} ...");
    let release = gh.latest_release()?;
    println!("📦 Ultima release: {}", release.tag_name);

    let asset = release.asset(ASSET)?;
    let sums = release.asset(SUMS)?;
    println!("⬇️  Scarico {ASSET} ({:.1} MB) ...", asset.size as f64 / 1_048_576.0);
    let pacchetto = gh.download(asset)?;
    let sums = String::from_utf8(gh.download(sums)?).map_err(err("SHA256SUMS non valido"))?;
    verify(&pacchetto, &sums, ASSET)?;
    println!("✅ SHA-256 verificato");

    let bin = install(&pacchetto)?;
    println!("✅ Argus {} installato in {}\n", release.tag_name, bin);

    // da qui prosegue il pannello: stesso processo, stesso terminale
    let e = Command::new(&bin).args(["--dir", dir, "install"]).exec();
    Err(format!("avvio di {bin}: {e}"))
}

fn check_system() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("serve root: wget -qO- https://get.gvconsulting.eu | sudo sh".into());
    }
    if std::env::consts::ARCH != "x86_64" {
        return Err(format!("architettura non supportata: {}", std::env::consts::ARCH));
    }
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let campo = |k: &str| {
        os.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(|v| v.trim_matches('"').to_lowercase())
            .unwrap_or_default()
    };
    let famiglia = format!("{} {}", campo("ID"), campo("ID_LIKE"));
    if !famiglia.contains("debian") && !famiglia.contains("ubuntu") {
        return Err(format!("sistema non supportato ({}): servono Debian o Ubuntu", campo("PRETTY_NAME")));
    }
    Ok(())
}

// --- GitHub device flow -------------------------------------------------------

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

fn post_form<T: serde::de::DeserializeOwned>(agent: &ureq::Agent, url: &str, form: &[(&str, &str)]) -> Result<T> {
    agent
        .post(url)
        .set("Accept", "application/json")
        .send_form(form)
        .map_err(err("GitHub non raggiungibile"))?
        .into_json()
        .map_err(err("risposta GitHub non valida"))
}

fn device_flow(agent: &ureq::Agent) -> Result<String> {
    let dc: DeviceCode =
        post_form(agent, "https://github.com/login/device/code", &[("client_id", CLIENT_ID)])?;

    println!("🔐 Accesso con il tuo account GitHub GV Consulting");
    println!();
    println!("   1. Apri    {}", dc.verification_uri);
    println!("   2. Inserisci il codice:   {}", dc.user_code);
    println!();
    print!("   In attesa dell'autorizzazione ");
    let _ = std::io::stdout().flush();

    let scadenza = Instant::now() + Duration::from_secs(dc.expires_in);
    let mut intervallo = dc.interval.max(5);
    loop {
        if Instant::now() >= scadenza {
            println!();
            return Err("codice scaduto: rilancia l'installer".into());
        }
        sleep(Duration::from_secs(intervallo));
        let r: TokenReply = post_form(
            agent,
            "https://github.com/login/oauth/access_token",
            &[
                ("client_id", CLIENT_ID),
                ("device_code", &dc.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )?;
        if let Some(t) = r.access_token {
            println!("✅\n");
            return Ok(t);
        }
        match r.error.as_deref() {
            Some("authorization_pending") => {
                print!(".");
                let _ = std::io::stdout().flush();
            }
            Some("slow_down") => intervallo = r.interval.unwrap_or(intervallo + 5),
            Some("expired_token") => return Err("\ncodice scaduto: rilancia l'installer".into()),
            Some("access_denied") => return Err("\nautorizzazione rifiutata".into()),
            Some(e) => {
                return Err(format!("\nerrore GitHub: {e} {}", r.error_description.unwrap_or_default()))
            }
            None => return Err("\nrisposta GitHub inattesa".into()),
        }
    }
}

// --- Release ------------------------------------------------------------------

struct GitHub {
    agent: ureq::Agent,
    token: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    url: String,
    size: u64,
}

impl Release {
    fn asset(&self, nome: &str) -> Result<&Asset> {
        self.assets
            .iter()
            .find(|a| a.name == nome)
            .ok_or_else(|| format!("la release {} non contiene {nome}", self.tag_name))
    }
}

impl GitHub {
    #[allow(clippy::result_large_err)] // ureq::Error, gestito subito dal chiamante
    fn get(&self, url: &str, accept: &str) -> std::result::Result<ureq::Response, ureq::Error> {
        self.agent
            .get(url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", accept)
            .set("X-GitHub-Api-Version", "2022-11-28")
            .call()
    }

    fn latest_release(&self) -> Result<Release> {
        match self.get(&format!("https://api.github.com/repos/{REPO}"), "application/vnd.github+json") {
            Ok(_) => {}
            Err(ureq::Error::Status(401 | 403 | 404, _)) => {
                return Err(format!(
                    "questo account GitHub non ha accesso a {REPO}.\n   Chiedi l'accesso a GV Consulting oppure accedi con un altro account."
                ))
            }
            Err(e) => return Err(format!("GitHub: {e}")),
        }
        match self.get(&format!("https://api.github.com/repos/{REPO}/releases/latest"), "application/vnd.github+json") {
            Ok(r) => r.into_json().map_err(err("release non valida")),
            Err(ureq::Error::Status(404, _)) => Err(format!("nessuna release pubblicata su {REPO}")),
            Err(e) => Err(format!("GitHub: {e}")),
        }
    }

    fn download(&self, a: &Asset) -> Result<Vec<u8>> {
        let r = self.get(&a.url, "application/octet-stream").map_err(err(&a.name))?;
        let mut buf = Vec::with_capacity(a.size as usize);
        r.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut buf).map_err(err(&a.name))?;
        if buf.len() as u64 != a.size {
            return Err(format!("{}: scaricati {} byte su {}", a.name, buf.len(), a.size));
        }
        Ok(buf)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Confronta lo SHA-256 con la riga `<hex>  <nome>` di SHA256SUMS.
fn verify(dati: &[u8], sums: &str, nome: &str) -> Result<()> {
    let atteso = sums
        .lines()
        .find_map(|l| {
            let mut p = l.split_whitespace();
            let h = p.next()?;
            (p.next()?.trim_start_matches('*') == nome).then(|| h.to_lowercase())
        })
        .ok_or_else(|| format!("{nome} non presente in SHA256SUMS"))?;
    let calcolato = hex(&Sha256::digest(dati));
    if calcolato != atteso {
        return Err(format!("SHA-256 non corrispondente per {nome}: pacchetto rifiutato"));
    }
    Ok(())
}

/// Estrae `argus` dal pacchetto in /opt/argus/bin e crea /usr/local/bin/argus.
fn install(pacchetto: &[u8]) -> Result<String> {
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(pacchetto));
    let mut binario = None;
    for e in ar.entries().map_err(err("pacchetto non valido"))? {
        let mut e = e.map_err(err("pacchetto non valido"))?;
        let nome = e.path().map_err(err("pacchetto non valido"))?.to_path_buf();
        if nome.file_name().is_some_and(|n| n == "argus") {
            let mut v = Vec::new();
            e.read_to_end(&mut v).map_err(err("pacchetto non valido"))?;
            binario = Some(v);
        }
    }
    let binario = binario.ok_or("il pacchetto non contiene il binario argus")?;

    fs::create_dir_all(BIN_DIR).map_err(err(BIN_DIR))?;
    let dest = Path::new(BIN_DIR).join("argus");
    let tmp = dest.with_extension("new");
    fs::write(&tmp, binario).map_err(err("scrittura binario"))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(err("permessi binario"))?;
    // rename atomico: un argus in esecuzione non viene corrotto
    fs::rename(&tmp, &dest).map_err(err("installazione binario"))?;

    let _ = fs::remove_file(LINK);
    symlink(&dest, LINK).map_err(err(LINK))?;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifica_sha() {
        let dati = b"ciao";
        let h = hex(&Sha256::digest(dati));
        let sums = format!("deadbeef  altro.tar.gz\n{h}  {ASSET}\n");
        assert!(verify(dati, &sums, ASSET).is_ok());
        assert!(verify(b"alterato", &sums, ASSET).is_err());
        assert!(verify(dati, "x  y\n", ASSET).is_err());
    }
}
