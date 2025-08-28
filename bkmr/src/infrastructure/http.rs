use crate::domain;
use crate::domain::error::DomainResult;
use std::time::{Duration, Instant};
use tracing::debug;

/// Check if a website is accessible
#[allow(dead_code)]
pub fn check_website(url: &str, timeout_milliseconds: u64) -> (bool, u128) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_milliseconds))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new()); // Fallback to default client in case of builder failure

    let start = Instant::now();
    let response = client.head(url).send();

    match response {
        Ok(resp) if resp.status().is_success() => {
            let duration = start.elapsed().as_millis();
            (true, duration)
        }
        _ => (false, 0), // Return false and 0 duration in case of error or non-success status
    }
}

// TODO: timeout
pub fn load_url_details(url: &str) -> DomainResult<(String, String, String)> {
    let client = reqwest::blocking::Client::new();
    let body = client
        .get(url)
        .send()
        .map_err(|e| domain::error::DomainError::CannotFetchMetadata(e.to_string()))?
        .text()
        .map_err(|e| domain::error::DomainError::CannotFetchMetadata(e.to_string()))?;

    let document = select::document::Document::from(body.as_str());

    let title = document
        .find(select::predicate::Name("title"))
        .next()
        .map(|n| n.text().trim().to_owned())
        .unwrap_or_default();

    let description = document
        .find(select::predicate::Attr("name", "description"))
        .next()
        .and_then(|n| n.attr("content"))
        .unwrap_or_default();

    let keywords = document
        .find(select::predicate::Attr("name", "keywords"))
        .next()
        .and_then(|node| node.attr("content"))
        .unwrap_or_default();

    debug!("Keywords {:?}", keywords);

    Ok((title, description.to_owned(), keywords.to_owned()))
}

// TODO: tests sporadically failing (example.com issue?)
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore]
    fn given_valid_url_when_load_details_then_returns_title() -> DomainResult<()> {
        // let _ = init_test_env();
        // let _guard = EnvGuard::new();

        let url = "http://example.com";
        // let url = "https://www.rust-lang.org/";
        let (title, description, keywords) = load_url_details(url)?;

        // Print values for debugging purposes
        println!("Title: {}", title);
        println!("Description: {}", description);
        println!("Keywords: {}", keywords);

        // Example.com returns "Example Domain" as title and typically no meta description or keywords.
        assert_eq!(title, "Example Domain");
        assert_eq!(description, "");
        assert_eq!(keywords, "");
        Ok(())
    }

    #[test]
    fn given_website_url_when_check_then_returns_accessibility_status() {
        // This test depends on network availability.
        let (accessible, duration) = check_website("https://google.com", 2000);
        assert!(accessible, "Expected example.com to be accessible");
        assert!(duration > 0, "Duration should be greater than 0");
    }
}
