use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;

use crate::cli::InstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::editor::insert_after_marker;
use crate::flake::parser::parse_local_package_attr;
use crate::flake::template::{generate_flake, has_custom_modifications, PreservedContent};
use crate::flake::{is_flake_file, is_nixy_managed};
use crate::nix::Nix;
use crate::profile::get_flake_dir;

use super::{info, success, warn};

pub fn run(config: &Config, args: InstallArgs) -> Result<()> {
    // Handle --file option
    if let Some(file) = args.file {
        return install_from_file(config, &file, args.force);
    }

    // Handle --from option
    if let Some(from) = args.from {
        let pkg = args
            .package
            .ok_or_else(|| Error::Usage("Package name is required with --from".to_string()))?;
        return install_from_registry(config, &from, &pkg);
    }

    // Standard nixpkgs install
    let pkg = args.package.ok_or_else(|| {
        Error::Usage(
            "Usage: nixy install <package> or nixy install --file <path> or nixy install --from <registry> <package>".to_string(),
        )
    })?;

    // Validate package exists in nixpkgs
    info(&format!("Validating package {}...", pkg));
    if !Nix::validate_package(&pkg)? {
        return Err(Error::PackageNotFound(pkg));
    }

    // Check if existing flake.nix is nixy-managed
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if flake_path.exists() && !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    // Add package to flake.nix
    add_package_to_flake(config, &pkg)?;

    info(&format!("Installing {}...", pkg));
    super::sync::run(config)?;

    Ok(())
}

/// Add a package to flake.nix
fn add_package_to_flake(config: &Config, pkg: &str) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // Check if existing flake.nix is nixy-managed
    if flake_path.exists() && !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    // If flake doesn't exist, create it
    if !flake_path.exists() {
        fs::create_dir_all(&flake_dir)?;
        let content = generate_flake(&[pkg.to_string()], Some(&flake_dir), None);
        fs::write(&flake_path, content)?;
        success(&format!("Added {} to {}", pkg, flake_path.display()));
        return Ok(());
    }

    // Check if package already exists
    let content = fs::read_to_string(&flake_path)?;
    let pattern = format!(
        r"^\s*{} = pkgs\.{};",
        regex::escape(pkg),
        regex::escape(pkg)
    );
    if Regex::new(&pattern)?.is_match(&content) {
        success(&format!(
            "Package {} already in {}",
            pkg,
            flake_path.display()
        ));
        return Ok(());
    }

    // Partial edit: insert package into existing flake.nix
    let pkg_entry = format!("          {} = pkgs.{};", pkg, pkg);
    let content = insert_after_marker(&content, "nixy:packages", &pkg_entry);

    let path_entry = format!("              {}", pkg);
    let content = insert_after_marker(&content, "nixy:env-paths", &path_entry);

    fs::write(&flake_path, content)?;
    success(&format!("Added {} to {}", pkg, flake_path.display()));

    Ok(())
}

