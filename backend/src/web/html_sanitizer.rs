/// Rejects HTML in `longUrl` the way an empty jsoup safelist does.
pub struct HtmlSanitizer;

impl HtmlSanitizer {
    pub fn looks_unsafe(value: &str) -> bool {
        if value.trim().is_empty() {
            return false;
        }
        let cleaned = ammonia::Builder::empty().clean(value).to_string();
        let changed = cleaned != value;
        let elements = fragment_has_elements(value);
        changed || elements
    }
}

fn fragment_has_elements(value: &str) -> bool {
    ammonia::is_html(value)
}

#[cfg(test)]
mod tests {
    use super::HtmlSanitizer;

    #[test]
    fn plain_url_is_safe_and_markup_is_unsafe() {
        assert!(!HtmlSanitizer::looks_unsafe("https://example.com/system-design"));
        assert!(!HtmlSanitizer::looks_unsafe("https://example.com/path"));
        assert!(!HtmlSanitizer::looks_unsafe(""));
        assert!(!HtmlSanitizer::looks_unsafe("   "));
        assert!(HtmlSanitizer::looks_unsafe(
            "https://example.com/<script>alert(1)</script>"
        ));
        assert!(HtmlSanitizer::looks_unsafe("<script>alert(1)</script>"));
        assert!(HtmlSanitizer::looks_unsafe("<img src=x onerror=alert(1)>"));
        assert!(HtmlSanitizer::looks_unsafe("<b>bold</b>"));
    }
}
