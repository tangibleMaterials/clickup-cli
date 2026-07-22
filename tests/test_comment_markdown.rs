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

// With --markdown, the body must use the `comment` doc-block array, not
// `comment_text`.
#[tokio::test]
async fn test_comment_create_markdown_posts_doc_blocks() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/list/list-1/comment"))
        .and(body_json(serde_json::json!({
            "comment": [
                { "type": "h1", "text": "Plan" },
                { "type": "p", "text": "Intro." },
                {
                    "type": "bullet_list",
                    "children": [
                        { "type": "list_item", "text": "Step 1" },
                        { "type": "list_item", "text": "Step 2" }
                    ]
                }
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

// A task comment with --markdown keeps notify_all alongside the doc blocks.
#[tokio::test]
async fn test_comment_create_markdown_task_keeps_notify_all() {
    let dir = tempfile::TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path_matcher("/v2/task/abc123/comment"))
        .and(body_json(serde_json::json!({
            "comment": [
                { "type": "code", "text": "puts 1", "attrs": { "language": "ruby" } }
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
