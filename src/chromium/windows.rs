use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use std::path::PathBuf;
use tracing::{debug, info};

pub async fn install(
    _client: ClientWithMiddleware,
    _base_url: Option<String>,
    extension_id: String,
    _dest_dir: PathBuf,
    profiles: Vec<String>,
) -> Result<Option<PathBuf>> {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let os_arch = std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default();

    for path in profiles {
        let path = if os_arch == "AMD64" {
            format!("Software\\Wow6432Node\\{path}\\Extensions\\{extension_id}")
        } else {
            format!("Software\\{path}\\Extensions\\{extension_id}")
        };

        let (key, disp) = hklm.create_subkey(&path)?;
        if disp == winreg::enums::RegDisposition::REG_CREATED_NEW_KEY {
            key.set_value(
                "update_url",
                &"https://clients2.google.com/service/update2/crx",
            )?;
            info!("Installed extension {extension_id} for {path}");
        } else {
            debug!("Extension {extension_id} already installed for {path}");
        }
    }

    Ok(None)
}
