use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use plist::Value;
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportNode {
    pub original_id: String, // 浏览器原始 ID
    pub parent_original_id: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub is_folder: bool,
    pub browser: String,
}

#[derive(Debug, Deserialize)]
struct ChromeBookmarkNode {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
    pub children: Option<Vec<ChromeBookmarkNode>>,
    #[serde(rename = "type")]
    pub node_type: String,
}

#[derive(Debug, Deserialize)]
struct ChromeBookmarksRoot {
    pub roots: std::collections::HashMap<String, ChromeBookmarkNode>,
}

pub fn scan_all_nodes() -> Vec<ImportNode> {
    let mut all_nodes = Vec::new();
    let _ = scan_chromium_nodes("Google/Chrome", "Chrome").map(|mut n| all_nodes.append(&mut n));
    let _ = scan_chromium_nodes("Microsoft Edge", "Edge").map(|mut n| all_nodes.append(&mut n));
    let _ = scan_safari_nodes().map(|mut n| all_nodes.append(&mut n));
    all_nodes
}

fn scan_chromium_nodes(folder_name: &str, browser_label: &str) -> Result<Vec<ImportNode>, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not found")?;
    let path = PathBuf::from(home).join("Library/Application Support").join(folder_name).join("Default/Bookmarks");
    if !path.exists() { return Err("Not found".into()); }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let root: ChromeBookmarksRoot = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for (_key, node) in root.roots {
        traverse_chrome_recursive(&node, None, &mut result, browser_label);
    }
    Ok(result)
}

fn traverse_chrome_recursive(node: &ChromeBookmarkNode, parent_id: Option<String>, result: &mut Vec<ImportNode>, browser: &str) {
    result.push(ImportNode {
        original_id: node.id.clone(),
        parent_original_id: parent_id,
        title: node.name.clone(),
        url: node.url.clone(),
        is_folder: node.node_type == "folder",
        browser: browser.into(),
    });

    if let Some(children) = &node.children {
        for child in children {
            traverse_chrome_recursive(child, Some(node.id.clone()), result, browser);
        }
    }
}

pub fn scan_safari_nodes() -> Result<Vec<ImportNode>, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not found")?;
    let path = PathBuf::from(home).join("Library/Safari/Bookmarks.plist");
    if !path.exists() { return Err("Not found".into()); }

    let root = Value::from_file(path).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    if let Some(dict) = root.as_dictionary() {
        traverse_safari_recursive(dict, None, &mut result);
    }
    Ok(result)
}

fn traverse_safari_recursive(dict: &plist::Dictionary, parent_id: Option<String>, result: &mut Vec<ImportNode>) {
    let web_bookmark_type = dict.get("WebBookmarkType").and_then(|v| v.as_string());
    let original_id = dict.get("WebBookmarkUUID").and_then(|v| v.as_string()).unwrap_or("safari-root").to_string();
    
    if web_bookmark_type == Some("WebBookmarkTypeLeaf") {
        let url = dict.get("URLString").and_then(|v| v.as_string());
        let title = dict.get("URIDictionary").and_then(|v| v.as_dictionary()).and_then(|d| d.get("title")).and_then(|v| v.as_string());
        if let Some(u) = url {
            result.push(ImportNode {
                original_id, parent_original_id: parent_id,
                title: title.unwrap_or(u).into(), url: Some(u.into()),
                is_folder: false, browser: "Safari".into(),
            });
        }
    } else if web_bookmark_type == Some("WebBookmarkTypeList") {
        let title = dict.get("Title").and_then(|v| v.as_string()).unwrap_or("Untitled");
        result.push(ImportNode {
            original_id: original_id.clone(), parent_original_id: parent_id,
            title: title.into(), url: None, is_folder: true, browser: "Safari".into(),
        });
        if let Some(children) = dict.get("Children").and_then(|v| v.as_array()) {
            for child in children {
                if let Some(child_dict) = child.as_dictionary() {
                    traverse_safari_recursive(child_dict, Some(original_id.clone()), result);
                }
            }
        }
    }
}

pub fn parse_browser_stable_id(stable_id: &str) -> Option<(String, String)> {
    let mut parts = stable_id.splitn(2, '-');
    let browser = parts.next()?.to_lowercase();
    let original_id = parts.next()?.to_string();
    if original_id.is_empty() {
        return None;
    }
    match browser.as_str() {
        "chrome" | "edge" | "safari" => Some((browser, original_id)),
        _ => None,
    }
}

