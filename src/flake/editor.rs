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
    fn test_remove_from_section() {
        let content = "# [nixy:packages]\n          hello = pkgs.hello;\n# [/nixy:packages]\n";
        let pattern = Regex::new(r"^\s*hello = pkgs\.hello;").unwrap();
        let result = remove_from_section(content, "# [nixy:packages]", "# [/nixy:packages]", &pattern);
        assert!(!result.contains("hello = pkgs.hello"));
    }

    #[test]
    fn test_extract_marker_content() {
        let content = "# [nixy:custom-inputs]\n    foo.url = \"github:foo/bar\";\n# [/nixy:custom-inputs]\n";
        let result = extract_marker_content(content, "nixy:custom-inputs");
        assert_eq!(result.trim(), "foo.url = \"github:foo/bar\";");
    }
}
