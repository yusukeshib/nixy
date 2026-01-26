use std::fs;
use std::process::Command;

use regex::Regex;

use crate::cli::UninstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::editor::remove_from_section;
use crate::flake::is_nixy_managed;
use crate::profile::get_flake_dir;

use super::info;

pub fn run(config: &Config, args: UninstallArgs) -> Result<()> {
    let package = &args.package;
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    if !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    info(&format!("Uninstalling {}...", package));

    // Remove local package file or flake directory if exists
    let pkg_dir = flake_dir.join("packages");
    let local_pkg_file = pkg_dir.join(format!("{}.nix", package));
    let local_flake_dir = pkg_dir.join(package);

    if local_pkg_file.exists() {
        info(&format!(
            "Removing local package definition: {}",
            local_pkg_file.display()
        ));
        fs::remove_file(&local_pkg_file)?;
        git_rm(&flake_dir, &format!("packages/{}.nix", package));
    } else if local_flake_dir.exists() && local_flake_dir.join("flake.nix").exists() {
        info(&format!(
            "Removing local flake: {}",
            local_flake_dir.display()
        ));
        fs::remove_dir_all(&local_flake_dir)?;
        git_rm_recursive(&flake_dir, &format!("packages/{}", package));
    }

    // Remove from flake.nix
    remove_package_from_flake(config, package)?;

    info("Rebuilding environment...");
    super::sync::run(config)?;

    Ok(())
}

/// Remove a package from flake.nix
fn remove_package_from_flake(config: &Config, pkg: &str) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    if !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    let content = fs::read_to_string(&flake_path)?;

    // Find which input the package uses (for custom packages)
    let input_used = find_input_for_package(&content, pkg);

    // Remove from packages section (standard nixpkgs packages)
    let pkg_pattern = Regex::new(&format!(
        r"^\s*{} = pkgs\.{};",
        regex::escape(pkg),
        regex::escape(pkg)
    ))?;
    let content = remove_from_section(
        &content,
        "# [nixy:packages]",
        "# [/nixy:packages]",
        &pkg_pattern,
    );

    // Remove from env-paths section
    let path_pattern = Regex::new(&format!(r"^\s*{}$", regex::escape(pkg)))?;
    let content = remove_from_section(
        &content,
        "# [nixy:env-paths]",
        "# [/nixy:env-paths]",
        &path_pattern,
    );

    // Remove from custom-packages section (registry packages)
    let custom_pkg_pattern = Regex::new(&format!(r"^\s*{} = inputs\.", regex::escape(pkg)))?;
    let content = remove_from_section(
        &content,
        "# [nixy:custom-packages]",
        "# [/nixy:custom-packages]",
        &custom_pkg_pattern,
    );

    // Remove from custom-paths section
    let content = remove_from_section(
        &content,
        "# [nixy:custom-paths]",
        "# [/nixy:custom-paths]",
        &path_pattern,
    );

    // Remove from local-packages section
    let local_pkg_pattern = Regex::new(&format!(r"^\s*{} = ", regex::escape(pkg)))?;
    let content = remove_from_section(
        &content,
        "# [nixy:local-packages]",
        "# [/nixy:local-packages]",
        &local_pkg_pattern,
    );

    // If we found an input that was used by this package, check if it's still needed
    let content = if let Some(input_name) = input_used {
        remove_unused_input(&content, &input_name)
    } else {
        content
    };

    // Check for overlay-based packages (naming convention: {pkg}-overlay).
    // Note: This heuristic only handles overlays named exactly "{pkg}-overlay".
    // For custom overlay naming (e.g., "neovim" from "neovim-nightly-overlay"),
    // the cleanup via find_input_for_package handles the custom-inputs section.
    let overlay_input_name = format!("{}-overlay", pkg);
    let content = remove_unused_overlay(&content, &overlay_input_name);

    fs::write(&flake_path, content)?;
    super::success(&format!("Removed {} from flake.nix", pkg));

    Ok(())
}

