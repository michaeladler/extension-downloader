mod chromium;
mod config;
mod crx3;
mod firefox;
mod manifest;

use anyhow::Result;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::path::Path;
use std::process::ExitCode;
use std::{collections::HashMap, path::PathBuf};
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

use config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    setup_logging(Level::INFO).unwrap();
    let cfg_path = dirs::config_dir()
        .unwrap()
        .join("extension-downloader")
        .join("config.toml");
    let cfg = config::from_file(&cfg_path).await.unwrap();
    let err_count = run(&cfg).await.unwrap();
    ExitCode::from(err_count as u8)
}

fn setup_logging(level: Level) -> Result<()> {
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

async fn run(cfg: &Config) -> Result<i16> {
    // Retry up to 3 times with increasing intervals between attempts.
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    let client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    let mut ext_to_profiles: HashMap<(String, config::BrowserKind), Vec<&PathBuf>> =
        HashMap::with_capacity(128);
    // deduplicate extensions
    for ext in &cfg.extensions {
        for name in &ext.names {
            ext_to_profiles
                .entry((name.to_string(), ext.browser))
                .or_default()
                .push(&ext.profile);
        }
    }

    let extensions_dir: PathBuf = get_extensions_dir(cfg);
    let dest_dir_chromium = extensions_dir.join("chromium");
    let dest_dir_firefox = extensions_dir.join("firefox");

    let mut err_count: i16 = 0;
    for ((name, kind), profiles) in &ext_to_profiles {
        match kind {
            config::BrowserKind::Chromium => {
                if let Err(err) = do_chromium(
                    cfg,
                    client.clone(),
                    name,
                    &dest_dir_chromium,
                    profiles.as_slice(),
                )
                .await
                {
                    error!("Failed to install Chromium extension {name}: {err}");
                    err_count += 1;
                }
            }
            config::BrowserKind::Firefox => {
                if let Err(err) = do_firefox(
                    cfg,
                    client.clone(),
                    name,
                    &dest_dir_firefox,
                    profiles.as_slice(),
                )
                .await
                {
                    error!("Failed to install Firefox extension {name}: {err}");
                    err_count += 1;
                }
            }
        }
    }
    Ok(err_count)
}

async fn do_chromium(
    cfg: &Config,
    client: ClientWithMiddleware,
    name: &str,
    dest_dir: &Path,
    profiles: &[&PathBuf],
) -> Result<()> {
    let crx_path = chromium::download_extension(
        client,
        cfg.base_url_google.clone(),
        name.to_string(),
        dest_dir,
    )
    .await?;
    for p in profiles {
        chromium::install_extension(&crx_path, p).await?;
    }
    Ok(())
}

async fn do_firefox(
    cfg: &Config,
    client: ClientWithMiddleware,
    name: &str,
    dest_dir: &Path,
    profiles: &[&PathBuf],
) -> Result<()> {
    let xpi_path = firefox::download_extension(
        client.clone(),
        cfg.base_url_mozilla.clone(),
        name.to_string(),
        dest_dir,
    )
    .await?;
    for p in profiles {
        firefox::install_extension(&xpi_path, p).await?;
    }
    Ok(())
}

fn get_extensions_dir(cfg: &Config) -> PathBuf {
    match &cfg.extensions_dir {
        Some(dir) => dir.clone(),
        None => dirs::data_dir()
            .unwrap_or(dirs::home_dir().unwrap().join(".local").join("share"))
            .join("extension-downloader"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::Component};
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
        let extensions_dir = tmp_dir.path().join("storage");
        let chromium_profile = tmp_dir.path().join("profile/chromium");
        let cfg = Config {
            base_url_mozilla: None,
            base_url_google: Some(server.url()),
            extensions_dir: Some(extensions_dir.clone()),
            extensions: vec![config::Extension {
                names: vec![extension_id.to_owned()],
                browser: config::BrowserKind::Chromium,
                profile: chromium_profile.clone(),
            }],
        };
        _ = run(&cfg).await;

        // check extension was downloaded
        let crx_file = extensions_dir
            .join("chromium")
            .join(format!("{}.crx", extension_id));
        assert!(fs::metadata(&crx_file).await.unwrap().is_file());

        // check that a symlink was created
        let mut f = fs::File::open(
            chromium_profile
                .join("External Extensions")
                .join(format!("{}.json", extension_id)),
        )
        .await
        .unwrap();
        let mut content = String::with_capacity(4096);
        f.read_to_string(&mut content).await.unwrap();
        let ext = serde_json::from_str::<chromium::ExternalExt>(&content).unwrap();
        assert_eq!(ext.external_crx, crx_file.to_str().unwrap());
        assert_eq!(ext.external_version, "2.1.2");

        m1.assert_async().await;
    }

    #[tokio::test]
    async fn test_firefox() {
        let mut server = mockito::Server::new_async().await;

        let mut f = fs::File::open("tests/fixtures/vimium-ff.body.json")
            .await
            .unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).await.unwrap();
        // adjust download url to use our mock server
        let contents = contents.replace(
            "https://addons.mozilla.org/firefox/downloads",
            &format!("{}/firefox/downloads", server.url()),
        );

        let m1 = server
            .mock("GET", "/api/v4/addons/addon/vimium-ff/")
            .with_header("content-type", "application/json")
            .with_body(&contents)
            .with_status(200)
            .create_async()
            .await;
        let m2 = server
            .mock("GET", "/firefox/downloads/file/4259790/vimium_ff-2.1.2.xpi")
            .with_header("content-type", "application/x-xpinstall")
            .with_body_from_file("tests/fixtures/vimium_ff-2.1.2.xpi")
            .with_status(200)
            .create_async()
            .await;

        let tmp_dir = TempDir::new().unwrap();
        let extensions_dir = tmp_dir.path().join("storage");
        let firefox_profile = tmp_dir.path().join("profile/firefox");
        let cfg = Config {
            base_url_mozilla: Some(server.url()),
            base_url_google: None,
            extensions_dir: Some(extensions_dir.clone()),
            extensions: vec![config::Extension {
                names: vec!["vimium-ff".to_string()],
                browser: config::BrowserKind::Firefox,
                profile: firefox_profile.clone(),
            }],
        };
        _ = run(&cfg).await;

        m1.assert_async().await;
        m2.assert_async().await;

        // check that the file was downloaded
        let mut count: i64 = 0;
        let mut fnames: Vec<PathBuf> = Vec::new();
        let mut read_dir = tokio::fs::read_dir(extensions_dir.join("firefox"))
            .await
            .unwrap();
        while let Some(entry) = read_dir.next_entry().await.unwrap() {
            fnames.push(entry.path());
            count += 1;
        }
        assert_eq!(count, 1);
        assert_eq!(
            fnames
                .first()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            "{d7742d87-e61d-4b78-b8a1-b469842139fa}.xpi"
        );

        // check that a symlink was created
        let metadata = fs::symlink_metadata(
            firefox_profile
                .join("extensions")
                .join("{d7742d87-e61d-4b78-b8a1-b469842139fa}.xpi"),
        )
        .await
        .unwrap();
        assert!(metadata.file_type().is_symlink());
    }

    #[tokio::test]
    async fn test_download_error() {
        let mut server = mockito::Server::new_async().await;

        let mut f = fs::File::open("tests/fixtures/vimium-ff.body.json")
            .await
            .unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).await.unwrap();
        // adjust download url to use our mock server
        let contents = contents.replace(
            "https://addons.mozilla.org/firefox/downloads",
            &format!("{}/firefox/downloads", server.url()),
        );

        let m1 = server
            .mock("GET", "/api/v4/addons/addon/vimium-ff/")
            .with_header("content-type", "application/json")
            .with_body(&contents)
            .with_status(200)
            .create_async()
            .await;
        let m2 = server
            .mock("GET", "/firefox/downloads/file/4259790/vimium_ff-2.1.2.xpi")
            .with_header("content-type", "application/x-xpinstall")
            .with_body("Forbidden")
            .with_status(403)
            .create_async()
            .await;

        let tmp_dir = TempDir::new().unwrap();
        let extensions_dir = tmp_dir.path().join("storage");
        let firefox_profile = tmp_dir.path().join("profile/firefox");
        let cfg = Config {
            base_url_mozilla: Some(server.url()),
            base_url_google: None,
            extensions_dir: Some(extensions_dir.clone()),
            extensions: vec![config::Extension {
                names: vec!["vimium-ff".to_string()],
                browser: config::BrowserKind::Firefox,
                profile: firefox_profile.clone(),
            }],
        };
        let result = run(&cfg).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        m1.assert_async().await;
        m2.assert_async().await;
    }

    #[test]
    fn test_get_extensions_dir() {
        let cfg = Config {
            base_url_mozilla: None,
            base_url_google: None,
            extensions_dir: None,
            extensions: vec![],
        };
        let dir = get_extensions_dir(&cfg);
        let components: Vec<Component> = dir.components().collect();
        assert!(!components.is_empty());
    }

    #[test]
    fn test_main() {
        let tmp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", tmp_dir.path().to_str().unwrap());
        let cfg = Config {
            base_url_mozilla: None,
            base_url_google: None,
            extensions_dir: Some(std::env::temp_dir()),
            extensions: vec![],
        };
        let content = toml::to_string(&cfg).unwrap();
        let config_path = tmp_dir
            .path()
            .join("extension-downloader")
            .join("config.toml");
        {
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            let mut f = std::fs::File::create(config_path).unwrap();
            f.write_all(content.as_bytes()).unwrap();
            f.flush().unwrap();
        }
        _ = main();
    }
}
