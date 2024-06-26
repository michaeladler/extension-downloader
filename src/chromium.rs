use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;
use tracing::info;

use crate::crx3;
use crate::manifest;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalExt {
    pub external_crx: PathBuf,
    pub external_version: String,
}

const DEFAULT_BASE_URL_GOOGLE: &str = "https://clients2.google.com";

/// download_extension downloads a chromium extension from the Chrome Web Store.
///
/// * `client` - A reqwest client with middleware.
/// * `base_url` - Use this to override the default base URL.
/// * `extension_id` - The ID of the extension to download.
/// * `dest_dir` - The directory to save the extension to.
pub async fn download_extension(
    client: ClientWithMiddleware,
    base_url: Option<String>,
    extension_id: String,
    dest_dir: &Path,
) -> Result<ExternalExt> {
    info!("Downloading Chromium extension {extension_id}");

    let base_url: String = base_url.unwrap_or(DEFAULT_BASE_URL_GOOGLE.to_string());
    let body = client.get(format!(
        "{base_url}/service/update2/crx?response=redirect&os=linux&arch=x64&os_arch=x86_64&nacl_arch=x86-64&prod=chromium&prodchannel=unknown&prodversion=91.0.4442.4&lang=en-US&acceptformat=crx2,crx3&x=id%3D{extension_id}%26installsource%3Dondemand%26uc",
    )).send().await?.bytes().await?;

    create_dir_all(&dest_dir).await?;

    let destination = dest_dir.join(format!("{}.crx", extension_id));
    let mut file = File::create(&destination).await?;
    file.write_all(&body).await?;
    file.flush().await?; // ensure file is fully persisted, otherwise install_extension can fail
                         // due to 'early eof'
    std::mem::drop(file); // close file

    let crx_file = crx3::parse_file(&destination).await?;
    let manifest = manifest::from_bytes(&crx_file.zip_archive)?;
    info!(
        "Downloaded extension with id {}: name={} version {}",
        extension_id, manifest.name, manifest.version
    );
    Ok(ExternalExt {
        external_crx: destination,
        external_version: manifest.version.clone(),
    })
}

pub async fn install_extension(ext: &ExternalExt, profile_dir: &Path) -> Result<()> {
    let profile_extensions = profile_dir.join("External Extensions");
    let mut json_path = profile_extensions.join(ext.external_crx.file_name().unwrap());
    json_path.set_extension("json");

    let profile_dir = profile_dir.to_str().unwrap();
    info!(
        "Installing Chromium extension {:?} into profile {}",
        ext.external_crx, profile_dir
    );

    create_dir_all(&profile_extensions).await?;

    let mut json_file = File::create(&json_path).await?;
    let contents = serde_json::to_vec_pretty(&ext).unwrap();
    json_file.write_all(&contents).await?;
    Ok(())
}
