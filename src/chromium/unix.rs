use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{create_dir_all, try_exists, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info};

use super::crx3;
use crate::manifest::{self, Manifest};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ExternalExt {
    pub external_crx: PathBuf,
    pub external_version: String,
}

const DEFAULT_BASE_URL_GOOGLE: &str = "https://clients2.google.com";

pub async fn install(
    client: ClientWithMiddleware,
    base_url: Option<String>,
    extension_id: String,
    dest_dir: PathBuf,
    profiles: Vec<String>,
) -> Result<Option<PathBuf>> {
    let (ext, manifest) = download_extension(client, base_url, extension_id, &dest_dir).await?;
    for p in profiles {
        let check_result = check_installed(&ext, &p).await?;
        match (check_result.installed, check_result.latest) {
            (true, true) => {
                info!(
                    "{} {} already up-to-date ({})",
                    manifest.name, manifest.version, p
                );
            }
            (true, false) => {
                info!(
                    "upgrading {}: {} -> {} ({})",
                    manifest.name,
                    check_result.ext.unwrap().external_version,
                    manifest.version,
                    p
                );
                install_extension(&ext, &p).await?;
            }
            (false, _) => {
                info!(
                    "installing {} {} into {}",
                    manifest.name, manifest.version, p
                );
                install_extension(&ext, &p).await?;
            }
        }
    }
    Ok(Some(ext.external_crx))
}

/// download_extension downloads a chromium extension from the Chrome Web Store.
///
/// * `client` - A reqwest client with middleware.
/// * `base_url` - Use this to override the default base URL.
/// * `extension_id` - The ID of the extension to download.
/// * `dest_dir` - The directory to save the extension to.
async fn download_extension(
    client: ClientWithMiddleware,
    base_url: Option<String>,
    extension_id: String,
    dest_dir: &Path,
) -> Result<(ExternalExt, Manifest)> {
    debug!("Downloading Chromium extension {extension_id}");

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
    debug!("Downloaded {extension_id} with manifest: {manifest}");
    Ok((
        ExternalExt {
            external_crx: destination,
            external_version: manifest.version.clone(),
        },
        manifest,
    ))
}

#[derive(Debug)]
struct CheckResult {
    installed: bool,
    latest: bool,
    ext: Option<ExternalExt>,
}

/// is_up_to_date checks if the extension is already installed and up-to-date.
async fn check_installed(ext: &ExternalExt, profile_dir: &str) -> Result<CheckResult> {
    let json_path = create_json_path(ext, profile_dir);
    if let Ok(true) = try_exists(&json_path).await {
        let installed = true;
        // parse json file and check if version matches
        let mut json_file = File::open(&json_path).await?;
        let mut contents = Vec::new();
        json_file.read_to_end(&mut contents).await?;
        let installed_ext: ExternalExt = serde_json::from_slice(&contents).unwrap();
        if installed_ext.external_version == ext.external_version {
            return Ok(CheckResult {
                installed,
                latest: true,
                ext: Some(installed_ext),
            });
        } else {
            return Ok(CheckResult {
                installed,
                latest: false,
                ext: Some(installed_ext),
            });
        }
    }
    Ok(CheckResult {
        installed: false,
        latest: false,
        ext: None,
    })
}

async fn install_extension(ext: &ExternalExt, profile_dir: &str) -> Result<()> {
    let path = ext.external_crx.to_str().unwrap();
    debug!("{profile_dir}: installing Chromium extension {path}");

    let json_path = create_json_path(ext, profile_dir);
    let profile_extensions = PathBuf::from(profile_dir).join("External Extensions");
    create_dir_all(&profile_extensions).await?;

    let mut json_file = File::create(&json_path).await?;
    let contents = serde_json::to_vec_pretty(&ext).unwrap();
    json_file.write_all(&contents).await?;
    Ok(())
}

fn create_json_path(ext: &ExternalExt, profile_dir: &str) -> PathBuf {
    let profile_extensions = PathBuf::from(profile_dir).join("External Extensions");
    let mut json_path = profile_extensions.join(ext.external_crx.file_name().unwrap());
    json_path.set_extension("json");
    json_path
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest_middleware::ClientBuilder;
    use temp_dir::TempDir;
    use tokio::{
        fs::{self},
        io::AsyncReadExt,
    };

    #[tokio::test]
    async fn test_chromium() {
        let mut server = mockito::Server::new_async().await;

        let extension_id = "dbepggeogbaibhgnhhndojpepiihcmeb";

        let url = format!("/service/update2/crx?response=redirect&os=linux&arch=x64&os_arch=x86_64&nacl_arch=x86-64&prod=chromium&prodchannel=unknown&prodversion=91.0.4442.4&lang=en-US&acceptformat=crx2,crx3&x=id%3D{extension_id}%26installsource%3Dondemand%26uc");
        let url_str: &str = &url;
        let m1 = server
            .mock("GET", url_str)
            .with_header("content-type", "application/json")
            .with_body_from_file("tests/fixtures/dbepggeogbaibhgnhhndojpepiihcmeb.crx")
            .with_status(200)
            .create_async()
            .await;

        let tmp_dir = TempDir::new().unwrap();
        let dest_dir = tmp_dir.path().join("storage");
        let expected_crx_path = dest_dir.join(format!("{}.crx", extension_id));

        let chromium_profile_missing = tmp_dir.path().join("profile/chromium_missing");

        let chromium_profile_outdated = tmp_dir.path().join("profile/chromium_outdated");
        {
            create_dir_all(chromium_profile_outdated.join("External Extensions"))
                .await
                .unwrap();
            let mut f = File::create(
                chromium_profile_outdated
                    .join("External Extensions")
                    .join(format!("{}.json", extension_id)),
            )
            .await
            .unwrap();
            f.write_all(
                format!(
                    "{{\"external_crx\": \"{}\", \"external_version\":\"0.0.0\"}}",
                    expected_crx_path.to_string_lossy()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        }

        let chromium_profile_up_to_date = tmp_dir.path().join("profile/chromium_up_to_date");
        {
            create_dir_all(chromium_profile_up_to_date.join("External Extensions"))
                .await
                .unwrap();
            let mut f = File::create(
                chromium_profile_up_to_date
                    .join("External Extensions")
                    .join(format!("{}.json", extension_id)),
            )
            .await
            .unwrap();
            f.write_all(
                format!(
                    "{{\"external_crx\": \"{}\", \"external_version\":\"2.1.2\"}}",
                    expected_crx_path.to_string_lossy()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        }

        let all_profiles = vec![
            chromium_profile_missing,
            chromium_profile_outdated,
            chromium_profile_up_to_date,
        ];

        install(
            ClientBuilder::new(reqwest::Client::new()).build(),
            Some(server.url()),
            extension_id.to_string(),
            dest_dir,
            all_profiles
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        )
        .await
        .unwrap();

        // check extension was downloaded
        assert!(fs::metadata(&expected_crx_path).await.unwrap().is_file());

        for profile in all_profiles {
            // check that a symlink was created
            let mut f = fs::File::open(
                profile
                    .join("External Extensions")
                    .join(format!("{}.json", extension_id)),
            )
            .await
            .unwrap();
            let mut content = String::with_capacity(4096);
            f.read_to_string(&mut content).await.unwrap();
            assert!(content.contains(expected_crx_path.to_str().unwrap()));
            assert!(content.contains("2.1.2"));
        }
        m1.assert_async().await;
    }
}
