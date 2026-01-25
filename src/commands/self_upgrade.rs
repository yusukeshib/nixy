use crate::error::{Error, Result};

use super::{info, success};

use self_update::backends::github::ReleaseList;
use self_update::self_replace;
use std::fs::File;
use std::io::Write;

pub fn run(force: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    info(&format!("Current version: {}", current_version));

    // Fetch releases from GitHub API
    info("Checking for updates...");
    let releases = ReleaseList::configure()
        .repo_owner("yusukeshib")
        .repo_name("nixy")
        .build()
        .map_err(|e| Error::SelfUpdate(e.to_string()))?
        .fetch()
        .map_err(|e| Error::SelfUpdate(e.to_string()))?;

    // Get latest release and compare versions
    let latest = releases
        .first()
        .ok_or_else(|| Error::SelfUpdate("No releases found".to_string()))?;
    let latest_version = latest.version.trim_start_matches('v');

    info(&format!("Latest version: {}", latest_version));

    if !force && current_version == latest_version {
        success("Already at latest version");
        return Ok(());
    }

    // Determine platform-specific asset name
    let asset_name = get_asset_name()?;
    info(&format!("Looking for asset: {}", asset_name));

    // Find the asset URL in the release
    let asset = latest
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            Error::SelfUpdate(format!(
                "Asset '{}' not found for this platform. Available assets: {}",
                asset_name,
                latest
                    .assets
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

    // Download binary to temp file
    info("Downloading new version...");
    let tmp_path = download_binary(&asset.download_url)?;

    // Replace current executable
    info("Installing update...");
    self_replace::self_replace(&tmp_path).map_err(|e| Error::SelfUpdate(e.to_string()))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);

    success(&format!("Upgraded to {}", latest_version));
    Ok(())
}

fn get_asset_name() -> Result<String> {
    let arch = std::env::consts::ARCH; // "x86_64" or "aarch64"
    let os = std::env::consts::OS; // "linux", "macos"
    let os_name = match os {
        "macos" => "darwin",
        other => other,
    };
    Ok(format!("nixy-{}-{}", arch, os_name))
}

fn download_binary(url: &str) -> Result<std::path::PathBuf> {
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("nixy-update");

    // Create temp file
    let mut tmp_file = File::create(&tmp_path)?;

    // Use self_update's download functionality
    self_update::Download::from_url(url)
        .download_to(&mut tmp_file)
        .map_err(|e| Error::SelfUpdate(e.to_string()))?;

    tmp_file.flush()?;

    // Make the file executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp_path, perms)?;
    }

    Ok(tmp_path)
}
