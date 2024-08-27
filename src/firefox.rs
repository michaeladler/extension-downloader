use anyhow::{anyhow, Result};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::path::{Path, PathBuf};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    task::JoinSet,
};
use tracing::{debug, info, warn};

use crate::manifest;

#[derive(Serialize, Deserialize, Debug)]
struct Extension {
    guid: String,
    current_version: Metadata,
}

#[derive(Serialize, Deserialize, Debug)]
struct Metadata {
    version: String,
    files: Vec<Src>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Src {
    url: String,
    hash: String,
}

const DEFAULT_BASE_URL_MOZILLA: &str = "https://services.addons.mozilla.org";

pub async fn install(
    client: ClientWithMiddleware,
    base_url: Option<String>,
    name: String,
    dest_dir: PathBuf,
    profiles: Vec<String>,
) -> Result<()> {
    let xpi_path =
        download_extension(client.clone(), base_url, name.to_string(), &dest_dir).await?;

    let mut set = JoinSet::new();
    for p in profiles {
        set.spawn(install_extension(xpi_path.clone(), p));
    }
    while let Some(res) = set.join_next().await {
        res??;
    }

    Ok(())
}

async fn download_extension(
    client: ClientWithMiddleware,
    base_url: Option<String>,
    name: String,
    dest_dir: &Path,
) -> Result<PathBuf> {
    debug!("Downloading Firefox extension {name}");

    let base_url: String = base_url.unwrap_or(DEFAULT_BASE_URL_MOZILLA.to_string());
    let url = format!("{base_url}/api/v4/addons/addon/{name}/");
    debug!("Fetching metadata from {url}");
    let ext: Extension = client.get(url).send().await?.json().await?;
    debug!("Successfully parsed metadata");

    fs::create_dir_all(&dest_dir).await?;
    let destination = dest_dir.join(format!("{}.xpi", ext.guid));
    let new_version = ext.current_version.version;

    if fs::metadata(&destination).await.is_ok() {
        let mf = manifest::from_file(&destination).await?;
        let old_version = mf.version;
        if old_version == new_version {
            info!(
                "{name} {old_version} already up-to-date ({})",
                dest_dir.to_string_lossy()
            );
            return Ok(destination);
        }
        info!("{name}: updating {old_version} -> {new_version}");
    } else {
        debug!("Downloading Firefox extension {name} {new_version}");
    }

    let url = &ext.current_version.files[0].url;

    debug!("Downloading Firefox extension from {url}");
    let body = client.get(url).send().await?.bytes().await?;

    let mut split = ext.current_version.files[0].hash.split(':');
    let algo = split.next().unwrap();
    if let Some(hash_computed) = compute_hash(algo, &body) {
        debug!("Hash of downloaded file is {hash_computed}");
        let hash_expected = split.next().unwrap();
        if hash_computed != hash_expected {
            return Err(anyhow!(
                "Hash mismatch! Expected {hash_expected}, found {hash_computed}"
            ));
        }
        debug!("Hash verified successfully");
    }

    let mut file = File::create(&destination).await?;
    file.write_all(&body).await?;
    file.flush().await?;
    std::mem::drop(file);

    Ok(destination)
}

async fn install_extension(xpi_file: PathBuf, profile_dir: String) -> Result<()> {
    let ext_dir = PathBuf::from(profile_dir).join("extensions");
    let fname = xpi_file.file_name().unwrap();

    let bak = ext_dir.join(format!("{}.bak", fname.to_str().unwrap()));
    let dst = ext_dir.join(fname);

    // check if xpi file already points to dst
    if let Ok(link) = fs::read_link(&dst).await {
        if link == xpi_file {
            debug!("{:?} is already installed in {:?}", xpi_file, dst);
            return Ok(());
        }
    }

    info!("Installing {:?} as {:?}", xpi_file, dst);
    fs::create_dir_all(&ext_dir).await?;
    create_symlink(&xpi_file, &bak).await?;
    fs::rename(&bak, &dst).await?;
    Ok(())
}

#[cfg(target_os = "windows")]
async fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    fs::symlink_file(src, dst).await?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
async fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    fs::symlink(src, dst).await?;
    Ok(())
}

