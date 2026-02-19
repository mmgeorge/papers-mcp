use papers_zotero::{CollectionListParams, ItemListParams, ZoteroClient, ZoteroError};

/// Returns `true` if `input` looks like a Zotero key.
///
/// Zotero keys are exactly 8 uppercase alphanumeric characters (e.g. `LF4MJWZK`).
pub fn looks_like_zotero_key(input: &str) -> bool {
    input.len() == 8 && input.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Resolve an item key or title/search string to a Zotero item key.
///
/// If `input` looks like a Zotero key (8 uppercase alphanumeric chars), it is
/// returned directly. Otherwise the library's top-level items are searched by
/// title/creator/year and the first match's key is returned.
pub async fn resolve_item_key(client: &ZoteroClient, input: &str) -> Result<String, ZoteroError> {
    if looks_like_zotero_key(input) {
        return Ok(input.to_string());
    }
    let params = ItemListParams::builder().q(input).limit(1).build();
    let resp = client.list_top_items(&params).await?;
    resp.items
        .into_iter()
        .next()
        .map(|item| item.key)
        .ok_or_else(|| ZoteroError::Api {
            status: 404,
            message: format!("No item found matching: {input}"),
        })
}

/// Resolve a collection key or name to a Zotero collection key.
///
/// If `input` looks like a Zotero key, it is returned directly. Otherwise all
/// collections are listed (up to 100) and the first whose name contains `input`
/// (case-insensitive) is returned.
pub async fn resolve_collection_key(
    client: &ZoteroClient,
    input: &str,
) -> Result<String, ZoteroError> {
    if looks_like_zotero_key(input) {
        return Ok(input.to_string());
    }
    let input_lower = input.to_lowercase();
    let params = CollectionListParams::builder().limit(100).build();
    let resp = client.list_collections(&params).await?;
    resp.items
        .into_iter()
        .find(|c| c.data.name.to_lowercase().contains(&input_lower))
        .map(|c| c.key)
        .ok_or_else(|| ZoteroError::Api {
            status: 404,
            message: format!("No collection found matching: {input}"),
        })
}

/// Resolve a saved-search key or name to a Zotero search key.
///
/// If `input` looks like a Zotero key, it is returned directly. Otherwise all
/// saved searches are listed and the first whose name contains `input`
/// (case-insensitive) is returned.
pub async fn resolve_search_key(
    client: &ZoteroClient,
    input: &str,
) -> Result<String, ZoteroError> {
    if looks_like_zotero_key(input) {
        return Ok(input.to_string());
    }
    let input_lower = input.to_lowercase();
    let resp = client.list_searches().await?;
    resp.items
        .into_iter()
        .find(|s| s.data.name.to_lowercase().contains(&input_lower))
        .map(|s| s.key)
        .ok_or_else(|| ZoteroError::Api {
            status: 404,
            message: format!("No saved search found matching: {input}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_zotero_key_valid() {
        assert!(looks_like_zotero_key("LF4MJWZK"));
        assert!(looks_like_zotero_key("ABC12345"));
        assert!(looks_like_zotero_key("QXNY8AX8"));
        assert!(looks_like_zotero_key("12345678"));
        assert!(looks_like_zotero_key("ABCDEFGH"));
        assert!(looks_like_zotero_key("U9PRIZJ7"));
    }

    #[test]
    fn test_looks_like_zotero_key_invalid() {
        assert!(!looks_like_zotero_key(""));
        assert!(!looks_like_zotero_key("ABC1234"));   // 7 chars
        assert!(!looks_like_zotero_key("ABC123456")); // 9 chars
        assert!(!looks_like_zotero_key("abc12345"));  // lowercase
        assert!(!looks_like_zotero_key("Abc12345"));  // mixed case
        assert!(!looks_like_zotero_key("LF4MJW K"));  // space
        assert!(!looks_like_zotero_key("Attention is all you need")); // title
        assert!(!looks_like_zotero_key("Test Paper")); // space
        assert!(!looks_like_zotero_key("GPU Papers")); // collection name
    }
}