/// Install from a flake registry or direct URL
fn install_from_registry(config: &Config, from_arg: &str, pkg: &str) -> Result<()> {
    let flake_url = if from_arg.contains(':') {
        // Direct flake URL
        info(&format!("Using flake URL: {}", from_arg));
        from_arg.to_string()
    } else {
        // Registry lookup
        info(&format!("Looking up '{}' in nix registry...", from_arg));
        let url = Nix::registry_lookup(from_arg)?
            .ok_or_else(|| Error::RegistryNotFound(from_arg.to_string()))?;
        info(&format!("Found: {}", url));
        url
    };

    // Derive input name
    let input_name = if !from_arg.contains(':') {
        // Use registry alias
        sanitize_input_name(from_arg)
    } else {
        // Derive from URL (owner-repo)
        derive_input_name_from_url(&flake_url)
    };

    // Validate the package exists
    info(&format!(
        "Validating package '{}' in {}...",
        pkg, input_name
    ));
    let pkg_output = Nix::validate_flake_package(&flake_url, pkg)?.ok_or_else(|| {
        let available = Nix::list_flake_packages(&flake_url, None)
            .unwrap_or_default()
            .into_iter()
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");
        if available.is_empty() {
            Error::FlakePackageNotFound(pkg.to_string(), input_name.clone())
        } else {
            Error::Usage(format!(
                "Package '{}' not found in '{}'. Available packages: {}...",
                pkg, input_name, available
            ))
        }
    })?;

    // Get or create flake
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        fs::create_dir_all(&flake_dir)?;
        let content = generate_flake(&[], Some(&flake_dir), None);
        fs::write(&flake_path, content)?;
    }

    if !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    // Add the flake as an input and the package
    add_registry_package_to_flake(config, &input_name, &flake_url, pkg, &pkg_output)?;

    info(&format!("Installing {} from {}...", pkg, input_name));
    super::sync::run(config)?;

    Ok(())
}

/// Add a package from a registry flake to flake.nix
fn add_registry_package_to_flake(
    config: &Config,
    input_name: &str,
    flake_url: &str,
    pkg: &str,
    pkg_output: &str,
) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let mut content = fs::read_to_string(&flake_path)?;

    // Check if we should reuse existing nixpkgs input
    let (final_input_name, use_existing_nixpkgs) =
        if flake_url.contains("NixOS/nixpkgs") && content.contains("nixpkgs.url") {
            info("Using existing nixpkgs input");
            ("nixpkgs".to_string(), true)
        } else {
            (input_name.to_string(), false)
        };

    // Add input if needed
    if !use_existing_nixpkgs {
        let input_pattern = format!(r"^\s*{}\.url", regex::escape(&final_input_name));
        if !Regex::new(&input_pattern)?.is_match(&content) {
            let input_entry = format!("    {}.url = \"{}\";", final_input_name, flake_url);
            content = insert_after_marker(&content, "nixy:custom-inputs", &input_entry);
            success(&format!("Added input '{}' to flake.nix", final_input_name));
        } else {
            info(&format!(
                "Input '{}' already exists in flake.nix",
                final_input_name
            ));
        }
    }

    // Check if package already exists
    let pkg_pattern = format!(r"^\s*{} = ", regex::escape(pkg));
    if Regex::new(&pkg_pattern)?.is_match(&content) {
        success(&format!("Package '{}' already in flake.nix", pkg));
        fs::write(&flake_path, content)?;
        return Ok(());
    }

    // Add package
    let pkg_entry = format!(
        "          {} = inputs.{}.{}.${{system}}.{};",
        pkg, final_input_name, pkg_output, pkg
    );
    content = insert_after_marker(&content, "nixy:custom-packages", &pkg_entry);

    // Add to env-paths
    let path_entry = format!("              {}", pkg);
    content = insert_after_marker(&content, "nixy:custom-paths", &path_entry);

    fs::write(&flake_path, content)?;
    success(&format!(
        "Added {} from {} to flake.nix",
        pkg, final_input_name
    ));

    Ok(())
}

