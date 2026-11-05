//! Actualización de la aplicación desde las Releases de GitHub.
//!
//! Los archivos publicados van firmados con zipsign (ver `.github/workflows/release.yml`) y aquí solo
//! se instalan si su firma coincide con la clave pública incluida en el ejecutable.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use eframe::egui;
use self_update::VerifyingKey;
use self_update::backends::github::{Update, UpdateBuilder};

use crate::worker::Worker;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const REPO_OWNER: &str = "Kallox";
const REPO_NAME: &str = "git-exile";
const BIN_NAME: &str = "git-exile";
const GITHUB_API: &str = "https://api.github.com";
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// Se aplica a cada petición, incluida la descarga completa.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
const VERIFYING_KEY: VerifyingKey = *include_bytes!("../assets/update-signing.pub");

/// Parte del nombre del archivo publicado para esta plataforma.
const TARGET: Option<&str> = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x86_64")
} else if cfg!(all(windows, target_arch = "x86_64")) {
    Some("windows-x86_64")
} else {
    None
};

/// Ejecutable al que cambiar cuando se cierre la aplicación.
static RELAUNCH: OnceLock<PathBuf> = OnceLock::new();

/// Página de una versión, para leer sus novedades o descargarla a mano.
pub fn release_url(version: &str) -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/tag/v{version}")
}

/// Abre el ejecutable nuevo si se pidió reiniciar. Se llama con la ventana ya cerrada, para que la
/// nueva instancia lea las pestañas que acaban de guardarse.
pub fn relaunch_if_requested() {
    if let Some(exe) = RELAUNCH.get()
        && let Err(e) = std::process::Command::new(exe).spawn()
    {
        eprintln!("No se pudo abrir {}: {e}", exe.display());
    }
}

/// De dónde salen las versiones y con qué claves se verifican.
#[derive(Clone)]
struct Source {
    api: String,
    target: String,
    keys: Vec<VerifyingKey>,
}

impl Source {
    fn github() -> Option<Self> {
        Some(Self {
            api: GITHUB_API.to_owned(),
            target: TARGET?.to_owned(),
            keys: vec![VERIFYING_KEY],
        })
    }

    fn builder(&self, timeout: Duration) -> UpdateBuilder {
        let mut builder = Update::configure();
        builder
            .api_base_url(&self.api)
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name(BIN_NAME)
            .target(&self.target)
            .current_version(CURRENT_VERSION)
            .verifying_keys(self.keys.clone())
            .timeout(timeout)
            .show_output(false)
            .show_download_progress(false)
            .no_confirm(true);
        builder
    }

    /// Devuelve la última versión publicada si es posterior a `current`.
    fn newer_version(&self, current: &str) -> self_update::Result<Option<String>> {
        let updater = self.builder(CHECK_TIMEOUT).build()?;
        let releases = updater.get_latest_release()?;
        let Some(latest) = releases.into_vec().into_iter().next() else {
            return Ok(None);
        };
        let newer = self_update::version::bump_is_greater(current, latest.version())?;
        Ok(newer.then(|| latest.version().to_owned()))
    }

    /// Descarga `version`, verifica su firma y reemplaza el ejecutable `exe`.
    fn install(&self, version: &str, exe: &Path) -> self_update::Result<()> {
        self.builder(DOWNLOAD_TIMEOUT)
            .release_tag(format!("v{version}"))
            .bin_install_path(exe)
            .build()?
            .update()?;
        Ok(())
    }
}

enum Event {
    Checked {
        manual: bool,
        result: Result<Option<String>, String>,
    },
    Installed {
        version: String,
        result: Result<PathBuf, String>,
    },
}

#[derive(Debug, PartialEq)]
pub enum State {
    Idle,
    Checking {
        /// Pedida por el usuario: se informa del resultado aunque no haya novedades.
        manual: bool,
    },
    UpToDate,
    Available {
        version: String,
    },
    Installing {
        version: String,
    },
    /// Ya está en disco, pero sigue en marcha la versión anterior.
    Installed {
        version: String,
        exe: PathBuf,
    },
    Failed {
        message: String,
        /// Versión que se intentaba instalar, para ofrecer su descarga manual.
        version: Option<String>,
    },
}

