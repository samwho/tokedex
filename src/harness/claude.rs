use super::{Harness, Message, content_text, jsonl_files, parse_lines, str_at, timestamp_at};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) struct ClaudeCode;

impl Harness for ClaudeCode {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn label(&self) -> &'static str {
        "Claude Code"
    }

    fn default_root(&self, home: &Path) -> PathBuf {
        home.join(".claude/projects")
    }

    fn conversation_files(&self, root: &Path) -> Vec<PathBuf> {
        jsonl_files(&[root.to_path_buf()])
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<Message>> {
        parse_lines(path, |value, messages, current_model, seen| {
            if value["type"] == "system" || value["type"] == "attachment" {
                let body = content_text(if value["type"] == "system" {
                    &value["content"]
                } else {
                    &value["attachment"]
                });
                if !body.is_empty() {
                    messages.push(Message {
                        harness: "claude",
                        model: None,
                        body,
                        used_at: timestamp_at(value),
                    });
                }
                return;
            }
            if value["type"] != "assistant" && value["type"] != "user" {
                return;
            }
            let payload = &value["message"];
            if value["type"] == "assistant"
                && let Some(model) = str_at(&payload["model"])
            {
                *current_model = Some(model);
            }
            let body = content_text(payload);
            if body.is_empty() {
                return;
            }
            let used_at = timestamp_at(value);
            if value["type"] == "assistant"
                && let Some(id) = payload["id"].as_str()
            {
                if let Some(&index) = seen.get(id) {
                    if body.len() > messages[index].body.len() {
                        messages[index].body = body;
                    }
                    messages[index].used_at = messages[index].used_at.max(used_at);
                    return;
                }
                seen.insert(id.to_string(), messages.len());
            }
            messages.push(Message {
                harness: "claude",
                model: (value["type"] == "assistant")
                    .then(|| current_model.clone())
                    .flatten(),
                body,
                used_at,
            });
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::fixture;

    #[test]
    fn deduplicates_streaming_snapshots() {
        let path = fixture(&[
            r#"{"type":"user","message":{"content":"hello"}}"#,
            r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"text","text":"hi"}]}}"#,
            r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"text","text":"hi there"}]}}"#,
        ]);
        let messages = ClaudeCode
            .parse_file(&path)
            .expect("Claude fixture should parse");
        std::fs::remove_file(path).expect("fixture should be removed");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].body, "hi there");
    }
}
