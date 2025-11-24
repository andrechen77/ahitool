use std::env;

use http::header::USER_AGENT;
use tracing::info;

pub const GITHUB_REPO: &str = "andrechen77/ahitool";

const USER_AGENT_VALUE: &str = "andrechen77/ahitool";

/// The name of the asset to download.
#[cfg(target_os = "windows")]
const ASSET_NAME: Option<&str> = Some("ahitool-win.exe");

/// The name of the asset to download.
#[cfg(target_os = "linux")]
const ASSET_NAME: Option<&str> = Some("ahitool-linux");

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const ASSET_NAME: Option<&str> = Some("ahitool-macos");

/// The name of the asset to download.
#[cfg(not(any(target_os = "windows", target_os = "linux", all(target_os = "macos", target_arch = "aarch64"))))]
const ASSET_NAME: Option<&str> = None;

pub fn update_executable(github_repo: &str) -> anyhow::Result<()> {
    let Some(asset_name) = ASSET_NAME else {
        anyhow::bail!(
            "unsupported platform; I don't know how to download assets for this platform"
        );
    };

    let api_url = format!("https://api.github.com/repos/{}/releases/latest", github_repo);

    info!("Checking for updates at {}", api_url);
    let response: serde_json::Value =
        ureq::get(&api_url).set(USER_AGENT.as_str(), USER_AGENT_VALUE).call()?.into_json()?;

    let version_tag =
        response["tag_name"].as_str().ok_or(anyhow::anyhow!("no tag_name found in release"))?;
    info!("Latest version is {}", version_tag);

    let asset_url = response["assets"]
        .as_array()
        .ok_or(anyhow::anyhow!("No assets found in release"))?
        .iter()
        .find_map(|asset| {
            let name = asset["name"].as_str()?;
            if name == asset_name {
                asset["browser_download_url"].as_str()
            } else {
                None
            }
        })
        .ok_or(anyhow::anyhow!("no suitable asset found for this platform"))?;

    info!("Downloading asset from {}", asset_url);
    let response = ureq::get(asset_url).set(USER_AGENT.as_str(), USER_AGENT_VALUE).call()?;
    let mut temp_file = tempfile::Builder::new().suffix(".tmp").tempfile()?;
    if let Err(e) = std::io::copy(&mut response.into_reader(), &mut temp_file) {
        return Err(e.into());
    }

    info!("Installing updated version");
    self_replace::self_replace(temp_file.path())?;

    info!("Updated executable to version {}", version_tag);
    Ok(())
}

pub fn restart_self() -> std::io::Result<()> {
    let current_exe = env::current_exe()?;
    let _ = std::process::Command::new(&current_exe).args(env::args().skip(1)).spawn()?;
    std::process::exit(0);
}
