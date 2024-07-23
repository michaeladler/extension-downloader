use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

#[derive(Debug, Serialize, Deserialize)]
/// The configuration for the extension manager.
pub struct Config {
    pub base_url_mozilla: Option<String>,
    pub base_url_google: Option<String>,
    /// The directory where the browser extensions are stored.
    pub extensions_dir: Option<PathBuf>,
    /// A list of extensions to install.
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Serialize, Deserialize)]
/// A browser extension to install.
pub struct Extension {
    // The kind of browser to install the extension for.
    pub browser: BrowserKind,
    // Either a file path to the browser profile directory or the Windows registry key.
    pub profile: String,
    // The extensions to install.
    pub names: Vec<String>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
/// The kind of browser to install the extension for.
pub enum BrowserKind {
    Firefox,
    Chromium,
}

pub async fn from_file(path: &Path) -> Result<Config> {
    debug!("Loading config file {:?}", path);
    let contents = fs::read_to_string(path).await?;
    let mut cfg: Config = toml::from_str(&contents)?;

    // expand user
    for ext in cfg.extensions.iter_mut() {
        ext.profile = expand_tilde(&ext.profile);
    }
    debug!("Loaded config: {:?}", cfg);
    Ok(cfg)
}

fn expand_tilde(path: &str) -> String {
    match (path.starts_with("~/"), dirs::home_dir()) {
        (true, Some(home)) => {
            // remove leading tilde and join with home dir
            home.join(&path[2..]).to_string_lossy().into_owned()
        }
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_dir::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_from_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let contents = r#"
            base_url_mozilla = "https://mozilla.org"
            base_url_google = "https://google.com"
            extensions_dir = "/tmp"

            [[extensions]]
            browser = "firefox"
            profile = "/tmp"
            names = ["foo"]
        "#;
        fs::write(&path, contents).await.unwrap();

        let cfg = from_file(&path).await.unwrap();
        assert_eq!(
            cfg.base_url_mozilla,
            Some("https://mozilla.org".to_string())
        );
        assert_eq!(cfg.base_url_google, Some("https://google.com".to_string()));
        assert_eq!(cfg.extensions_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(cfg.extensions.len(), 1);
        assert_eq!(cfg.extensions[0].browser, BrowserKind::Firefox);
        assert_eq!(cfg.extensions[0].profile, "/tmp");
        assert_eq!(cfg.extensions[0].names, vec!["foo".to_string()]);
    }

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let path = "~/foo";
        let expanded = expand_tilde(path);
        assert_eq!(expanded, home.join("foo").to_str().unwrap());
    }
}
