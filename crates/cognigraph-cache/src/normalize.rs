/// Normalize a query string for cache key generation.
///
/// Trims leading/trailing whitespace, lowercases, and collapses
/// internal whitespace runs to a single space.
pub fn normalize_query(s: &str) -> String {
    let trimmed = s.trim().to_lowercase();
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_was_space = false;

    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_basic() {
        assert_eq!(normalize_query("  Hello   World  "), "hello world");
    }

    #[test]
    fn test_normalize_tabs_newlines() {
        assert_eq!(normalize_query("foo\t\nbar"), "foo bar");
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_query("   "), "");
    }

    #[test]
    fn test_normalize_already_clean() {
        assert_eq!(normalize_query("hello world"), "hello world");
    }
}
