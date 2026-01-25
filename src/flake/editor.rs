use regex::Regex;

/// Insert a line after a marker in content
pub fn insert_after_marker(content: &str, marker: &str, line: &str) -> String {
    let mut result = String::new();
    let marker_pattern = format!("# [{}]", marker);

    for content_line in content.lines() {
        result.push_str(content_line);
        result.push('\n');

        if content_line.contains(&marker_pattern) {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Remove lines matching a pattern from between two markers
pub fn remove_from_section(content: &str, start_marker: &str, end_marker: &str, pattern: &Regex) -> String {
    let mut result = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.contains(start_marker) {
            in_section = true;
        }

        if line.contains(end_marker) {
            in_section = false;
        }

        // Skip lines matching pattern within section
        if in_section && pattern.is_match(line) {
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Extract content between markers (excluding the markers themselves)
pub fn extract_marker_content(content: &str, marker: &str) -> String {
    let start_marker = format!("# [{}]", marker);
    let end_marker = format!("# [/{}]", marker);

    let mut result = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.contains(&end_marker) {
            in_section = false;
            continue;
        }

        if in_section {
            result.push_str(line);
            result.push('\n');
        }

        if line.contains(&start_marker) {
            in_section = true;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_after_marker() {
        let content = "# [nixy:packages]\n# [/nixy:packages]\n";
        let result = insert_after_marker(content, "nixy:packages", "          hello = pkgs.hello;");
        assert!(result.contains("          hello = pkgs.hello;"));
    }

    #[test]
    fn test_insert_multiple_packages() {
        let content = "# [nixy:packages]\n# [/nixy:packages]\n";
        let result = insert_after_marker(content, "nixy:packages", "          ripgrep = pkgs.ripgrep;");
        let result = insert_after_marker(&result, "nixy:packages", "          fzf = pkgs.fzf;");

        assert!(result.contains("ripgrep = pkgs.ripgrep;"));
        assert!(result.contains("fzf = pkgs.fzf;"));
    }

    #[test]
    fn test_insert_preserves_existing_content() {
        let content = "before\n# [nixy:packages]\n          existing = pkgs.existing;\n# [/nixy:packages]\nafter\n";
        let result = insert_after_marker(content, "nixy:packages", "          new = pkgs.new;");

        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(result.contains("existing = pkgs.existing;"));
        assert!(result.contains("new = pkgs.new;"));
    }

    #[test]
    fn test_remove_from_section() {
        let content = "# [nixy:packages]\n          hello = pkgs.hello;\n# [/nixy:packages]\n";
        let pattern = Regex::new(r"^\s*hello = pkgs\.hello;").unwrap();
        let result = remove_from_section(content, "# [nixy:packages]", "# [/nixy:packages]", &pattern);
        assert!(!result.contains("hello = pkgs.hello"));
    }

    #[test]
    fn test_remove_preserves_other_packages() {
        let content = "# [nixy:packages]\n          ripgrep = pkgs.ripgrep;\n          fzf = pkgs.fzf;\n          bat = pkgs.bat;\n# [/nixy:packages]\n";
        let pattern = Regex::new(r"^\s*fzf = pkgs\.fzf;").unwrap();
        let result = remove_from_section(content, "# [nixy:packages]", "# [/nixy:packages]", &pattern);

        assert!(!result.contains("fzf = pkgs.fzf;"));
        assert!(result.contains("ripgrep = pkgs.ripgrep;"));
        assert!(result.contains("bat = pkgs.bat;"));
    }

    #[test]
    fn test_remove_preserves_content_outside_section() {
        let content = "before section\n# [nixy:packages]\n          hello = pkgs.hello;\n# [/nixy:packages]\nafter section\n";
        let pattern = Regex::new(r"^\s*hello = pkgs\.hello;").unwrap();
        let result = remove_from_section(content, "# [nixy:packages]", "# [/nixy:packages]", &pattern);

        assert!(result.contains("before section"));
        assert!(result.contains("after section"));
    }

    #[test]
    fn test_remove_only_in_correct_section() {
        let content = "# [nixy:packages]\n          hello = pkgs.hello;\n# [/nixy:packages]\n# [nixy:custom-packages]\n          hello = custom.hello;\n# [/nixy:custom-packages]\n";
        let pattern = Regex::new(r"^\s*hello = pkgs\.hello;").unwrap();
        let result = remove_from_section(content, "# [nixy:packages]", "# [/nixy:packages]", &pattern);

        // Should remove from nixy:packages
        assert!(!result.contains("hello = pkgs.hello;"));
        // Should NOT remove from custom-packages (different pattern)
        assert!(result.contains("hello = custom.hello;"));
    }

    #[test]
    fn test_extract_marker_content() {
        let content = "# [nixy:custom-inputs]\n    foo.url = \"github:foo/bar\";\n# [/nixy:custom-inputs]\n";
        let result = extract_marker_content(content, "nixy:custom-inputs");
        assert_eq!(result.trim(), "foo.url = \"github:foo/bar\";");
    }

    #[test]
    fn test_extract_empty_marker_content() {
        let content = "# [nixy:custom-inputs]\n# [/nixy:custom-inputs]\n";
        let result = extract_marker_content(content, "nixy:custom-inputs");
        assert!(result.trim().is_empty());
    }

    #[test]
    fn test_extract_multiline_content() {
        let content = "# [nixy:custom-inputs]\n    foo.url = \"github:foo/bar\";\n    bar.url = \"github:bar/baz\";\n# [/nixy:custom-inputs]\n";
        let result = extract_marker_content(content, "nixy:custom-inputs");
        assert!(result.contains("foo.url"));
        assert!(result.contains("bar.url"));
    }

    #[test]
    fn test_extract_nonexistent_marker() {
        let content = "# [nixy:packages]\nhello = pkgs.hello;\n# [/nixy:packages]\n";
        let result = extract_marker_content(content, "nixy:nonexistent");
        assert!(result.is_empty());
    }
}
