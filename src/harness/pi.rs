use super::{
    Harness, Message, content_text, is_message_role, jsonl_files, parse_lines, str_at, timestamp_at,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) struct Pi;

impl Harness for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn label(&self) -> &'static str {
        "pi"
    }

    fn default_root(&self, home: &Path) -> PathBuf {
        home.join(".pi/agent/sessions")
    }

    fn conversation_files(&self, root: &Path) -> Vec<PathBuf> {
        jsonl_files(&[root.to_path_buf()])
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<Message>> {
        parse_lines(path, |value, messages, current_model, _| {
            if value["type"] == "model_change" {
                *current_model = str_at(&value["modelId"]).or_else(|| str_at(&value["model"]));
                return;
            }
            if value["type"] != "message" {
                return;
            }
            let payload = &value["message"];
            if !is_message_role(&payload["role"]) {
                return;
            }
            let model = str_at(&payload["model"]).or_else(|| current_model.clone());
            if payload["role"] == "assistant" {
                current_model.clone_from(&model);
            }
            let body = content_text(payload);
            if !body.is_empty() {
                messages.push(Message {
                    harness: "pi",
                    model: (payload["role"] == "assistant").then_some(model).flatten(),
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
    fn uses_the_assistant_model() {
        let path = fixture(&[
            r#"{"type":"model_change","modelId":"gpt-4o"}"#,
            r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"question"}]}}"#,
            r#"{"type":"message","message":{"role":"assistant","model":"gpt-5","content":[{"type":"text","text":"answer"}]}}"#,
        ]);
        let messages = Pi.parse_file(&path).expect("pi fixture should parse");
        std::fs::remove_file(path).expect("fixture should be removed");
        assert!(messages.iter().all(|m| m.model.as_deref() == Some("gpt-5")));
    }
}