/// Install from a local nix file
fn install_from_file(config: &Config, file: &Path, force: bool) -> Result<()> {
    if !file.exists() {
        return Err(Error::FileNotFound(file.display().to_string()));
    }

    // Check if this is a flake file
    if is_flake_file(file) {
        return install_from_flake_file(config, file, force);
    }

    // Extract package name
    let content = fs::read_to_string(file)?;
    let pkg_name = parse_local_package_attr(&content, "pname")
        .or_else(|| parse_local_package_attr(&content, "name"))
        .ok_or_else(|| Error::NoPackageName(file.display().to_string()))?;

    info(&format!(
        "Installing local package: {} from {}",
        pkg_name,
        file.display()
    ));

    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // Check if existing flake.nix is nixy-managed
    if flake_path.exists() && !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    // Check for custom modifications
    if flake_path.exists() {
        let packages = Nix::eval_packages(&flake_dir)?;
        if has_custom_modifications(&flake_path, &packages, &flake_dir) {
            if !force {
                warn("flake.nix has modifications outside nixy markers.");
                warn("Use --force to proceed (custom changes will be lost).");
                return Err(Error::CustomModifications);
            }
            warn("Proceeding with --force: custom modifications outside markers will be lost.");
        }
    }

    // Create packages directory
    let pkg_dir = flake_dir.join("packages");
    fs::create_dir_all(&pkg_dir)?;

    // Copy file to packages directory
    let dest = pkg_dir.join(format!("{}.nix", pkg_name));
    fs::copy(file, &dest)?;
    success(&format!("Copied package definition to {}", dest.display()));

    // Add to git if in a git repo
    git_add(&flake_dir, &format!("packages/{}.nix", pkg_name));

    // Regenerate flake.nix
    let packages = Nix::eval_packages(&flake_dir)?;
    let preserved = PreservedContent::from_file(&flake_path);
    let new_content = generate_flake(&packages, Some(&flake_dir), Some(&preserved));
    fs::write(&flake_path, new_content)?;

    info(&format!("Installing {}...", pkg_name));
    super::sync::run(config)?;

    Ok(())
}

/// Install from a local flake file
fn install_from_flake_file(config: &Config, file: &Path, force: bool) -> Result<()> {
    // Extract package name from filename
    let pkg_name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_input_name)
        .ok_or_else(|| Error::InvalidFilename(file.display().to_string()))?;

    if pkg_name.is_empty() {
        return Err(Error::InvalidFilename(file.display().to_string()));
    }

    info(&format!(
        "Installing local flake: {} from {}",
        pkg_name,
        file.display()
    ));

    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // Check if existing flake.nix is nixy-managed
    if flake_path.exists() && !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    // Check for custom modifications
    if flake_path.exists() {
        let packages = Nix::eval_packages(&flake_dir)?;
        if has_custom_modifications(&flake_path, &packages, &flake_dir) {
            if !force {
                warn("flake.nix has modifications outside nixy markers.");
                warn("Use --force to proceed (custom changes will be lost).");
                return Err(Error::CustomModifications);
            }
            warn("Proceeding with --force: custom modifications outside markers will be lost.");
        }
    }

    // Create package directory
    let pkg_dir = flake_dir.join("packages").join(&pkg_name);
    fs::create_dir_all(&pkg_dir)?;

    // Copy file as flake.nix
    let dest = pkg_dir.join("flake.nix");
    fs::copy(file, &dest)?;
    success(&format!("Copied flake to {}", dest.display()));

    // Add to git if in a git repo
    git_add(&flake_dir, &format!("packages/{}/flake.nix", pkg_name));

    // Regenerate flake.nix
    let packages = Nix::eval_packages(&flake_dir)?;
    let preserved = PreservedContent::from_file(&flake_path);
    let new_content = generate_flake(&packages, Some(&flake_dir), Some(&preserved));
    fs::write(&flake_path, new_content)?;

    info(&format!("Installing {}...", pkg_name));
    super::sync::run(config)?;

    Ok(())
}

/// Sanitize a string for use as an input name
fn sanitize_input_name(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    sanitized.trim_matches('-').to_string()
}

/// Derive an input name from a flake URL
fn derive_input_name_from_url(url: &str) -> String {
    // Try to extract owner-repo from URL
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() >= 2 {
        let owner = parts[parts.len() - 2];
        let repo = parts[parts.len() - 1].trim_end_matches(".git");
        sanitize_input_name(&format!("{}-{}", owner, repo))
    } else {
        "custom-flake".to_string()
    }
}

/// Add a file to git if in a git repo
fn git_add(dir: &Path, file: &str) {
    // Check if in a git repo
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "add", file])
            .output();
    }
}
