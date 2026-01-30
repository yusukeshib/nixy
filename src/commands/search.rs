use crate::error::Result;
use crate::nixhub::NixhubClient;

use super::info;

pub fn run(query: &str) -> Result<()> {
    info(&format!("Searching for {}...", query));

    let client = NixhubClient::new();
    let results = client.search(query)?;

    if results.results.is_empty() {
        println!("No packages found for '{}'", query);
        return Ok(());
    }

    println!();
    println!("Found {} packages:", results.total_results);
    println!();

    for pkg in &results.results {
        println!("  {} - {}", pkg.name, pkg.summary);
    }

    // Show version info for the first (most relevant) result
    if let Some(first) = results.results.first() {
        println!();
        info(&format!("Fetching versions for {}...", first.name));

        match client.get_package(&first.name) {
            Ok(details) => {
                if !details.releases.is_empty() {
                    println!();
                    println!("Available versions for {}:", first.name);
                    for release in &details.releases {
                        println!("  {}", release.version);
                    }
                    println!();
                    println!("Install with: nixy add {}@<version>", first.name);
                }
            }
            Err(e) => {
                eprintln!("  Failed to fetch versions: {}", e);
            }
        }
    }

    Ok(())
}
