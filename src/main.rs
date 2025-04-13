mod chromium;
mod config;
mod firefox;
mod manifest;

use anyhow::Result;
use dirs::{config_dir, data_dir, home_dir};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::{collections::HashMap, path::PathBuf};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info, Level};
use tracing_subscriber::{fmt::Subscriber as FmtSubscriber, EnvFilter};
use walkdir::WalkDir;

use config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(Level::INFO.as_str())); // default to "info" if RUST_LOG is not set

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");

    let cfg_path = get_config_dir().join("config.toml");
    let start = Instant::now();
    let result = run(&cfg_path).await;
    let duration = start.elapsed();
    info!("Finished in {:?}", duration);
    match result {
        Ok(0) => ExitCode::SUCCESS,
        Ok(err_count) => {
            error!("{} errors occurred", err_count);
            ExitCode::FAILURE
        }
        Err(err) => {
            error!("Fatal Error: {}", err);
            ExitCode::FAILURE
        }
    }
}

async fn run<P: AsRef<Path>>(cfg_path: P) -> Result<u32> {
    if !cfg_path.as_ref().exists() {
        return Err(anyhow::anyhow!(
            "Config file {:?} does not exist",
            cfg_path.as_ref()
        ));
    }
    let cfg = config::from_file(cfg_path.as_ref()).await?;

    // Retry up to 3 times with increasing intervals between attempts.
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    let client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    let mut ext_to_profiles: HashMap<(String, config::BrowserKind), Vec<String>> =
        HashMap::with_capacity(128);
    // deduplicate extensions
    for ext in &cfg.extensions {
        for name in &ext.names {
            ext_to_profiles
                .entry((name.to_string(), ext.browser))
                .or_default()
                .push(ext.profile.clone());
        }
    }

    let extensions_dir: PathBuf = get_extensions_dir(&cfg);
    let dest_dir_chromium = extensions_dir.join("chromium");
    let dest_dir_firefox = extensions_dir.join("firefox");

    let mut set = JoinSet::new();

    let mut err_count = 0;
    for ((name, kind), profiles) in ext_to_profiles.drain() {
        match kind {
            config::BrowserKind::Chromium => {
                set.spawn(chromium::install(
                    client.clone(),
                    cfg.base_url_google.clone(),
                    name,
                    dest_dir_chromium.clone(),
                    profiles,
                ));
            }
            config::BrowserKind::Firefox => {
                set.spawn(firefox::install(
                    client.clone(),
                    cfg.base_url_mozilla.clone(),
                    name,
                    dest_dir_firefox.clone(),
                    profiles,
                ));
            }
        }
    }

    let mut known_files = HashSet::new();
    while let Some(result) = set.join_next().await {
        match result.unwrap() {
            Ok(Some(path)) => {
                known_files.insert(path);
            }
            Err(err) => {
                error!("{}", err);
                err_count += 1;
            }
            _ => {}
        }
    }

    if err_count == 0 {
        for dir in &[dest_dir_chromium, dest_dir_firefox] {
            if !dir.exists() {
                continue;
            }
            for file in WalkDir::new(dir).into_iter().filter_map(|file| file.ok()) {
                if file.metadata().unwrap().is_file() && !known_files.contains(file.path()) {
                    info!("Purging old extension: {:?}", file.path());
                    tokio::fs::remove_file(file.path()).await?;
                }
            }
        }
    }
    Ok(err_count)
}

fn get_extensions_dir(cfg: &Config) -> PathBuf {
    match &cfg.extensions_dir {
        Some(dir) => dir.clone(),
        None => data_dir()
            .unwrap_or(home_dir().unwrap().join(".local").join("share"))
            .join("extension-downloader"),
    }
}

fn get_config_dir() -> PathBuf {
    // prefer env EXTENSION_DOWNLOADER_CONFIG_DIR if set
    env::var("EXTENSION_DOWNLOADER_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir().unwrap().join("extension-downloader"))
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

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn test_chromium() {
        let mut server = mockito::Server::new_async().await;

        let extension_id = "dbepggeogbaibhgnhhndojpepiihcmeb";

        let url = format!("/service/update2/crx?response=redirect&prodversion=119.0.6045.199&acceptformat=crx2,crx3&x=id%3D{extension_id}%26installsource%3Dondemand%26uc");
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
                profile: chromium_profile.to_string_lossy().to_string(),
            }],
        };
        let cfg_path = tmp_dir.path().join("config.toml");
        fs::write(&cfg_path, toml::to_string(&cfg).unwrap())
            .await
            .unwrap();

        let chromium_extensions_dir = extensions_dir.join("chromium");
        // create stale extension
        fs::create_dir_all(&chromium_extensions_dir).await.unwrap();
        let stale_path = chromium_extensions_dir.join("test.crx");
        fs::File::create(&stale_path).await.unwrap();

        _ = run(&cfg_path).await;

        // check that stale file was removed
        assert_eq!(
            fs::metadata(stale_path).await.unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );

        // check extension was downloaded
        let crx_file = chromium_extensions_dir.join(format!("{}.crx", extension_id));
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
        assert!(content.contains(crx_file.to_str().unwrap()));
        assert!(content.contains("2.1.2"));

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
                profile: firefox_profile.to_string_lossy().to_string(),
            }],
        };
        let cfg_path = tmp_dir.path().join("config.toml");
        fs::write(&cfg_path, toml::to_string(&cfg).unwrap())
            .await
            .unwrap();

        // create stale extension
        let ff_extensions_dir = extensions_dir.join("firefox");
        fs::create_dir_all(&ff_extensions_dir).await.unwrap();
        let stale_path = ff_extensions_dir.join("test.crx");
        fs::File::create(&stale_path).await.unwrap();

        _ = run(&cfg_path).await;

        m1.assert_async().await;
        m2.assert_async().await;

        // check that stale file was removed
        assert_eq!(
            fs::metadata(stale_path).await.unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );

        // check that the file was downloaded
        let mut count: i64 = 0;
        let mut fnames: Vec<PathBuf> = Vec::new();
        let mut read_dir = tokio::fs::read_dir(ff_extensions_dir).await.unwrap();
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
                profile: firefox_profile.to_string_lossy().to_string(),
            }],
        };
        let cfg_path = tmp_dir.path().join("config.toml");
        fs::write(&cfg_path, toml::to_string(&cfg).unwrap())
            .await
            .unwrap();

        let result = run(&cfg_path).await;
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
        let config_dir = tmp_dir.path().join("extension-downloader");
        std::env::set_var(
            "EXTENSION_DOWNLOADER_CONFIG_DIR",
            config_dir.to_str().unwrap(),
        );
        let cfg = Config {
            base_url_mozilla: None,
            base_url_google: None,
            extensions_dir: Some(std::env::temp_dir()),
            extensions: vec![],
        };
        let content = toml::to_string(&cfg).unwrap();
        let config_path = config_dir.join("config.toml");
        {
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            let mut f = std::fs::File::create(config_path).unwrap();
            f.write_all(content.as_bytes()).unwrap();
            f.flush().unwrap();
        }
        _ = main();
    }

    #[tokio::test]
    async fn test_run_config_not_found() {
        let err = run("/does/not/exist.toml").await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Config file \"/does/not/exist.toml\" does not exist"
        );
    }
}
