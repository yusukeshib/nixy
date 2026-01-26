use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::NIX_FLAGS;
use crate::error::{Error, Result};

/// Wrapper for Nix command execution
pub struct Nix;

/// Format a path as a flake reference with optional output
/// Handles paths with spaces by using proper escaping
fn flake_ref(path: &Path, output: Option<&str>) -> String {
    let path_str = path.to_string_lossy();
    // URL-encode spaces for nix flake references
    let encoded = path_str.replace(' ', "%20");
    match output {
        Some(out) => format!("{}#{}", encoded, out),
        None => encoded.to_string(),
    }
}

impl Nix {
    /// Check if nix is installed
    pub fn check_installed() -> Result<()> {
        Command::new("nix")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| Error::NixNotInstalled)?;
        Ok(())
    }

    /// Get the current system (e.g., "x86_64-darwin", "aarch64-linux")
    pub fn current_system() -> Result<String> {
        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args([
                "eval",
                "--impure",
                "--expr",
                "builtins.currentSystem",
                "--raw",
            ])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !output.status.success() {
            return Err(Error::NixCommand(
                "Failed to get current system".to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Build a flake and create an out-link
    pub fn build(flake_dir: &Path, output: &str, out_link: &Path) -> Result<()> {
        let ref_str = flake_ref(flake_dir, Some(output));
        let out_link_str = out_link.to_string_lossy();

        let mut cmd = Command::new("nix");
        cmd.args(NIX_FLAGS);
        cmd.env("NIXPKGS_ALLOW_UNFREE", "1");
        cmd.args(["build", &ref_str, "--out-link", &out_link_str, "--impure"]);

        let status = cmd.status().map_err(|e| Error::NixCommand(e.to_string()))?;

        if !status.success() {
            return Err(Error::NixCommand("Failed to build environment".to_string()));
        }

        Ok(())
    }

    /// Evaluate packages from a flake using nix eval
    pub fn eval_packages(flake_dir: &Path) -> Result<Vec<String>> {
        let system = Self::current_system()?;
        let ref_str = flake_ref(flake_dir, Some(&format!("packages.{}", system)));

        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args([
                "eval",
                &ref_str,
                "--apply",
                r#"pkgs: builtins.concatStringsSep "\n" (builtins.filter (n: n != "default") (builtins.attrNames pkgs))"#,
                "--raw",
            ])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !output.status.success() {
            // flake.lock might not exist yet
            return Ok(Vec::new());
        }

        let packages: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        Ok(packages)
    }

    /// Search for packages in nixpkgs (passes through to stdout)
    pub fn search(query: &str) -> Result<()> {
        let status = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["search", "nixpkgs", query])
            .status()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !status.success() {
            return Err(Error::NixCommand("Search failed".to_string()));
        }

        Ok(())
    }

    /// Update flake inputs
    pub fn flake_update(flake_dir: &Path, inputs: &[String]) -> Result<()> {
        let mut cmd = Command::new("nix");
        cmd.args(NIX_FLAGS).arg("flake").arg("update");

        for input in inputs {
            cmd.arg(input);
        }

        cmd.arg("--flake").arg(flake_dir);

        let status = cmd.status().map_err(|e| Error::NixCommand(e.to_string()))?;

        if !status.success() {
            return Err(Error::NixCommand("Failed to update flake".to_string()));
        }

        Ok(())
    }

    /// Update all flake inputs
    pub fn flake_update_all(flake_dir: &Path) -> Result<()> {
        let status = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["flake", "update", "--flake"])
            .arg(flake_dir)
            .status()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !status.success() {
            return Err(Error::NixCommand("Failed to update flake".to_string()));
        }

        Ok(())
    }

    /// Look up a flake URL from the nix registry
    pub fn registry_lookup(name: &str) -> Result<Option<String>> {
        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["registry", "list"])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let target = format!("flake:{}", name);
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == target {
                return Ok(Some(parts[2].to_string()));
            }
        }

        Ok(None)
    }

    /// Validate that a package exists in nixpkgs
    pub fn validate_package(pkg: &str) -> Result<bool> {
        let attr = format!("nixpkgs#{}.type", pkg);

        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["eval", &attr])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("derivation"))
    }

    /// Validate that a package exists in a flake
    /// Returns the output type ("packages" or "legacyPackages") if found
    pub fn validate_flake_package(flake_url: &str, pkg: &str) -> Result<Option<String>> {
        let system = Self::current_system()?;

        // Try packages.<system>.<pkg> first
        let attr = format!("{}#packages.{}.{}.type", flake_url, system, pkg);
        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["eval", &attr])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("derivation")
        {
            return Ok(Some("packages".to_string()));
        }

        // Try legacyPackages.<system>.<pkg>
        let attr = format!("{}#legacyPackages.{}.{}.type", flake_url, system, pkg);
        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args(["eval", &attr])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("derivation")
        {
            return Ok(Some("legacyPackages".to_string()));
        }

        Ok(None)
    }

    /// List packages in a flake
    pub fn list_flake_packages(flake_url: &str, output_type: Option<&str>) -> Result<Vec<String>> {
        let system = Self::current_system()?;

        let candidates = match output_type {
            Some(t) => vec![format!("{}.{}", t, system)],
            None => vec![
                format!("packages.{}", system),
                format!("legacyPackages.{}", system),
            ],
        };

        for attr_path in candidates {
            let attr = format!("{}#{}", flake_url, attr_path);
            let output = Command::new("nix")
                .args(NIX_FLAGS)
                .args([
                    "eval",
                    &attr,
                    "--apply",
                    r#"pkgs: builtins.concatStringsSep "\n" (builtins.attrNames pkgs)"#,
                    "--raw",
                ])
                .output()
                .map_err(|e| Error::NixCommand(e.to_string()))?;

            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(String::from)
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    /// Get flake inputs from flake.lock
    pub fn get_flake_inputs(lock_file: &Path) -> Result<Vec<String>> {
        let lock_path = lock_file.to_string_lossy();
        let expr = format!(
            r#"builtins.attrNames (builtins.fromJSON (builtins.readFile "{}")).nodes.root.inputs"#,
            lock_path
        );

        let output = Command::new("nix")
            .args(NIX_FLAGS)
            .args([
                "eval",
                "--impure",
                "--expr",
                &expr,
                "--apply",
                r#"names: builtins.concatStringsSep "\n" names"#,
                "--raw",
            ])
            .output()
            .map_err(|e| Error::NixCommand(e.to_string()))?;

        if !output.status.success() {
            return Err(Error::InvalidFlakeLock);
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_flake_ref_simple_path() {
        let path = PathBuf::from("/home/user/.config/nixy");
        let result = flake_ref(&path, Some("default"));
        assert_eq!(result, "/home/user/.config/nixy#default");
    }

    #[test]
    fn test_flake_ref_path_with_spaces() {
        // Paths like ~/Library/Application Support/nixy should have spaces encoded
        let path = PathBuf::from("/Users/user/Library/Application Support/nixy");
        let result = flake_ref(&path, Some("default"));
        assert_eq!(
            result,
            "/Users/user/Library/Application%20Support/nixy#default"
        );
    }

    #[test]
    fn test_flake_ref_without_output() {
        let path = PathBuf::from("/home/user/.config/nixy");
        let result = flake_ref(&path, None);
        assert_eq!(result, "/home/user/.config/nixy");
    }

    #[test]
    fn test_flake_ref_with_nested_output() {
        let path = PathBuf::from("/home/user/.config/nixy");
        let result = flake_ref(&path, Some("packages.x86_64-linux"));
        assert_eq!(result, "/home/user/.config/nixy#packages.x86_64-linux");
    }

    #[test]
    fn test_flake_ref_multiple_spaces() {
        let path = PathBuf::from("/tmp/nixy test dir/config");
        let result = flake_ref(&path, Some("default"));
        assert_eq!(result, "/tmp/nixy%20test%20dir/config#default");
    }
}