pub struct Updater {
    worker: Worker<Event>,
    source: Option<Source>,
    state: State,
    /// El aviso de reinicio se cerró; "Buscar actualizaciones" lo recupera.
    hidden: bool,
}

impl Updater {
    pub fn new(ctx: egui::Context) -> Self {
        Self::with_source(ctx, Source::github())
    }

    fn with_source(ctx: egui::Context, source: Option<Source>) -> Self {
        Self {
            worker: Worker::new(ctx),
            source,
            state: State::Idle,
            hidden: false,
        }
    }

    /// `false` en plataformas para las que no se publican ejecutables.
    pub fn is_supported(&self) -> bool {
        self.source.is_some()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    pub fn can_check(&self) -> bool {
        !matches!(
            self.state,
            State::Checking { .. } | State::Installing { .. }
        )
    }

    pub fn check(&mut self, manual: bool) {
        if matches!(self.state, State::Installed { .. }) {
            // No hay nada más que buscar hasta reiniciar.
            self.hidden = false;
            return;
        }
        let Some(source) = self.source.clone() else {
            return;
        };
        if !self.can_check() {
            return;
        }
        self.state = State::Checking { manual };
        self.worker.spawn(move || Event::Checked {
            manual,
            result: source
                .newer_version(CURRENT_VERSION)
                .map_err(|e| e.to_string()),
        });
    }

    pub fn install(&mut self) {
        let State::Available { version } = &self.state else {
            return;
        };
        let Some(source) = self.source.clone() else {
            return;
        };
        let version = version.clone();
        self.state = State::Installing {
            version: version.clone(),
        };
        self.worker.spawn(move || {
            // Se lee antes de reemplazarlo: después, en Linux, la ruta apunta al archivo borrado.
            let result = std::env::current_exe()
                .map_err(|e| e.to_string())
                .and_then(|exe| {
                    source
                        .install(&version, &exe)
                        .map_err(|e| e.to_string())
                        .map(|()| exe)
                });
            Event::Installed { version, result }
        });
    }

    /// Cierra la aplicación y abre la versión instalada.
    pub fn restart(&self, ctx: &egui::Context) {
        if let State::Installed { exe, .. } = &self.state {
            let _ = RELAUNCH.set(exe.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn dismiss(&mut self) {
        match self.state {
            State::Installed { .. } => self.hidden = true,
            State::Installing { .. } => {}
            _ => self.state = State::Idle,
        }
    }

    pub fn poll(&mut self) {
        for event in self.worker.poll() {
            self.state = match event {
                Event::Checked {
                    result: Ok(Some(version)),
                    ..
                } => State::Available { version },
                Event::Checked {
                    manual: true,
                    result: Ok(None),
                } => State::UpToDate,
                Event::Checked {
                    manual: true,
                    result: Err(e),
                } => State::Failed {
                    message: format!("No se pudo buscar actualizaciones: {e}"),
                    version: None,
                },
                // La comprobación al arrancar no molesta si no hay red o no hay novedades.
                Event::Checked { manual: false, .. } => State::Idle,
                Event::Installed {
                    version,
                    result: Ok(exe),
                } => State::Installed { version, exe },
                Event::Installed {
                    version,
                    result: Err(e),
                } => State::Failed {
                    message: format!("No se pudo instalar v{version}: {e}"),
                    version: Some(version),
                },
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    use tempfile::{TempDir, tempfile};
    use zipsign_api::SigningKey;

    use super::*;

    const LINUX_ASSET: &str = "git-exile-linux-x86_64.tar.gz";
    const WINDOWS_ASSET: &str = "git-exile-windows-x86_64.zip";
    const NEW_BINARY: &[u8] = b"binario nuevo";
    const OLD_BINARY: &[u8] = b"binario viejo";

    fn binary_name() -> String {
        format!("{BIN_NAME}{}", std::env::consts::EXE_SUFFIX)
    }

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn tar_gz() -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(NEW_BINARY.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, binary_name(), NEW_BINARY)
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    }

    fn zip_archive() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(binary_name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(NEW_BINARY).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn read_all(mut file: fs::File) -> Vec<u8> {
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut bytes).unwrap();
        bytes
    }

    fn sign_tar(archive: &[u8], key: &SigningKey, name: &str) -> Vec<u8> {
        let mut output = tempfile().unwrap();
        zipsign_api::sign::copy_and_sign_tar(
            &mut Cursor::new(archive),
            &mut output,
            std::slice::from_ref(key),
            Some(name.as_bytes()),
        )
        .unwrap();
        read_all(output)
    }

    fn sign_zip(archive: &[u8], key: &SigningKey, name: &str) -> Vec<u8> {
        let mut output = tempfile().unwrap();
        zipsign_api::sign::copy_and_sign_zip(
            &mut Cursor::new(archive),
            &mut output,
            std::slice::from_ref(key),
            Some(name.as_bytes()),
        )
        .unwrap();
        read_all(output)
    }

    /// Imita la API de GitHub con una única release, la v9.9.9.
    fn serve(asset: &'static str, archive: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let release = format!(
            r#"{{"tag_name":"v9.9.9","name":"v9.9.9","created_at":"2026-01-01T00:00:00Z",
            "body":"Novedades","assets":[{{"name":"{asset}","url":"{base}/asset",
            "browser_download_url":"{base}/asset"}}]}}"#
        );
        let archive = Arc::new(archive);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (release, archive) = (release.clone(), Arc::clone(&archive));
                thread::spawn(move || respond(stream, &release, &archive));
            }
        });
        base
    }

