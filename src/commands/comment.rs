use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum CommentCommands {
    /// List comments on a task, list, or view
    List {
        /// Task ID
        #[arg(long, conflicts_with_all = ["list", "view"])]
        task: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = ["task", "view"])]
        list: Option<String>,
        /// View ID
        #[arg(long, conflicts_with_all = ["task", "list"])]
        view: Option<String>,
    },
    /// Create a comment on a task, list, or view
    Create {
        /// Task ID
        #[arg(long, conflicts_with_all = ["list", "view"])]
        task: Option<String>,
        /// List ID
        #[arg(long, conflicts_with_all = ["task", "view"])]
        list: Option<String>,
        /// View ID
        #[arg(long, conflicts_with_all = ["task", "list"])]
        view: Option<String>,
        /// Comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Plain text by default (ClickUp's v2 comment API stores markdown as literal text); pass --markdown to render it as rich ClickUp doc blocks.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Parse --text as markdown and post it as ClickUp doc blocks (headings, lists, code, quotes) so it renders as rich content instead of literal text.
        #[arg(long)]
        markdown: bool,
        /// Assignee user ID (task comments only)
        #[arg(long)]
        assignee: Option<i64>,
        /// Notify all watchers (task comments only)
        #[arg(long)]
        notify_all: bool,
    },
    /// Update a comment
    Update {
        /// Comment ID
        id: String,
        /// New comment text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Mark as resolved
        #[arg(long)]
        resolved: bool,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
    },
    /// Delete a comment
    Delete {
        /// Comment ID
        id: String,
    },
    /// List threaded replies on a comment
    Replies {
        /// Comment ID
        id: String,
    },
    /// Reply to a comment
    Reply {
        /// Comment ID
        id: String,
        /// Reply text (use @path to read from a file, @- for stdin, @@ for a literal leading @). Note: ClickUp's v2 comment API does not render markdown; markdown syntax is stored as literal text.
        #[arg(long, value_parser = crate::input::resolve_value_arg)]
        text: String,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<i64>,
    },
}

const COMMENT_FIELDS: &[&str] = &["id", "user", "date", "comment_text"];

pub async fn execute(command: CommentCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        CommentCommands::List { task, list, view } => {
            let base = if let Some(id) = list {
                format!("/v2/list/{}/comment", id)
            } else if let Some(id) = view {
                format!("/v2/view/{}/comment", id)
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                format!("/v2/task/{}/comment", resolved.id)
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                "comments",
                |start, start_id| match (start, start_id) {
                    (Some(s), Some(sid)) => format!("{}?start={}&start_id={}", base, s, sid),
                    _ => base.clone(),
                },
            )
            .await?;
            let truncated: Vec<serde_json::Value> = comments
                .into_iter()
                .map(|mut c| {
                    if let Some(text) = c.get("comment_text").and_then(|v| v.as_str()) {
                        // Truncate by chars (not bytes) so the 60-byte boundary
                        // can't land inside a multibyte UTF-8 codepoint.
                        let truncated = if text.chars().count() > 60 {
                            let head: String = text.chars().take(60).collect();
                            format!("{}…", head)
                        } else {
                            text.to_string()
                        };
                        c["comment_text"] = serde_json::Value::String(truncated);
                    }
                    c
                })
                .collect();
            output.print_items(&truncated, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Create {
            task,
            list,
            view,
            text,
            markdown,
            assignee,
            notify_all,
        } => {
            // With --markdown, parse the text into ClickUp doc blocks and send
            // them via the `comment` array (rendered rich content). Otherwise
            // send the raw string via `comment_text` (stored verbatim).
            let comment_body = || -> serde_json::Value {
                if markdown {
                    serde_json::json!({ "comment": crate::markdown::to_doc_blocks(&text) })
                } else {
                    serde_json::json!({ "comment_text": text })
                }
            };

            let (endpoint, body) = if let Some(id) = list {
                (format!("/v2/list/{}/comment", id), comment_body())
            } else if let Some(id) = view {
                (format!("/v2/view/{}/comment", id), comment_body())
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let mut body = comment_body();
                body["notify_all"] = serde_json::json!(notify_all);
                if let Some(a) = assignee {
                    body["assignee"] = serde_json::json!(a);
                }
                (format!("/v2/task/{}/comment", resolved.id), body)
            } else {
                return Err(CliError::ClientError {
                    message: "One of --task, --list, or --view is required".to_string(),
                    status: 0,
                });
            };
            let resp = client.post(&endpoint, &body).await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Update {
            id,
            text,
            resolved,
            assignee,
        } => {
            let mut body = serde_json::json!({ "comment_text": text });
            if resolved {
                body["resolved"] = serde_json::Value::Bool(true);
            }
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
            let resp = client.put(&format!("/v2/comment/{}", id), &body).await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Delete { id } => {
            client.delete(&format!("/v2/comment/{}", id)).await?;
            output.print_message(&format!("Comment {} deleted", id));
            Ok(())
        }
        CommentCommands::Replies { id } => {
            let comments = crate::commands::pagination::walk_start_id(
                cli,
                &client,
                "comments",
                |start, start_id| match (start, start_id) {
                    (Some(s), Some(sid)) => {
                        format!("/v2/comment/{}/reply?start={}&start_id={}", id, s, sid)
                    }
                    _ => format!("/v2/comment/{}/reply", id),
                },
            )
            .await?;
            output.print_items(&comments, COMMENT_FIELDS, "id");
            Ok(())
        }
        CommentCommands::Reply { id, text, assignee } => {
            let mut body = serde_json::json!({ "comment_text": text });
            if let Some(a) = assignee {
                body["assignee"] = serde_json::json!(a);
            }
            let resp = client
                .post(&format!("/v2/comment/{}/reply", id), &body)
                .await?;
            output.print_single(&resp, COMMENT_FIELDS, "id");
            Ok(())
        }
    }
}
