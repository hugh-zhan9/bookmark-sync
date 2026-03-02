use url::Url;

/// List of common tracking parameters to strip
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "msclkid",
    "ref",
    "_hsenc",
    "mc_cid",
    "mc_eid",
];

/// Normalizes a URL by parsing it and stripping known tracking query parameters.
/// Returns the cleaned URL string.
pub fn clean_url(raw_url: &str) -> String {
    let mut parsed = match Url::parse(raw_url) {
        Ok(u) => u,
        Err(_) => return raw_url.to_string(), // Return raw if unparseable
    };

    let mut new_query = Vec::new();

    // Iterate over existing query parameters
    for (key, value) in parsed.query_pairs() {
        if !TRACKING_PARAMS.contains(&key.as_ref()) {
            new_query.push((key.into_owned(), value.into_owned()));
        }
    }

    if new_query.is_empty() {
        parsed.set_query(None);
    } else {
        // Clear and rebuild query without trackers
        parsed.query_pairs_mut().clear().extend_pairs(new_query.iter());
    }

    // Attempt to drop any trailing hash/fragment if it's empty
    if let Some(fragment) = parsed.fragment() {
        if fragment.is_empty() {
            parsed.set_fragment(None);
        }
    }

    parsed.into()
}
