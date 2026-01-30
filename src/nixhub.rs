//! Nixhub API client for package resolution.
//!
//! This module provides a client for the Nixhub/Devbox Search API to resolve
//! package versions to specific nixpkgs commits and attribute paths.
//!
//! API documentation: https://www.jetify.com/docs/nixhub

use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::nix::Nix;

/// Deserialize a Vec that might be null in the JSON
fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let opt: Option<Vec<T>> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

const SEARCH_API_ENDPOINT: &str = "https://search.devbox.sh";

/// Nixhub API client
pub struct NixhubClient {
    host: String,
}

impl Default for NixhubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NixhubClient {
    pub fn new() -> Self {
        Self {
            host: SEARCH_API_ENDPOINT.to_string(),
        }
    }

    /// Search for packages by query
    pub fn search(&self, query: &str) -> Result<SearchResponse> {
        if query.is_empty() {
            return Err(Error::Usage("Search query cannot be empty".to_string()));
        }

        let url = format!("{}/v2/search?q={}", self.host, urlencoding::encode(query));
        let response: SearchResponse = ureq::get(&url)
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404, _) => Error::NixhubPackageNotFound(query.to_string()),
                ureq::Error::Transport(_) => Error::NixhubUnreachable,
                _ => Error::NixhubApi(e.to_string()),
            })?
            .into_json()
            .map_err(|e| Error::NixhubApi(format!("Failed to parse response: {}", e)))?;

        Ok(response)
    }

    /// Get package details including all versions
    pub fn get_package(&self, name: &str) -> Result<PackageDetails> {
        let url = format!("{}/v2/pkg?name={}", self.host, urlencoding::encode(name));
        let response: PackageDetails = ureq::get(&url)
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404, _) => Error::NixhubPackageNotFound(name.to_string()),
                ureq::Error::Transport(_) => Error::NixhubUnreachable,
                _ => Error::NixhubApi(e.to_string()),
            })?
            .into_json()
            .map_err(|e| Error::NixhubApi(format!("Failed to parse response: {}", e)))?;

        Ok(response)
    }

    /// Resolve a package name and version to a nixpkgs commit and attribute path
    pub fn resolve(&self, name: &str, version: &str) -> Result<ResolveResponse> {
        let url = format!(
            "{}/v2/resolve?name={}&version={}",
            self.host,
            urlencoding::encode(name),
            urlencoding::encode(version)
        );
        let response: ResolveResponse = ureq::get(&url)
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404, _) => {
                    Error::NixhubVersionNotFound(name.to_string(), version.to_string())
                }
                ureq::Error::Transport(_) => Error::NixhubUnreachable,
                _ => Error::NixhubApi(e.to_string()),
            })?
            .into_json()
            .map_err(|e| Error::NixhubApi(format!("Failed to parse response: {}", e)))?;

        Ok(response)
    }

    /// Resolve a package to the current system's details
    pub fn resolve_for_current_system(
        &self,
        name: &str,
        version: &str,
    ) -> Result<ResolvedPackageInfo> {
        let response = self.resolve(name, version)?;
        let system = Nix::current_system()?;

        let system_info = response.systems.get(&system).ok_or_else(|| {
            Error::NixhubResolve(
                name.to_string(),
                version.to_string(),
                format!("Package not available for system '{}'", system),
            )
        })?;

        Ok(ResolvedPackageInfo {
            name: response.name,
            version: response.version,
            attribute_path: system_info.flake_installable.attr_path.clone(),
            commit_hash: system_info.flake_installable.r#ref.rev.clone(),
        })
    }
}

/// Parsed package specification (name and optional version)
#[derive(Debug, Clone, PartialEq)]
pub struct PackageSpec {
    pub name: String,
    pub version: Option<String>,
}

/// Parse a package specification like "nodejs@20.1.0" or "nodejs"
pub fn parse_package_spec(spec: &str) -> PackageSpec {
    if let Some((name, version)) = spec.split_once('@') {
        PackageSpec {
            name: name.to_string(),
            version: Some(version.to_string()),
        }
    } else {
        PackageSpec {
            name: spec.to_string(),
            version: None,
        }
    }
}

/// Resolved package information for a specific system
#[derive(Debug, Clone)]
pub struct ResolvedPackageInfo {
    pub name: String,
    pub version: String,
    pub attribute_path: String,
    pub commit_hash: String,
}

// API Response types
// Note: Many fields are kept for potential future use and API completeness

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub total_results: i32,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub results: Vec<SearchResult>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub summary: String,
    pub last_updated: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PackageDetails {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub homepage_url: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub releases: Vec<Release>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub platforms_summary: Option<String>,
    #[serde(default)]
    pub outputs_summary: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Platform {
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub attribute_path: Option<String>,
    #[serde(default)]
    pub commit_hash: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub outputs: Vec<Output>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Output {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub default: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ResolveResponse {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub systems: HashMap<String, SystemInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SystemInfo {
    pub flake_installable: FlakeInstallable,
    pub last_updated: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub outputs: Vec<ResolveOutput>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FlakeInstallable {
    pub r#ref: FlakeRef,
    pub attr_path: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FlakeRef {
    pub r#type: String,
    pub owner: String,
    pub repo: String,
    pub rev: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ResolveOutput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub nar: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_spec_with_version() {
        let spec = parse_package_spec("nodejs@20.1.0");
        assert_eq!(spec.name, "nodejs");
        assert_eq!(spec.version, Some("20.1.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_semver_range() {
        let spec = parse_package_spec("python@3.11");
        assert_eq!(spec.name, "python");
        assert_eq!(spec.version, Some("3.11".to_string()));
    }

    #[test]
    fn test_parse_package_spec_without_version() {
        let spec = parse_package_spec("ripgrep");
        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.version, None);
    }

    #[test]
    fn test_parse_package_spec_empty_version() {
        // Edge case: "pkg@" should result in empty version string
        let spec = parse_package_spec("pkg@");
        assert_eq!(spec.name, "pkg");
        assert_eq!(spec.version, Some("".to_string()));
    }

    #[test]
    fn test_parse_package_spec_multiple_at_signs() {
        // Should only split on first @
        let spec = parse_package_spec("pkg@1.0@extra");
        assert_eq!(spec.name, "pkg");
        assert_eq!(spec.version, Some("1.0@extra".to_string()));
    }
}
