//! Integration tests for MCP tool classification and filtering.

use clickup_cli::mcp::classify::{classify, ALL_GROUPS};
use clickup_cli::mcp::tool_list;

#[test]
fn every_tool_classifies() {
    let tools = tool_list();
    let array = tools.as_array().expect("tool_list must return a JSON array");
    assert!(!array.is_empty(), "tool_list is empty");

    let mut unclassified: Vec<String> = Vec::new();
    let mut unknown_group: Vec<(String, String)> = Vec::new();

    for tool in array {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .expect("each tool must have a string `name`");

        match classify(name) {
            None => unclassified.push(name.to_string()),
            Some(meta) => {
                if !ALL_GROUPS.contains(&meta.group) {
                    unknown_group.push((name.to_string(), meta.group.to_string()));
                }
            }
        }
    }

    assert!(
        unclassified.is_empty(),
        "unclassified tools (add to OVERRIDES or extend verb sets): {:?}",
        unclassified
    );
    assert!(
        unknown_group.is_empty(),
        "tools mapped to unknown groups: {:?}",
        unknown_group
    );
}

#[test]
fn expected_tool_count() {
    // Sanity check: we don't want a future refactor to silently drop tools.
    let tools = tool_list();
    let array = tools.as_array().unwrap();
    assert_eq!(array.len(), 143, "tool count changed; update this test");
}
