use anyhow::Result;
use serde::Deserialize;
use std::{io::Cursor, path::Path};
use tokio::{fs::File, io::AsyncReadExt};
use tracing::debug;

#[derive(Debug, Deserialize, PartialEq, Eq)]
/// A (subset of a) browser extension manifest.json.
/// The manifest.json file is the only file that every extension using WebExtension APIs must contain.
/// See https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json
pub struct Manifest {
    pub name: String,
    pub version: String,
}

impl std::fmt::Display for Manifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} v{}", self.name, self.version)
    }
}

pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Manifest> {
    debug!("Parsing manifest {:?}", path.as_ref());
    let mut file = File::open(&path).await?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).await?;
    from_bytes(&contents)
}

pub fn from_bytes(contents: &[u8]) -> Result<Manifest> {
    let cursor = Cursor::new(contents);
    let mut archive = zip::ZipArchive::new(cursor)?;
    let manifest_file = archive.by_name("manifest.json")?;
    let manifest: Manifest = serde_json::from_reader(manifest_file)?;
    debug!("Parsed {:?}", manifest);
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_from_file() {
        let manifest = from_file("tests/fixtures/dbepggeogbaibhgnhhndojpepiihcmeb.crx")
            .await
            .unwrap();
        assert_eq!(
            manifest,
            Manifest {
                name: "Vimium".to_string(),
                version: "2.1.2".to_string()
            }
        );
    }

    #[test]
    fn test_display() {
        let manifest = Manifest {
            name: "Vimium".to_string(),
            version: "2.1.2".to_string(),
        };
        assert_eq!(format!("{manifest}"), "Vimium v2.1.2");
    }
}