pub fn delete_bookmark_in_browser(stable_id: &str) -> Result<bool, String> {
    let Some((browser, original_id)) = parse_browser_stable_id(stable_id) else {
        return Ok(false);
    };
    match browser.as_str() {
        "chrome" => delete_chromium_bookmark("Google/Chrome", &original_id),
        "edge" => delete_chromium_bookmark("Microsoft Edge", &original_id),
        "safari" => delete_safari_bookmark(&original_id),
        _ => Ok(false),
    }
}

fn delete_chromium_bookmark(folder_name: &str, original_id: &str) -> Result<bool, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not found".to_string())?;
    let path = PathBuf::from(home)
        .join("Library/Application Support")
        .join(folder_name)
        .join("Default/Bookmarks");
    if !path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut value: JsonValue = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let removed = remove_chromium_bookmark_from_value(&mut value, original_id);
    if removed {
        let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())?;
    }
    Ok(removed)
}

pub fn remove_chromium_bookmark_from_value(node: &mut JsonValue, original_id: &str) -> bool {
    let mut removed = false;
    if let Some(obj) = node.as_object_mut() {
        if let Some(children) = obj.get_mut("children").and_then(|v| v.as_array_mut()) {
            let mut idx = 0usize;
            while idx < children.len() {
                let is_target = children[idx].get("id").and_then(|v| v.as_str()) == Some(original_id);
                if is_target {
                    children.remove(idx);
                    removed = true;
                    continue;
                }
                if remove_chromium_bookmark_from_value(&mut children[idx], original_id) {
                    removed = true;
                }
                idx += 1;
            }
        }
        for value in obj.values_mut() {
            if remove_chromium_bookmark_from_value(value, original_id) {
                removed = true;
            }
        }
    } else if let Some(arr) = node.as_array_mut() {
        for value in arr {
            if remove_chromium_bookmark_from_value(value, original_id) {
                removed = true;
            }
        }
    }
    removed
}

fn delete_safari_bookmark(original_id: &str) -> Result<bool, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not found".to_string())?;
    let path = PathBuf::from(home).join("Library/Safari/Bookmarks.plist");
    if !path.exists() {
        return Ok(false);
    }
    let mut root = Value::from_file(&path).map_err(|e| e.to_string())?;
    let removed = remove_safari_bookmark_from_value(&mut root, original_id);
    if removed {
        root.to_file_xml(&path).map_err(|e| e.to_string())?;
    }
    Ok(removed)
}

fn remove_safari_bookmark_from_value(value: &mut Value, original_id: &str) -> bool {
    let mut removed = false;
    if let Some(dict) = value.as_dictionary_mut() {
        if let Some(children) = dict.get_mut("Children").and_then(|v| v.as_array_mut()) {
            let mut idx = 0usize;
            while idx < children.len() {
                let is_target = children[idx]
                    .as_dictionary()
                    .and_then(|d| d.get("WebBookmarkUUID"))
                    .and_then(|v| v.as_string())
                    == Some(original_id);
                if is_target {
                    children.remove(idx);
                    removed = true;
                    continue;
                }
                if remove_safari_bookmark_from_value(&mut children[idx], original_id) {
                    removed = true;
                }
                idx += 1;
            }
        }
    } else if let Some(arr) = value.as_array_mut() {
        for child in arr {
            if remove_safari_bookmark_from_value(child, original_id) {
                removed = true;
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::{parse_browser_stable_id, remove_chromium_bookmark_from_value};

    #[test]
    fn parse_browser_stable_id_should_parse_prefix_and_id() {
        let parsed = parse_browser_stable_id("chrome-123").expect("should parse");
        assert_eq!(parsed.0.as_str(), "chrome");
        assert_eq!(parsed.1.as_str(), "123");
        assert!(parse_browser_stable_id("local-id").is_none());
    }

    #[test]
    fn remove_chromium_bookmark_from_value_should_remove_matching_node() {
        let mut value = serde_json::json!({
            "roots": {
                "bookmark_bar": {
                    "id": "1",
                    "type": "folder",
                    "name": "bar",
                    "children": [
                        {"id": "2", "type": "url", "name": "target", "url": "https://example.com"},
                        {"id": "3", "type": "url", "name": "other", "url": "https://rust-lang.org"}
                    ]
                }
            }
        });
        let removed = remove_chromium_bookmark_from_value(&mut value, "2");
        assert!(removed);
        let children = value["roots"]["bookmark_bar"]["children"].as_array().expect("children array");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["id"].as_str(), Some("3"));
    }
}
