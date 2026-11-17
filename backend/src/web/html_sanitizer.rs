/// Rejects HTML in `longUrl` the way an empty jsoup safelist does.
pub struct HtmlSanitizer;

impl HtmlSanitizer {
    pub fn looks_unsafe(value: &str) -> bool {
        if value.trim().is_empty() {
            return false;
        }
        // Ammonia escapes `&` in ordinary query strings, so a changed
        // serialization is not enough. Reject markup the parser sees as HTML.
        fragment_has_elements(value)
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
        assert!(!HtmlSanitizer::looks_unsafe(
            "https://example.com/search?q=rust&lang=en"
        ));
        assert!(!HtmlSanitizer::looks_unsafe(
            "https://www.google.com/search?q=weather+today&sca_esv=d692c529a49cacda&sxsrf=AKL5yDgYdS29-XsLWACkqg-AdaNBYoCJiw%3A1794927563865&source=hp&ei=y2v8asSAMoutmtkPge7oyQ8&iflsig=AHSINFcAAAAAavx522ARvCQUIIQe3RX5ck_E1MTJpLFP&oq=weath&gs_lp=Egdnd3Mtd2l6IgV3ZWF0aCoQCAIYgAQYigUYQxixAxiDATITEAAYgAQYigUYQxixAxiDARjJAzINEAAYgAQYigUYQxixAzIQEAAYgAQYigUYQxixAxiDATIIEAAYgAQYkgMyDhAAGIAEGLEDGIMBGJIDMhAQABiABBiKBRhDGLEDGIMBMhMQLhiABBiKBRhDGLEDGMcBGNEDMgsQABiABBixAxiDATILEAAYgAQYsQMYgwEyChAAGIAEGIoFGENIrnBQ7ydYmUVwAXgAkAEAmAE4oAGKAqoBATW4AQHIAQD4AQGYAgagAswCqAIKwgIHECMY6gIYJ8ICBBAjGCfCAhAQLhiABBiKBRhDGLEDGIMBwgIREC4YgAQYsQMYgwEYxwEY0QPCAhYQLhiABBiKBRhDGLEDGIMBGMcBGNEDwgINEAAYgAQYigUYQxi0B5gDEfEFXxhT_gOYCT2SBwE2oAfQLLIHATW4B7sCwgcFMi0zLjPIBzCACAE&sclient=gws-wiz"
        ));
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
