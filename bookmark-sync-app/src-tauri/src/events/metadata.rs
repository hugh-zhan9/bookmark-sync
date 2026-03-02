use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::time::Duration;

pub struct SiteMetadata {
    pub title: Option<String>,
    pub favicon_url: Option<String>,
}

/// Fetches the target URL and extracts <title> and OpenGraph / icon data.
pub fn fetch_metadata(url: &str) -> Result<SiteMetadata, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 BookmarkSyncBot")
        .build()
        .map_err(|e| e.to_string())?;

    let res = client.get(url).send().map_err(|e| e.to_string())?;
    
    // We only care if it's successful HTML
    if !res.status().is_success() {
        return Err(format!("Bad status: {}", res.status()));
    }

    let html_content = res.text().map_err(|e| e.to_string())?;
    let document = Html::parse_document(&html_content);

    let mut title = None;
    let mut favicon_url = None;

    // 1. Try to get title
    if let Ok(title_selector) = Selector::parse("title") {
        if let Some(el) = document.select(&title_selector).next() {
            title = Some(el.inner_html().trim().to_string());
        }
    }

    // 2. Try OG Title if <title> is missing
    if title.is_none() {
        if let Ok(og_title) = Selector::parse("meta[property='og:title']") {
            if let Some(el) = document.select(&og_title).next() {
                if let Some(content) = el.value().attr("content") {
                    title = Some(content.trim().to_string());
                }
            }
        }
    }

    // 3. Try to get favicon (Rel shortcut icon, icon, apple-touch-icon)
    if let Ok(icon_selector) = Selector::parse("link[rel~='icon'], link[rel='shortcut icon']") {
        for el in document.select(&icon_selector) {
            if let Some(href) = el.value().attr("href") {
                favicon_url = Some(resolve_url(url, href));
                break;
            }
        }
    }

    // If still no favicon, guess the standard path
    if favicon_url.is_none() {
        if let Ok(parsed_url) = url::Url::parse(url) {
            favicon_url = Some(format!("{}://{}/favicon.ico", parsed_url.scheme(), parsed_url.host_str().unwrap_or("")));
        }
    }

    Ok(SiteMetadata { title, favicon_url })
}

/// Extremely basic absolute URL resolver for relative paths
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http") {
        return relative.to_string();
    }
    
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(resolved) = base_url.join(relative) {
            return resolved.to_string();
        }
    }
    
    relative.to_string()
}