/// Find which input a custom package uses
fn find_input_for_package(content: &str, pkg: &str) -> Option<String> {
    // Pattern: pkg = inputs.INPUT_NAME.packages...
    let pattern = Regex::new(&format!(
        r"^\s*{} = inputs\.([a-zA-Z0-9_-]+)\.",
        regex::escape(pkg)
    ))
    .ok()?;

    for line in content.lines() {
        if let Some(caps) = pattern.captures(line) {
            return Some(caps[1].to_string());
        }
    }
    None
}

/// Remove an input from custom-inputs if no packages use it anymore
fn remove_unused_input(content: &str, input_name: &str) -> String {
    // Check if any package still references this input
    let usage_pattern = format!(r"inputs\.{}\.", regex::escape(input_name));
    if Regex::new(&usage_pattern)
        .map(|r| r.is_match(content))
        .unwrap_or(false)
    {
        // Input is still used
        return content.to_string();
    }

    // Remove the input from custom-inputs section
    let input_pattern = Regex::new(&format!(r"^\s*{}\.url = ", regex::escape(input_name))).unwrap();
    remove_from_section(
        content,
        "# [nixy:custom-inputs]",
        "# [/nixy:custom-inputs]",
        &input_pattern,
    )
}

/// Remove an overlay-based input if it's no longer used
/// This handles packages installed via overlays (e.g., neovim-nightly-overlay)
fn remove_unused_overlay(content: &str, overlay_name: &str) -> String {
    // Check if the overlay input exists in local-inputs
    let input_pattern = format!(r"{}\.url\s*=", regex::escape(overlay_name));
    if !Regex::new(&input_pattern)
        .map(|r| r.is_match(content))
        .unwrap_or(false)
    {
        // Overlay input doesn't exist, nothing to clean up
        return content.to_string();
    }

    // First, remove the overlay from local-overlays section
    let overlay_pattern = Regex::new(&format!(
        r"^\s*{}\.overlays\.[a-zA-Z0-9_-]+",
        regex::escape(overlay_name)
    ))
    .unwrap();
    let content = remove_from_section(
        content,
        "# [nixy:local-overlays]",
        "# [/nixy:local-overlays]",
        &overlay_pattern,
    );

    // Now check if any references to the overlay still exist
    // (could be used by other packages or overlays)
    let overlay_usage = format!(r"{}\.overlays", regex::escape(overlay_name));
    if Regex::new(&overlay_usage)
        .map(|r| r.is_match(&content))
        .unwrap_or(false)
    {
        // Overlay is still referenced elsewhere, don't remove the input
        return content;
    }

    // No more references, safe to remove the input from local-inputs
    let input_pattern =
        Regex::new(&format!(r"^\s*{}\.url = ", regex::escape(overlay_name))).unwrap();
    let content = remove_from_section(
        &content,
        "# [nixy:local-inputs]",
        "# [/nixy:local-inputs]",
        &input_pattern,
    );

    // Update the outputs function signature to remove the overlay input
    remove_from_outputs_signature(&content, overlay_name)
}

/// Remove an input from the outputs function signature
fn remove_from_outputs_signature(content: &str, input_name: &str) -> String {
    // Pattern (bounded): outputs = { self, nixpkgs, ..., input_name, ... }@inputs:
    // Only modify the parameter list inside the outputs signature, not the entire file.
    //
    // Capture groups:
    // 1: "outputs = {"
    // 2: parameter list contents
    // 3: "}@inputs:"
    let sig_re = match Regex::new(r"(?s)(outputs\s*=\s*\{)([^}]*)\}(@inputs\s*:)") {
        Ok(r) => r,
        Err(_) => return content.to_string(),
    };

    let caps = match sig_re.captures(content) {
        Some(c) => c,
        None => return content.to_string(),
    };

    let full_match = caps.get(0).unwrap();
    let before_brace = caps.get(1).unwrap().as_str();
    let params = caps.get(2).unwrap().as_str();
    let after_sig = caps.get(3).unwrap().as_str();

    // Within the parameter list, remove entries that match `input_name` (ignoring surrounding
    // whitespace), then rebuild the list with normalized commas and spacing.
    let mut kept_params: Vec<String> = Vec::new();
    for raw_part in params.split(',') {
        let trimmed = raw_part.trim();

        // Skip empty segments and the one matching `input_name`.
        if trimmed.is_empty() || trimmed == input_name {
            continue;
        }

        kept_params.push(trimmed.to_string());
    }

    let new_params = kept_params.join(", ");

    // Reconstruct the content with the updated outputs signature.
    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..full_match.start()]);
    result.push_str(before_brace);
    result.push_str(&new_params);
    result.push('}');
    result.push_str(after_sig);
    result.push_str(&content[full_match.end()..]);

    result
}