    fn respond(mut stream: TcpStream, release: &str, archive: &[u8]) {
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(n) => request.extend_from_slice(&buffer[..n]),
            }
        }
        let request = String::from_utf8_lossy(&request);
        let path = request.split_whitespace().nth(1).unwrap_or_default();
        let (status, body) = if path == "/asset" {
            ("200 OK", archive)
        } else if path.ends_with("/releases/latest") || path.ends_with("/releases/tags/v9.9.9") {
            ("200 OK", release.as_bytes())
        } else {
            ("404 Not Found", &b"{}"[..])
        };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body);
    }

    fn source(api: String, target: &str, trusted: &SigningKey) -> Source {
        Source {
            api,
            target: target.to_owned(),
            keys: vec![trusted.verifying_key().to_bytes()],
        }
    }

    fn old_exe() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let exe = dir.path().join(binary_name());
        fs::write(&exe, OLD_BINARY).unwrap();
        (dir, exe)
    }

    #[test]
    fn embedded_key_is_a_valid_public_key() {
        let keys = zipsign_api::verify::collect_keys([Ok(VERIFYING_KEY)]);
        assert!(keys.is_ok());
    }

    #[test]
    fn release_url_points_to_the_tag() {
        assert_eq!(
            release_url("1.2.3"),
            "https://github.com/Kallox/git-exile/releases/tag/v1.2.3"
        );
    }

    #[test]
    fn detects_only_newer_versions() {
        let source = source(serve(LINUX_ASSET, Vec::new()), "linux-x86_64", &key(1));
        assert_eq!(
            source.newer_version("0.1.1").unwrap().as_deref(),
            Some("9.9.9")
        );
        assert_eq!(source.newer_version("9.9.9").unwrap(), None);
        assert_eq!(source.newer_version("10.0.0").unwrap(), None);
    }

    #[test]
    fn installs_signed_tar_gz() {
        let signed = sign_tar(&tar_gz(), &key(1), LINUX_ASSET);
        let source = source(serve(LINUX_ASSET, signed), "linux-x86_64", &key(1));
        let (_dir, exe) = old_exe();
        source.install("9.9.9", &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), NEW_BINARY);
    }

    #[test]
    fn installs_signed_zip() {
        let signed = sign_zip(&zip_archive(), &key(1), WINDOWS_ASSET);
        let source = source(serve(WINDOWS_ASSET, signed), "windows-x86_64", &key(1));
        let (_dir, exe) = old_exe();
        source.install("9.9.9", &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), NEW_BINARY);
    }

    fn assert_rejected(asset: &'static str, archive: Vec<u8>, target: &str) {
        let source = source(serve(asset, archive), target, &key(1));
        let (_dir, exe) = old_exe();
        assert!(source.install("9.9.9", &exe).is_err());
        assert_eq!(fs::read(&exe).unwrap(), OLD_BINARY);
    }

    #[test]
    fn rejects_unsigned_archives() {
        assert_rejected(LINUX_ASSET, tar_gz(), "linux-x86_64");
        assert_rejected(WINDOWS_ASSET, zip_archive(), "windows-x86_64");
    }

    #[test]
    fn rejects_archives_signed_with_another_key() {
        let tar = sign_tar(&tar_gz(), &key(2), LINUX_ASSET);
        assert_rejected(LINUX_ASSET, tar, "linux-x86_64");
        let zip = sign_zip(&zip_archive(), &key(2), WINDOWS_ASSET);
        assert_rejected(WINDOWS_ASSET, zip, "windows-x86_64");
    }

    #[test]
    fn rejects_tampered_archives() {
        let mut tar = sign_tar(&tar_gz(), &key(1), LINUX_ASSET);
        tar[20] ^= 0xFF;
        assert_rejected(LINUX_ASSET, tar, "linux-x86_64");
    }

    #[test]
    fn rejects_signatures_made_for_another_file_name() {
        let tar = sign_tar(&tar_gz(), &key(1), "otro-archivo.tar.gz");
        assert_rejected(LINUX_ASSET, tar, "linux-x86_64");
    }

    fn poll_until(updater: &mut Updater, done: impl Fn(&State) -> bool) {
        let start = Instant::now();
        while !done(updater.state()) {
            assert!(start.elapsed() < Duration::from_secs(10), "sin respuesta");
            updater.poll();
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn updater(api: String) -> Updater {
        let source = source(api, "linux-x86_64", &key(1));
        Updater::with_source(egui::Context::default(), Some(source))
    }

    /// Una dirección a la que no responde nadie.
    fn unreachable_api() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    }

    #[test]
    fn startup_check_offers_the_new_version() {
        let mut updater = updater(serve(LINUX_ASSET, Vec::new()));
        updater.check(false);
        poll_until(&mut updater, |s| !matches!(s, State::Checking { .. }));
        let version = "9.9.9".to_owned();
        assert_eq!(*updater.state(), State::Available { version });
    }

    #[test]
    fn startup_check_fails_silently() {
        let mut updater = updater(unreachable_api());
        updater.check(false);
        poll_until(&mut updater, |s| !matches!(s, State::Checking { .. }));
        assert_eq!(*updater.state(), State::Idle);
    }

    #[test]
    fn manual_check_reports_the_failure() {
        let mut updater = updater(unreachable_api());
        updater.check(true);
        poll_until(&mut updater, |s| !matches!(s, State::Checking { .. }));
        assert!(matches!(
            updater.state(),
            State::Failed { version: None, .. }
        ));
    }

    #[test]
    fn a_new_check_only_reveals_a_hidden_restart_notice() {
        let mut updater = updater(unreachable_api());
        updater.state = State::Installed {
            version: "9.9.9".to_owned(),
            exe: PathBuf::from("git-exile"),
        };
        updater.dismiss();
        assert!(updater.is_hidden());
        updater.check(true);
        assert!(!updater.is_hidden());
        assert!(matches!(updater.state(), State::Installed { .. }));
    }

    #[test]
    fn unsupported_platforms_never_check() {
        let mut updater = Updater::with_source(egui::Context::default(), None);
        assert!(!updater.is_supported());
        updater.check(true);
        assert_eq!(*updater.state(), State::Idle);
    }
}
