//! Identidad con la que git crea los commits y perfiles para cambiarla por repositorio.
//!
//! Un perfil se aplica escribiendo en la configuración local del repositorio
//! (`.git/config`), así los commits hechos desde la terminal usan la misma identidad.

use serde::{Deserialize, Serialize};

use super::commit::author_ident;
#[cfg(test)]
use super::remote::Protocol;
use super::remote::{Hosting, Remote};
use super::repo::Repo;
use super::runner::{GitError, Result};

/// Claves que escribe un perfil y que se borran al volver a la identidad global.
const PROFILE_KEYS: [&str; 5] = [
    "user.name",
    "user.email",
    "user.signingkey",
    "gpg.format",
    "commit.gpgsign",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SigningFormat {
    #[default]
    OpenPgp,
    Ssh,
    X509,
}

impl SigningFormat {
    pub const ALL: [SigningFormat; 3] = [Self::OpenPgp, Self::Ssh, Self::X509];

    /// Valor de `gpg.format`.
    fn config_value(self) -> &'static str {
        match self {
            Self::OpenPgp => "openpgp",
            Self::Ssh => "ssh",
            Self::X509 => "x509",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenPgp => "GPG",
            Self::Ssh => "SSH",
            Self::X509 => "X.509",
        }
    }
}

/// Servicio donde se usa un perfil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Service {
    GitHub,
    /// gitlab.com o un GitLab propio (p. ej. el de una institución).
    GitLab,
}

impl Service {
    pub const ALL: [Service; 2] = [Self::GitHub, Self::GitLab];

    pub fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
        }
    }

    fn hosts(self, hosting: &Hosting) -> bool {
        matches!(
            (self, hosting),
            (Self::GitHub, Hosting::GitHub) | (Self::GitLab, Hosting::GitLab)
        )
    }
}

/// Identidad guardada por la aplicación para aplicarla a los repositorios.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub email: String,
    /// Firmar los commits (`commit.gpgsign`).
    pub sign: bool,
    pub format: SigningFormat,
    /// Id de la clave GPG o ruta a la clave SSH. Vacía: git elige la clave del correo.
    pub signing_key: String,
    /// Servicio cuyos repositorios deben usar este perfil: se avisa si usan otra identidad.
    pub service: Option<Service>,
}

impl Profile {
    /// `Nombre <correo>`, como lo muestra git.
    pub fn label(&self) -> String {
        format!("{} <{}>", self.name.trim(), self.email.trim())
    }

    /// Explica por qué no se puede usar el perfil, o `None` si es válido.
    pub fn problem(&self) -> Option<&'static str> {
        let name = self.name.trim();
        let email = self.email.trim();
        let fields = [name, email, self.signing_key.trim()];
        if name.is_empty() {
            Some("Falta el nombre")
        } else if email.is_empty() {
            Some("Falta el correo")
        } else if !email.contains('@') || email.contains(['<', '>', ' ']) {
            Some("El correo no es válido")
        } else if name.contains(['<', '>']) {
            Some("El nombre no puede tener < ni >")
        } else if fields.iter().any(|f| f.contains(['\n', '\r'])) {
            Some("Los campos no pueden tener saltos de línea")
        } else {
            None
        }
    }

    /// `true` si es la identidad `email` (sin distinguir mayúsculas).
    pub fn has_email(&self, email: &str) -> bool {
        self.email.trim().eq_ignore_ascii_case(email.trim())
    }
}

/// Primer perfil del servicio de alguno de los remotos, con su posición en `profiles`.
pub fn suggested_profile<'a>(
    profiles: &'a [Profile],
    remotes: &[Remote],
) -> Option<(usize, &'a Profile)> {
    profiles.iter().enumerate().find(|(_, p)| {
        p.service
            .is_some_and(|s| remotes.iter().any(|r| s.hosts(&r.hosting)))
    })
}

/// Identidad que git usará en el próximo commit y de dónde sale.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Identity {
    /// `Nombre <correo>`; `None` si a git le falta el nombre o el correo.
    pub ident: Option<String>,
    pub email: Option<String>,
    /// Ámbito de `user.email`: `local`, `global`, `system`, `worktree` o `command`.
    pub scope: Option<String>,
    /// Archivo del que sale `user.email` (un `includeIf` muestra el archivo incluido).
    pub origin: Option<String>,
    /// El repositorio define su propia identidad en `.git/config`.
    pub local: bool,
    /// Formato y clave de firma, si los commits se firman.
    pub signing: Option<String>,
}

