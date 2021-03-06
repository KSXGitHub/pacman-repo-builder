pub fn extract_value_from_line<'a>(prefix: &str, line: &'a str) -> Option<&'a str> {
    let line = line.trim();
    if !line.starts_with(prefix) {
        return None;
    }
    let line = &line[prefix.len()..].trim_start();
    if !line.starts_with('=') {
        return None;
    }
    Some(line[1..].trim())
}

#[test]
fn test_extract_value_from_line_some() {
    assert_eq!(
        extract_value_from_line("pkgname", "  pkgname = foo  "),
        Some("foo"),
    );
}

#[test]
fn test_extract_value_from_line_none() {
    assert_eq!(
        extract_value_from_line("pkgname", "  pkgbase = foo  "),
        None,
    );
}
