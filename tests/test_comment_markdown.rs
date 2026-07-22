use assert_cmd::Command;
use std::path::Path;
use wiremock::matchers::{body_json, method, path as path_matcher};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn clickup(dir: &Path, server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("clickup-cli").unwrap();
    cmd.current_dir(dir)
        .env("CLICKUP_API_URL", server.uri())
        .env("CLICKUP_TOKEN", "pk_test")
        .env("CLICKUP_WORKSPACE", "99")
        .env_remove("CLICKUP_GIT_DETECT")
        .env_remove("CLICKUP_TASK_ID");
    cmd
}

// With --markdown, the body must use the `comment` op array, not
// `comment_text`. The body is a flat Quill-style op stream: text ops carry
// inline marks, and each line is terminated by a newline op whose attributes
// carry the block formatting (header, list, …).
#[tokio::test]
async fn test_comment_create_markdown_posts_doc_blocks() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/comment"))
        .and(body_json(serde_json::json!({
            "comment": [
                { "text": "Plan" },
                { "text": "\n", "attributes": { "header": 1 } },
                { "text": "Intro." },
                { "text": "\n" },
                { "text": "Step 1" },
                { "text": "\n", "attributes": { "list": "bullet" } },
                { "text": "Step 2" },
                { "text": "\n", "attributes": { "list": "bullet" } }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c1"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--list",
            "list-1",
            "--text",
            "# Plan\n\nIntro.\n\n- Step 1\n- Step 2",
            "--markdown",
        ])
        .assert()
        .success();
}

// A task comment with --markdown keeps notify_all alongside the comment ops.
#[tokio::test]
async fn test_comment_create_markdown_task_keeps_notify_all() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/abc123/comment"))
        .and(body_json(serde_json::json!({
            "comment": [
                { "text": "puts 1" },
                { "text": "\n", "attributes": { "code-block": true } }
            ],
            "notify_all": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c2"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--task",
            "abc123",
            "--text",
            "```ruby\nputs 1\n```",
            "--markdown",
        ])
        .assert()
        .success();
}

// Without --markdown, existing behaviour is unchanged: plain comment_text.
#[tokio::test]
async fn test_comment_create_without_markdown_sends_plain_text() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/comment"))
        .and(body_json(serde_json::json!({
            "comment_text": "# Not rendered"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "c3"
        })))
        .expect(1)
        .mount(&server)
        .await;

    clickup(dir.path(), &server)
        .args([
            "comment",
            "create",
            "--list",
            "list-1",
            "--text",
            "# Not rendered",
        ])
        .assert()
        .success();
}
