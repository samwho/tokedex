mod claude;
mod codex;
mod pi;

use anyhow::{Context, Result};
use chrono::DateTime;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use claude::ClaudeCode;
use codex::Codex;
use pi::Pi;

#[derive(Debug)]
pub struct Message {
    pub harness: &'static str,
    pub model: Option<String>,
    pub body: String,
    pub used_at: Option<i64>,
}

pub trait Harness: Send + Sync {
    fn name(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn default_root(&self, home: &Path) -> PathBuf;
    fn conversation_files(&self, root: &Path) -> Vec<PathBuf>;
    fn parse_file(&self, path: &Path) -> Result<Vec<Message>>;
}

pub fn harnesses() -> Vec<Box<dyn Harness>> {
    vec![Box::new(Codex), Box::new(ClaudeCode), Box::new(Pi)]
}

pub fn parse_lines(
    path: &Path,
    mut handle: impl FnMut(&Value, &mut Vec<Message>, &mut Option<String>, &mut HashMap<String, usize>),
) -> Result<Vec<Message>> {
    let file = File::open(path).with_context(|| "opening session")?;
    let mut messages = Vec::new();
    let mut current_model = None;
    let mut seen = HashMap::new();
    for (line_no, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("reading line {}", line_no + 1))?;
        let value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("warning: {}:{}: {err}", path.display(), line_no + 1);
                continue;
            }
        };
        handle(&value, &mut messages, &mut current_model, &mut seen);
    }
    // A user message belongs to the following assistant turn, including after model switches.
    let mut next_model = None;
    for message in messages.iter_mut().rev() {
        if message.model.is_none() {
            message.model.clone_from(&next_model);
        } else {
            next_model.clone_from(&message.model);
        }
    }
    Ok(messages)
}

pub fn jsonl_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots.iter().filter(|root| root.exists()) {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|x| x == "jsonl")
            {
                files.push(entry.into_path());
            }
        }
    }
    files.sort();
    files
}

pub fn timestamp_at(value: &Value) -> Option<i64> {
    let timestamp = &value["timestamp"];
    timestamp
        .as_i64()
        .or_else(|| {
            timestamp
                .as_u64()
                .and_then(|value| i64::try_from(value).ok())
        })
        .or_else(|| {
            timestamp
                .as_str()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.timestamp_millis())
        })
        .or_else(|| value["message"]["timestamp"].as_i64())
}

pub fn str_at(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn is_message_role(value: &Value) -> bool {
    matches!(
        value.as_str(),
        Some("system" | "user" | "assistant" | "toolResult" | "custom")
    )
}

pub fn content_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(values) => values
            .iter()
            .map(content_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(object) => {
            let mut parts = Vec::new();
            for key in [
                "name",
                "toolName",
                "text",
                "thinking",
                "content",
                "arguments",
                "output",
            ] {
                if let Some(value) = object.get(key) {
                    let text = content_text(value);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
            for key in ["input", "sections", "toolsAdded", "toolsRemoved"] {
                if let Some(value) = object.get(key)
                    && !value.is_null()
                {
                    parts.push(
                        serde_json::to_string(value)
                            .expect("serializing saved structured content should be infallible"),
                    );
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
pub fn fixture(lines: &[&str]) -> PathBuf {
    use std::io::Write;

    let path = std::env::temp_dir().join(format!(
        "tokedex-harness-test-{}-{:?}.jsonl",
        std::process::id(),
        std::thread::current().id()
    ));
    let mut file = File::create(&path).expect("fixture should be created");
    for line in lines {
        writeln!(file, "{line}").expect("fixture line should be written");
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_reasoning_and_tool_io() {
        let content = serde_json::json!([
            {"type":"thinking","thinking":"plan"},
            {"type":"tool_use","name":"read","input":{"path":"README.md"}},
            {"type":"tool_result","content":[{"type":"text","text":"contents"}]}
        ]);
        let text = content_text(&content);
        assert!(text.contains("plan"));
        assert!(text.contains("read"));
        assert!(text.contains("README.md"));
        assert!(text.contains("contents"));
    }
}
