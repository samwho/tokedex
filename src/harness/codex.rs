use super::{Harness, Message, content_text, jsonl_files, parse_lines, str_at, timestamp_at};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) struct Codex;

impl Harness for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn label(&self) -> &'static str {
        "Codex"
    }

    fn default_root(&self, home: &Path) -> PathBuf {
        home.join(".codex")
    }

    fn conversation_files(&self, root: &Path) -> Vec<PathBuf> {
        jsonl_files(&[root.join("sessions"), root.join("archived_sessions")])
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<Message>> {
        parse_lines(path, |value, messages, current_model, _| {
            if value["type"] == "turn_context" {
                *current_model = str_at(&value["payload"]["model"]);
            } else if value["type"] == "response_item" {
                let payload = &value["payload"];
                let body = content_text(payload);
                if body.is_empty() {
                    return;
                }
                let is_model_output = payload["role"] == "assistant"
                    || matches!(
                        payload["type"].as_str(),
                        Some("function_call" | "reasoning")
                    );
                messages.push(Message {
                    harness: "codex",
                    model: is_model_output.then(|| current_model.clone()).flatten(),
                    body,
                    used_at: timestamp_at(value),
                });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::fixture;

    #[test]
    fn attributes_users_after_switches() {
        let path = fixture(&[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first"}]}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-4o"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"one"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"second"}]}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-5"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"two"}]}}"#,
        ]);
        let messages = Codex.parse_file(&path).expect("Codex fixture should parse");
        std::fs::remove_file(path).expect("fixture should be removed");
        assert_eq!(
            messages
                .iter()
                .map(|message| message.model.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("gpt-4o"), Some("gpt-4o"), Some("gpt-5"), Some("gpt-5")]
        );
        assert!(messages.iter().all(|message| message.harness == "codex"));
    }

    #[test]
    fn includes_tool_calls_and_outputs() {
        let path = fixture(&[
            r#"{"type":"turn_context","payload":{"model":"gpt-5"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call","name":"read","arguments":"{\"path\":\"README.md\"}"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","output":"file contents"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#,
        ]);
        let messages = Codex.parse_file(&path).expect("Codex fixture should parse");
        std::fs::remove_file(path).expect("fixture should be removed");
        assert_eq!(messages.len(), 3);
        assert!(messages[0].body.contains("README.md"));
        assert!(messages[1].body.contains("file contents"));
        assert!(
            messages
                .iter()
                .all(|message| message.model.as_deref() == Some("gpt-5"))
        );
    }
}