/// Remove a file from git index
fn git_rm(dir: &std::path::Path, file: &str) {
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rm", "--cached", file])
            .output();
    }
}

/// Remove a directory from git index recursively
fn git_rm_recursive(dir: &std::path::Path, path: &str) {
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rm", "-r", "--cached", path])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_input_for_package() {
        let content = r#"
# [nixy:custom-packages]
          neovim = inputs.github-nix-community-neovim-nightly-overlay.packages.${system}.neovim;
# [/nixy:custom-packages]
"#;
        let result = find_input_for_package(content, "neovim");
        assert_eq!(
            result,
            Some("github-nix-community-neovim-nightly-overlay".to_string())
        );
    }

    #[test]
    fn test_find_input_for_package_not_found() {
        let content = r#"
# [nixy:packages]
          hello = pkgs.hello;
# [/nixy:packages]
"#;
        let result = find_input_for_package(content, "hello");
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_input_for_package_different_package() {
        let content = r#"
# [nixy:custom-packages]
          neovim = inputs.neovim-overlay.packages.${system}.neovim;
# [/nixy:custom-packages]
"#;
        let result = find_input_for_package(content, "vim");
        assert_eq!(result, None);
    }

    #[test]
    fn test_remove_unused_input_removes_when_not_used() {
        let content = r#"# [nixy:custom-inputs]
    neovim-overlay.url = "github:nix-community/neovim-nightly-overlay";
# [/nixy:custom-inputs]
# [nixy:custom-packages]
# [/nixy:custom-packages]
"#;
        let result = remove_unused_input(content, "neovim-overlay");
        assert!(!result.contains("neovim-overlay.url"));
    }

    #[test]
    fn test_remove_unused_input_keeps_when_still_used() {
        let content = r#"# [nixy:custom-inputs]
    neovim-overlay.url = "github:nix-community/neovim-nightly-overlay";
# [/nixy:custom-inputs]
# [nixy:custom-packages]
          neovim = inputs.neovim-overlay.packages.${system}.neovim;
# [/nixy:custom-packages]
"#;
        let result = remove_unused_input(content, "neovim-overlay");
        assert!(result.contains("neovim-overlay.url"));
    }

    #[test]
    fn test_remove_unused_input_keeps_other_inputs() {
        let content = r#"# [nixy:custom-inputs]
    neovim-overlay.url = "github:nix-community/neovim-nightly-overlay";
    other-input.url = "github:foo/bar";
# [/nixy:custom-inputs]
# [nixy:custom-packages]
          foo = inputs.other-input.packages.${system}.foo;
# [/nixy:custom-packages]
"#;
        let result = remove_unused_input(content, "neovim-overlay");
        assert!(!result.contains("neovim-overlay.url"));
        assert!(result.contains("other-input.url"));
    }

    #[test]
    fn test_remove_unused_overlay() {
        // When overlay is only used once, removing it should also remove the input
        // and clean up the outputs signature
        let content = r#"outputs = { self, nixpkgs, neovim-nightly-overlay, ... }@inputs:
# [nixy:local-inputs]
    neovim-nightly-overlay.url = "github:nix-community/neovim-nightly-overlay";
# [/nixy:local-inputs]
        overlays = [
          # [nixy:local-overlays]
          neovim-nightly-overlay.overlays.default
          # [/nixy:local-overlays]
        ];
"#;
        let result = remove_unused_overlay(content, "neovim-nightly-overlay");
        // Overlay reference should be removed from local-overlays
        assert!(!result.contains("neovim-nightly-overlay.overlays"));
        // Input should also be removed since no other references exist
        assert!(!result.contains("neovim-nightly-overlay.url"));
        // Outputs signature should also be cleaned up - neovim-nightly-overlay removed
        assert!(!result.contains("neovim-nightly-overlay"));
        // Still contains self, nixpkgs, and ...
        assert!(result.contains("self, nixpkgs, ..."));
        assert!(result.contains("}@inputs:"));
    }

    #[test]
    fn test_remove_unused_overlay_keeps_input_if_still_used() {
        // If overlay is used elsewhere, input should be kept
        let content = r#"# [nixy:local-inputs]
    neovim-nightly-overlay.url = "github:nix-community/neovim-nightly-overlay";
# [/nixy:local-inputs]
        overlays = [
          # [nixy:local-overlays]
          neovim-nightly-overlay.overlays.default
          # [/nixy:local-overlays]
        ];
        # Another reference outside local-overlays
        other-pkg = neovim-nightly-overlay.overlays.something;
"#;
        let result = remove_unused_overlay(content, "neovim-nightly-overlay");
        // Overlay reference should be removed from local-overlays section
        assert!(!result.contains("          neovim-nightly-overlay.overlays.default"));
        // But input should be kept because another reference exists
        assert!(result.contains("neovim-nightly-overlay.url"));
    }

    #[test]
    fn test_remove_unused_overlay_no_overlay() {
        let content = r#"# [nixy:local-inputs]
    gke-plugin.url = "path:./packages/gke";
# [/nixy:local-inputs]
"#;
        let result = remove_unused_overlay(content, "neovim-nightly-overlay");
        // Should return unchanged content
        assert_eq!(result, content);
    }

    #[test]
    fn test_remove_from_outputs_signature() {
        let content = r#"outputs = { self, nixpkgs, gke-plugin, neovim-nightly-overlay }@inputs:"#;
        let result = remove_from_outputs_signature(content, "neovim-nightly-overlay");
        assert!(!result.contains("neovim-nightly-overlay"));
        assert!(result.contains("gke-plugin"));
        assert!(result.contains("nixpkgs"));
    }

    #[test]
    fn test_remove_from_outputs_signature_middle() {
        let content = r#"outputs = { self, nixpkgs, neovim-nightly-overlay, gke-plugin }@inputs:"#;
        let result = remove_from_outputs_signature(content, "neovim-nightly-overlay");
        assert!(!result.contains("neovim-nightly-overlay"));
        assert!(result.contains("gke-plugin"));
    }

    #[test]
    fn test_remove_from_outputs_signature_only_custom_input() {
        // Edge case: removing the only custom input (besides self and nixpkgs)
        let content = r#"outputs = { self, nixpkgs, my-input }@inputs:"#;
        let result = remove_from_outputs_signature(content, "my-input");
        assert!(!result.contains("my-input"));
        assert!(result.contains("self"));
        assert!(result.contains("nixpkgs"));
        // Verify no trailing comma before }
        assert!(!result.contains("nixpkgs,}") && !result.contains("nixpkgs, }"));
        assert!(result.contains("}@inputs:"));
    }

    #[test]
    fn test_remove_from_outputs_signature_first_custom() {
        // Edge case: removing first parameter after nixpkgs
        let content = r#"outputs = { self, nixpkgs, first-input, second-input }@inputs:"#;
        let result = remove_from_outputs_signature(content, "first-input");
        assert!(!result.contains("first-input"));
        assert!(result.contains("second-input"));
        assert!(result.contains("self, nixpkgs, second-input"));
    }

    #[test]
    fn test_remove_from_outputs_signature_with_ellipsis() {
        // Test with ... in signature
        let content = r#"outputs = { self, nixpkgs, my-input, ... }@inputs:"#;
        let result = remove_from_outputs_signature(content, "my-input");
        assert!(!result.contains("my-input"));
        assert!(result.contains("..."));
        assert!(result.contains("self, nixpkgs, ..."));
    }
}