fn compute_hash(algo: &str, content: &[u8]) -> Option<String> {
    match algo {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(content);
            Some(format!("{:x}", hasher.finalize()))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(content);
            Some(format!("{:x}", hasher.finalize()))
        }
        _ => {
            warn!("Unsupported hash algorithm '{algo}'. Skipping hash verification.");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest_middleware::ClientBuilder;
    use temp_dir::TempDir;

    #[tokio::test]
    async fn test_download_extension_already_exists_same_version() {
        let extension = Extension {
            guid: "123-456-789-0".to_string(),
            current_version: Metadata {
                version: "2.1.2".to_string(),
                files: vec![Src {
                    url: "http://".to_string(),
                    hash: "sha256:dummy".to_string(),
                }],
            },
        };
        let body = serde_json::to_string(&extension).unwrap();

        let mut server = mockito::Server::new_async().await;
        let m1 = server
            .mock("GET", "/api/v4/addons/addon/dummy/")
            .with_header("content-type", "application/json")
            .with_body(&body)
            .with_status(200)
            .create_async()
            .await;

        let client = ClientBuilder::new(reqwest::Client::new()).build();

        let dest_dir = TempDir::new().unwrap();
        // create extension in dest_dir to prevent re-download
        let to = dest_dir.path().join(format!("{}.xpi", extension.guid));
        fs::copy("tests/fixtures/vimium_ff-2.1.2.xpi", to)
            .await
            .unwrap();

        download_extension(
            client,
            Some(server.url()),
            "dummy".to_string(),
            dest_dir.path(),
        )
        .await
        .unwrap();

        m1.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_extension_already_exists_different_version() {
        let mut server = mockito::Server::new_async().await;

        let extension = Extension {
            guid: "123-456-789-0".to_string(),
            current_version: Metadata {
                version: "2.1.0".to_string(), // different from version in xpi file
                files: vec![Src {
                    url: format!("{}/vimium_ff-2.1.2.xpi", server.url()),
                    hash: "sha256:3b9d43ee277ff374e3b1153f97dc20cb06e654116a833674c79b43b8887820e1"
                        .to_string(),
                }],
            },
        };
        let body = serde_json::to_string(&extension).unwrap();

        let m1 = server
            .mock("GET", "/api/v4/addons/addon/dummy/")
            .with_header("content-type", "application/json")
            .with_body(&body)
            .with_status(200)
            .create_async()
            .await;
        let m2 = server
            .mock("GET", "/vimium_ff-2.1.2.xpi")
            .with_header("content-type", "application/x-xpinstall")
            .with_body_from_file("tests/fixtures/vimium_ff-2.1.2.xpi")
            .with_status(200)
            .create_async()
            .await;

        let client = ClientBuilder::new(reqwest::Client::new()).build();

        let dest_dir = TempDir::new().unwrap();
        // create extension in dest_dir to prevent re-download
        let to = dest_dir.path().join(format!("{}.xpi", extension.guid));
        fs::copy("tests/fixtures/vimium_ff-2.1.2.xpi", to)
            .await
            .unwrap();

        download_extension(
            client,
            Some(server.url()),
            "dummy".to_string(),
            dest_dir.path(),
        )
        .await
        .unwrap();

        m1.assert_async().await;
        m2.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_extension_invalid_hash() {
        let mut server = mockito::Server::new_async().await;

        let extension = Extension {
            guid: "123-456-789-0".to_string(),
            current_version: Metadata {
                version: "2.1.0".to_string(), // different from version in xpi file
                files: vec![Src {
                    url: format!("{}/vimium_ff-2.1.2.xpi", server.url()),
                    hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                }],
            },
        };
        let body = serde_json::to_string(&extension).unwrap();

        let m1 = server
            .mock("GET", "/api/v4/addons/addon/dummy/")
            .with_header("content-type", "application/json")
            .with_body(&body)
            .with_status(200)
            .create_async()
            .await;
        let m2 = server
            .mock("GET", "/vimium_ff-2.1.2.xpi")
            .with_header("content-type", "application/x-xpinstall")
            .with_body_from_file("tests/fixtures/vimium_ff-2.1.2.xpi")
            .with_status(200)
            .create_async()
            .await;

        let client = ClientBuilder::new(reqwest::Client::new()).build();

        let dest_dir = TempDir::new().unwrap();

        let result = download_extension(
            client,
            Some(server.url()),
            "dummy".to_string(),
            dest_dir.path(),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), "Hash mismatch! Expected aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, found 3b9d43ee277ff374e3b1153f97dc20cb06e654116a833674c79b43b8887820e1");

        m1.assert_async().await;
        m2.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_extension_no_hash() {
        let mut server = mockito::Server::new_async().await;

        let extension = Extension {
            guid: "123-456-789-0".to_string(),
            current_version: Metadata {
                version: "2.1.0".to_string(), // different from version in xpi file
                files: vec![Src {
                    url: format!("{}/vimium_ff-2.1.2.xpi", server.url()),
                    hash: "custom:xyz".to_string(),
                }],
            },
        };
        let body = serde_json::to_string(&extension).unwrap();

        let m1 = server
            .mock("GET", "/api/v4/addons/addon/dummy/")
            .with_header("content-type", "application/json")
            .with_body(&body)
            .with_status(200)
            .create_async()
            .await;
        let m2 = server
            .mock("GET", "/vimium_ff-2.1.2.xpi")
            .with_header("content-type", "application/x-xpinstall")
            .with_body_from_file("tests/fixtures/vimium_ff-2.1.2.xpi")
            .with_status(200)
            .create_async()
            .await;

        let client = ClientBuilder::new(reqwest::Client::new()).build();

        let dest_dir = TempDir::new().unwrap();

        let result = download_extension(
            client,
            Some(server.url()),
            "dummy".to_string(),
            dest_dir.path(),
        )
        .await;
        assert!(result.is_ok());

        m1.assert_async().await;
        m2.assert_async().await;
    }

    #[test]
    fn test_compute_hash_sha256() {
        let value = compute_hash("sha256", b"hello world").unwrap();
        assert_eq!(
            value,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_hash_sha512() {
        let value = compute_hash("sha512", b"hello world").unwrap();
        assert_eq!(
            value,
            "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
        );
    }

    #[test]
    fn test_compute_hash_unsupported() {
        assert!(compute_hash("unsupported", b"hello world").is_none())
    }
}