impl Identity {
    /// Texto corto para mostrar el origen de la identidad.
    pub fn scope_label(&self) -> &str {
        match self.scope.as_deref() {
            Some("local") => "este repositorio",
            Some("worktree") => "este worktree",
            Some("global") if self.is_included() => "regla includeIf",
            Some("global") => "configuración global",
            Some("system") => "configuración del sistema",
            Some("command") => "variable de entorno",
            Some(other) => other,
            None => "sin configurar",
        }
    }

    /// El correo global sale de un archivo incluido, no de `~/.gitconfig`.
    fn is_included(&self) -> bool {
        self.origin.as_deref().is_some_and(|o| {
            let o = o.replace('\\', "/");
            !(o.ends_with("/.gitconfig") || o.ends_with("/git/config"))
        })
    }
}

/// Lee la identidad efectiva del repositorio.
pub fn identity(repo: &Repo) -> Result<Identity> {
    let ident = match author_ident(repo) {
        Ok(ident) => Some(ident),
        // Sin nombre o correo (p. ej. con `user.useConfigOnly`).
        Err(GitError::Failed { .. }) => None,
        Err(e) => return Err(e),
    };
    let mut identity = Identity {
        ident,
        ..Default::default()
    };
    // Formato: `ámbito\torigen\tvalor`.
    if let Some(line) = config_get(
        repo,
        &["--show-scope", "--show-origin", "--get", "user.email"],
    )? {
        let mut parts = line.splitn(3, '\t');
        identity.scope = parts.next().map(str::to_owned);
        identity.origin = parts
            .next()
            .map(|o| o.strip_prefix("file:").unwrap_or(o).to_owned());
        identity.email = parts.next().map(str::to_owned);
    }
    identity.local = config_get(repo, &["--local", "--get", "user.email"])?.is_some()
        || config_get(repo, &["--local", "--get", "user.name"])?.is_some();

    let sign = config_get(repo, &["--type=bool", "--get", "commit.gpgsign"])?;
    if sign.as_deref() == Some("true") {
        let format = config_get(repo, &["--get", "gpg.format"])?;
        let label = SigningFormat::ALL
            .into_iter()
            .find(|f| Some(f.config_value()) == format.as_deref())
            .map_or("GPG", SigningFormat::label);
        let key = config_get(repo, &["--get", "user.signingkey"])?
            .unwrap_or_else(|| "clave del correo".to_owned());
        identity.signing = Some(format!("{label} · {key}"));
    }
    Ok(identity)
}

/// Escribe el perfil en la configuración local del repositorio.
pub fn apply_profile(repo: &Repo, profile: &Profile) -> Result<()> {
    if let Some(problem) = profile.problem() {
        return Err(GitError::Refused(problem.to_owned()));
    }
    set_local(repo, "user.name", profile.name.trim())?;
    set_local(repo, "user.email", profile.email.trim())?;
    // Siempre explícito: si no, la firma global (con la clave de otra identidad) seguiría activa.
    set_local(
        repo,
        "commit.gpgsign",
        if profile.sign { "true" } else { "false" },
    )?;
    let key = profile.signing_key.trim();
    if profile.sign {
        set_local(repo, "gpg.format", profile.format.config_value())?;
        if key.is_empty() {
            unset_local(repo, "user.signingkey")?;
        } else if profile.format == SigningFormat::Ssh {
            set_local(repo, "user.signingkey", &expand_home(key))?;
        } else {
            set_local(repo, "user.signingkey", key)?;
        }
    } else {
        unset_local(repo, "user.signingkey")?;
        unset_local(repo, "gpg.format")?;
    }
    Ok(())
}

/// Borra la identidad local del repositorio para volver a la global (o a sus `includeIf`).
pub fn clear_local_identity(repo: &Repo) -> Result<()> {
    for key in PROFILE_KEYS {
        unset_local(repo, key)?;
    }
    Ok(())
}

