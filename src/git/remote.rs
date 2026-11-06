//! Remotos del repositorio y servicio en el que están alojados.

use super::repo::Repo;
use super::runner::{GitError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hosting {
    GitHub,
    GitLab,
    /// Otro servidor, con su nombre.
    Other(String),
    /// Una ruta de este equipo u otra dirección sin servidor.
    Disk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    /// Dirección para mostrar, sin credenciales.
    pub url: String,
    pub hosting: Hosting,
}

impl Remote {
    pub fn hosting_label(&self) -> &str {
        match &self.hosting {
            Hosting::GitHub => "GitHub",
            Hosting::GitLab => "GitLab",
            Hosting::Other(host) => host,
            Hosting::Disk => "Remoto en disco",
        }
    }
}

/// Remotos del repositorio, con `origin` primero. Sin remotos, está solo en local.
pub fn list_remotes(repo: &Repo) -> Result<Vec<Remote>> {
    let mut remotes = parse_remotes(&repo.git(&["remote", "-v"])?);
    remotes.sort_by_key(|r| r.name != "origin");
    Ok(remotes)
}

/// Conecta el repositorio a `url` con el remoto `origin`.
pub fn connect_remote(repo: &Repo, url: &str) -> Result<()> {
    let url = valid_url(url)?;
    repo.git(&["remote", "add", "--", "origin", url]).map(drop)
}

/// Quita los espacios de `url` y la rechaza si no se puede usar como dirección.
pub(super) fn valid_url(url: &str) -> Result<&str> {
    let url = url.trim();
    // Git lo interpretaría como una opción al usar el remoto.
    if url.is_empty() || url.starts_with('-') {
        return Err(GitError::Refused(format!(
            "\"{url}\" no es una dirección válida"
        )));
    }
    Ok(url)
}

/// `git remote -v` da una línea `nombre\turl (fetch)` y otra `(push)` por remoto.
fn parse_remotes(output: &str) -> Vec<Remote> {
    output
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once('\t')?;
            let (url, _) = rest.rsplit_once(" (fetch)")?;
            Some(Remote {
                name: name.to_owned(),
                url: strip_credentials(url),
                hosting: classify(url),
            })
        })
        .collect()
}

fn classify(url: &str) -> Hosting {
    let Some(host) = host(url) else {
        return Hosting::Disk;
    };
    // Por etiquetas: `ssh.github.com`, `gitlab.empresa.com`, alias `github-trabajo`.
    let has = |name: &str| host.split('.').any(|label| label.starts_with(name));
    if has("github") {
        Hosting::GitHub
    } else if has("gitlab") {
        Hosting::GitLab
    } else {
        Hosting::Other(host.to_owned())
    }
}

/// Servidor de una URL (`esquema://...`) o de la forma corta `usuario@host:ruta`.
fn host(url: &str) -> Option<&str> {
    if let Some((scheme, rest)) = url.split_once("://") {
        if scheme == "file" {
            return None;
        }
        let authority = rest.split('/').next()?;
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        return host.split(':').next().filter(|h| !h.is_empty());
    }
    let (start, _) = url.split_once(':')?;
    // Una ruta relativa, o de Windows (`C:\\repos\\b`).
    if start.contains('/') || start.len() == 1 {
        return None;
    }
    let host = start.rsplit_once('@').map_or(start, |(_, h)| h);
    (!host.is_empty()).then_some(host)
}

/// Quita usuario y contraseña (o token) de las URL http(s).
fn strip_credentials(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://")
        && scheme.starts_with("http")
    {
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        if let Some((_, host)) = authority.rsplit_once('@') {
            return format!("{scheme}://{host}{path}");
        }
    }
    url.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testutil::TestRepo;

    #[test]
    fn classifies_hosting_from_url() {
        let other = |h: &str| Hosting::Other(h.to_owned());
        for (url, hosting) in [
            ("https://github.com/a/b.git", Hosting::GitHub),
            ("git@github.com:a/b.git", Hosting::GitHub),
            ("ssh://git@ssh.github.com:443/a/b.git", Hosting::GitHub),
            ("git@github-trabajo:a/b.git", Hosting::GitHub),
            ("https://gitlab.com/a/b", Hosting::GitLab),
            ("git@gitlab.empresa.com:a/b.git", Hosting::GitLab),
            ("https://bitbucket.org/a/b.git", other("bitbucket.org")),
            ("https://git.empresa.com:8443/a/b", other("git.empresa.com")),
            ("/srv/git/b.git", Hosting::Disk),
            ("../b.git", Hosting::Disk),
            ("C:\\repos\\b.git", Hosting::Disk),
            ("D:/repos/b.git", Hosting::Disk),
            ("./con:dos/puntos", Hosting::Disk),
            ("file:///srv/git/b.git", Hosting::Disk),
        ] {
            assert_eq!(classify(url), hosting, "{url}");
        }
    }

    #[test]
    fn hides_credentials_of_http_urls() {
        assert_eq!(
            strip_credentials("https://user:token@github.com/a/b.git"),
            "https://github.com/a/b.git"
        );
        assert_eq!(
            strip_credentials("https://token@gitlab.com"),
            "https://gitlab.com"
        );
        // Una arroba en la ruta no es una credencial.
        assert_eq!(
            strip_credentials("https://example.com/a@b/c.git"),
            "https://example.com/a@b/c.git"
        );
        assert_eq!(
            strip_credentials("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
    }

    #[test]
    fn parses_one_entry_per_remote() {
        let output = "origin\tgit@github.com:a/b.git (fetch)\n\
                      origin\tgit@github.com:a/b.git (push)\n\
                      parcial\thttps://gitlab.com/a/b (fetch) [blob:none]\n\
                      parcial\thttps://gitlab.com/a/b (push)\n";
        let remotes = parse_remotes(output);
        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].hosting_label(), "GitHub");
        assert_eq!(remotes[1].name, "parcial");
        assert_eq!(remotes[1].url, "https://gitlab.com/a/b");
        assert_eq!(remotes[1].hosting_label(), "GitLab");
    }

    #[test]
    fn connects_origin_and_detects_its_hosting() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let repo = Repo::open(t.path()).unwrap();

        connect_remote(&repo, "  git@github.com:a/b.git\n").unwrap();
        let remotes = list_remotes(&repo).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].url, "git@github.com:a/b.git");
        assert_eq!(remotes[0].hosting, Hosting::GitHub);

        // Ya existe: se mantiene el que había.
        assert!(connect_remote(&repo, "https://gitlab.com/a/b.git").is_err());
        assert_eq!(list_remotes(&repo).unwrap()[0].hosting, Hosting::GitHub);
    }

    #[test]
    fn rejects_empty_and_option_like_urls() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let repo = Repo::open(t.path()).unwrap();
        for bad in ["", "   ", "--upload-pack=x"] {
            assert!(connect_remote(&repo, bad).is_err(), "{bad:?}");
        }
        assert!(list_remotes(&repo).unwrap().is_empty());
    }

    #[test]
    fn lists_remotes_with_origin_first() {
        let t = TestRepo::new();
        t.commit_file("a.txt", "1\n", "first");
        let repo = Repo::open(t.path()).unwrap();
        assert!(list_remotes(&repo).unwrap().is_empty());

        t.git(&["remote", "add", "copia", "https://gitlab.com/a/b.git"]);
        t.git(&["remote", "add", "origin", "https://github.com/a/b.git"]);
        let names: Vec<String> = list_remotes(&repo)
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["origin", "copia"]);
    }
}
