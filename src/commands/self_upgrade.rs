use crate::error::{Error, Result};

use super::{info, success};

use self_update::backends::github::ReleaseList;
use self_update::self_replace;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// RAII guard to ensure temporary update file is cleaned up.
struct TempFileGuard {
    path: PathBuf,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        TempFileGuard { path }
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

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

    // Ensure temp file is cleaned up even if installation fails
    let _tmp_guard = TempFileGuard::new(tmp_path.clone());

    // Replace current executable
    info("Installing update...");
    self_replace::self_replace(&tmp_path).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Permission denied") || err_str.contains("permission denied") {
            Error::SelfUpdate(format!(
                "Permission denied. If nixy is installed in a system directory, try running with elevated privileges (e.g., sudo nixy self-upgrade)"
            ))
        } else {
            Error::SelfUpdate(err_str)
        }
    })?;

    success(&format!("Upgraded to {}", latest_version));
    Ok(())
}

fn get_asset_name() -> Result<String> {
    let arch = std::env::consts::ARCH; // e.g., "x86_64", "aarch64"
    let os = std::env::consts::OS; // e.g., "linux" or "macos" (mapped to "darwin" below)
    let os_name = match os {
        "macos" => "darwin",
        "linux" => "linux",
        other => {
            return Err(Error::SelfUpdate(format!(
                "Self-upgrade is only supported on macOS and Linux (detected platform: '{}')",
                other
            )));
        }
    };
    Ok(format!("nixy-{}-{}", arch, os_name))
}

fn download_binary(url: &str) -> Result<PathBuf> {
    let tmp_dir = std::env::temp_dir();
    // Use unique temp file name to avoid conflicts with concurrent runs
    let pid = std::process::id();
    let tmp_path = tmp_dir.join(format!("nixy-update-{}", pid));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_asset_name_format() {
        let result = get_asset_name();
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(name.starts_with("nixy-"));
        // Should contain arch and os
        assert!(name.contains("-"));
    }

    #[test]
    fn test_temp_file_guard_cleanup() {
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("nixy-test-guard");

        // Create a test file
        std::fs::write(&tmp_path, "test").unwrap();
        assert!(tmp_path.exists());

        // Guard should clean up on drop
        {
            let _guard = TempFileGuard::new(tmp_path.clone());
        }

        assert!(!tmp_path.exists());
    }
}