/// `git config <args>`; `None` si la clave no existe.
fn config_get(repo: &Repo, args: &[&str]) -> Result<Option<String>> {
    let mut all = vec!["config"];
    all.extend(args);
    match repo.git(&all) {
        Ok(out) => Ok(Some(out.trim_end_matches(['\n', '\r']).to_owned())),
        Err(GitError::Failed { code: 1, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

fn set_local(repo: &Repo, key: &str, value: &str) -> Result<()> {
    repo.git(&["config", "--local", "--", key, value]).map(drop)
}

fn unset_local(repo: &Repo, key: &str) -> Result<()> {
    match repo.git(&["config", "--local", "--unset-all", key]) {
        // 5: la clave no existía.
        Ok(_) | Err(GitError::Failed { code: 5, .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Cambia `~/` por la carpeta personal: la ruta de la clave se guarda tal cual.
fn expand_home(path: &str) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => std::path::Path::new(&home).join(rest).display().to_string(),
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    fn work() -> Profile {
        Profile {
            name: "Ana Torres".into(),
            email: "ana.torres@uni.example.edu".into(),
            service: Some(Service::GitLab),
            ..Default::default()
        }
    }

    fn local(t: &TestRepo, key: &str) -> Option<String> {
        crate::git::run(t.path(), &["config", "--local", "--get", key])
            .ok()
            .map(|v| v.trim().to_owned())
    }

    #[test]
    fn reads_local_identity_and_scope() {
        let t = TestRepo::new();
        let id = identity(&Repo::open(t.path()).unwrap()).unwrap();
        assert_eq!(id.ident.as_deref(), Some("Test <test@example.com>"));
        assert_eq!(id.email.as_deref(), Some("test@example.com"));
        assert_eq!(id.scope.as_deref(), Some("local"));
        assert_eq!(id.scope_label(), "este repositorio");
        assert!(id.local);
        assert_eq!(id.signing, None);
    }

    #[test]
    fn applies_and_clears_a_profile() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        let mut p = work();
        p.sign = true;
        p.format = SigningFormat::Ssh;
        p.signing_key = "/claves/work.pub".into();
        apply_profile(&repo, &p).unwrap();

        let id = identity(&repo).unwrap();
        assert_eq!(
            id.ident.as_deref(),
            Some("Ana Torres <ana.torres@uni.example.edu>")
        );
        assert_eq!(id.signing.as_deref(), Some("SSH · /claves/work.pub"));
        assert_eq!(local(&t, "gpg.format").as_deref(), Some("ssh"));

        // Sin firma se desactiva explícitamente y se quita la clave.
        p.sign = false;
        apply_profile(&repo, &p).unwrap();
        assert_eq!(local(&t, "commit.gpgsign").as_deref(), Some("false"));
        assert_eq!(local(&t, "user.signingkey"), None);
        assert_eq!(identity(&repo).unwrap().signing, None);

        clear_local_identity(&repo).unwrap();
        for key in PROFILE_KEYS {
            assert_eq!(local(&t, key), None, "{key}");
        }
        assert!(!identity(&repo).unwrap().local);
        // Repetirlo sin claves no falla.
        clear_local_identity(&repo).unwrap();
    }

    #[test]
    fn commits_use_the_applied_profile_as_author_and_committer() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        apply_profile(&repo, &work()).unwrap();
        t.commit_file("a.txt", "1\n", "first");
        let who = t.git(&["log", "-1", "--format=%an <%ae>|%cn <%ce>"]);
        assert_eq!(
            who.trim(),
            "Ana Torres <ana.torres@uni.example.edu>|Ana Torres <ana.torres@uni.example.edu>"
        );
    }

    #[test]
    fn rejects_invalid_profiles() {
        let t = TestRepo::new();
        let repo = Repo::open(t.path()).unwrap();
        for bad in [
            Profile {
                email: "".into(),
                ..work()
            },
            Profile {
                email: "sin-arroba".into(),
                ..work()
            },
            Profile {
                name: "A <b>".into(),
                ..work()
            },
            Profile {
                name: "A\nB".into(),
                ..work()
            },
        ] {
            assert!(bad.problem().is_some(), "{bad:?}");
            assert!(apply_profile(&repo, &bad).is_err());
        }
        // No se tocó la identidad del repositorio.
        assert_eq!(local(&t, "user.name").as_deref(), Some("Test"));
    }

    #[test]
    fn suggests_profile_by_remote_service() {
        let personal = Profile {
            name: "Luis".into(),
            email: "luis@example.com".into(),
            service: Some(Service::GitHub),
            ..Default::default()
        };
        let any = Profile {
            service: None,
            ..work()
        };
        let profiles = [any, personal, work()];
        let remote = |hosting| Remote {
            name: "origin".into(),
            url: String::new(),
            hosting,
            protocol: Protocol::Ssh,
        };
        let found = |hosting| suggested_profile(&profiles, &[remote(hosting)]).map(|(i, _)| i);
        assert_eq!(found(Hosting::GitLab), Some(2));
        assert_eq!(found(Hosting::GitHub), Some(1));
        assert_eq!(found(Hosting::Other("git.example.com".into())), None);
        assert_eq!(found(Hosting::Disk), None);
        assert_eq!(suggested_profile(&profiles, &[]), None);
    }

    #[test]
    fn labels_include_rules() {
        let id = |origin: &str| Identity {
            scope: Some("global".into()),
            origin: Some(origin.into()),
            ..Default::default()
        };
        assert_eq!(
            id("/home/a/.gitconfig").scope_label(),
            "configuración global"
        );
        assert_eq!(
            id("/home/a/.config/git/config").scope_label(),
            "configuración global"
        );
        assert_eq!(
            id("/home/a/.gitconfig-work").scope_label(),
            "regla includeIf"
        );
    }
}
