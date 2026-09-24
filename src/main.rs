use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(about = "Measure text token vocabulary coverage in Codex, Claude Code, and pi histories")]
struct Args {
    /// Override the default ~/.codex directory
    #[arg(long)]
    codex_dir: Option<PathBuf>,
    /// Override the default ~/.claude/projects directory
    #[arg(long)]
    claude_dir: Option<PathBuf>,
    /// Override the default ~/.pi/agent/sessions directory
    #[arg(long)]
    pi_dir: Option<PathBuf>,
    /// Include GPT-6 using o200k_base as an unverified assumption
    #[arg(long)]
    assume_gpt6_o200k: bool,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Codex,
    Claude,
    Pi,
}

#[derive(Debug)]
struct Message {
    model: Option<String>,
    text: String,
}

#[derive(Default)]
struct Counts {
    messages: usize,
    tokens: usize,
    by_id: HashMap<u32, usize>,
}

#[derive(Serialize)]
struct TokenReport {
    id: u32,
    text: String,
    count: usize,
}

#[derive(Serialize)]
struct ModelReport {
    model: String,
    status: String,
    encoding: Option<String>,
    messages: usize,
    tokens: Option<usize>,
    unique_tokens: Option<usize>,
    vocabulary_size: Option<usize>,
    vocabulary_percent: Option<f64>,
    highest_id_token: Option<TokenReport>,
    most_common_token: Option<TokenReport>,
}

#[derive(Serialize)]
struct Report {
    files_scanned: usize,
    models: Vec<ModelReport>,
    unattributed_messages: usize,
    notes: Vec<&'static str>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    let home = PathBuf::from(home);
    let roots = [
        (
            Source::Codex,
            args.codex_dir.unwrap_or_else(|| home.join(".codex")),
        ),
        (
            Source::Claude,
            args.claude_dir
                .unwrap_or_else(|| home.join(".claude/projects")),
        ),
        (
            Source::Pi,
            args.pi_dir
                .unwrap_or_else(|| home.join(".pi/agent/sessions")),
        ),
    ];
    let mut grouped: BTreeMap<String, Counts> = BTreeMap::new();
    let mut unattributed = 0;
    let mut files_scanned = 0;
    let mut encodings: HashMap<&'static str, CoreBPE> = HashMap::new();
    for (source, root) in roots {
        for path in source_files(source, &root) {
            let messages = match read_messages(source, &path) {
                Ok(messages) => messages,
                Err(err) => {
                    eprintln!("warning: {}: {err:#}", path.display());
                    continue;
                }
            };
            files_scanned += 1;
            for message in messages {
                let Some(model) = message.model else {
                    unattributed += 1;
                    continue;
                };
                let counts = grouped.entry(model.clone()).or_default();
                counts.messages += 1;
                if let Some(encoding) = encoding_for(&model, args.assume_gpt6_o200k) {
                    if !encodings.contains_key(encoding) {
                        encodings.insert(
                            encoding,
                            match encoding {
                                "cl100k_base" => cl100k_base()?,
                                _ => o200k_base()?,
                            },
                        );
                    }
                    let bpe = &encodings[encoding];
                    for id in bpe.encode_ordinary(&message.text) {
                        counts.tokens += 1;
                        *counts.by_id.entry(id).or_default() += 1;
                    }
                }
            }
        }
    }
    let models = grouped
        .into_iter()
        .map(|(model, counts)| {
            let encoding = encoding_for(&model, args.assume_gpt6_o200k);
            let status = if model.starts_with("claude-") {
                "unsupported: Anthropic has not published the current tokenizer or token IDs"
            } else if model.starts_with("gpt-6") && encoding.is_some() {
                "assumed: GPT-6 encoding has not been verified"
            } else if encoding.is_none() {
                "unsupported: no verified tokenizer mapping"
            } else {
                "exact for visible text"
            };
            let vocabulary_size = encoding.map(|name| {
                if name == "cl100k_base" {
                    100_256
                } else {
                    199_998
                }
            });
            let bpe = encoding.and_then(|name| encodings.get(name));
            let highest_id_token = counts
                .by_id
                .iter()
                .max_by_key(|(id, _)| *id)
                .and_then(|(&id, &count)| bpe.map(|bpe| token_report(bpe, id, count)));
            let most_common_token = counts
                .by_id
                .iter()
                .max_by(|(a_id, a_count), (b_id, b_count)| {
                    a_count.cmp(b_count).then_with(|| b_id.cmp(a_id))
                })
                .and_then(|(&id, &count)| bpe.map(|bpe| token_report(bpe, id, count)));
            ModelReport {
                model,
                status: status.to_string(),
                encoding: encoding.map(str::to_string),
                messages: counts.messages,
                tokens: encoding.map(|_| counts.tokens),
                unique_tokens: encoding.map(|_| counts.by_id.len()),
                vocabulary_size,
                vocabulary_percent: vocabulary_size
                    .map(|size| counts.by_id.len() as f64 * 100.0 / size as f64),
                highest_id_token,
                most_common_token,
            }
        })
        .collect();
    let report = Report {
        files_scanned,
        models,
        unattributed_messages: unattributed,
        notes: vec![
            "Counts cover saved user and assistant text only. Images, tools, hidden reasoning, and protocol tokens are excluded.",
            "A token ID is decoded alone; its bytes may not be valid UTF-8, so invalid bytes use the U+FFFD replacement character.",
        ],
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn token_report(bpe: &CoreBPE, id: u32, count: usize) -> TokenReport {
    let bytes = bpe.decode_bytes(&[id]).unwrap_or_default();
    TokenReport {
        id,
        text: String::from_utf8_lossy(&bytes).into_owned(),
        count,
    }
}

fn source_files(source: Source, root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }
    let roots: Vec<PathBuf> = if source == Source::Codex {
        vec![root.join("sessions"), root.join("archived_sessions")]
    } else {
        vec![root.to_path_buf()]
    };
    let mut files = Vec::new();
    for root in roots {
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

fn read_messages(source: Source, path: &Path) -> Result<Vec<Message>> {
    let file = File::open(path).with_context(|| "opening session")?;
    let mut messages = Vec::new();
    let mut current_model: Option<String> = None;
    let mut claude_seen: HashMap<String, usize> = HashMap::new();
    for (line_no, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("reading line {}", line_no + 1))?;
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("warning: {}:{}: {err}", path.display(), line_no + 1);
                continue;
            }
        };
        match source {
            Source::Codex => {
                if value["type"] == "turn_context" {
                    current_model = str_at(&value["payload"]["model"]);
                } else if value["type"] == "response_item" && value["payload"]["type"] == "message"
                {
                    let payload = &value["payload"];
                    if is_chat_role(&payload["role"]) {
                        let text = block_text(&payload["content"]);
                        if !text.is_empty() {
                            messages.push(Message {
                                model: (payload["role"] == "assistant")
                                    .then(|| current_model.clone())
                                    .flatten(),
                                text,
                            });
                        }
                    }
                }
            }
            Source::Claude => {
                if value["type"] == "assistant" || value["type"] == "user" {
                    let payload = &value["message"];
                    if value["type"] == "assistant"
                        && let Some(model) = str_at(&payload["model"])
                    {
                        current_model = Some(model);
                    }
                    let text = if let Some(text) = payload["content"].as_str() {
                        text.to_string()
                    } else {
                        block_text(&payload["content"])
                    };
                    if !text.is_empty() {
                        if value["type"] == "assistant"
                            && let Some(id) = payload["id"].as_str()
                        {
                            if let Some(&index) = claude_seen.get(id) {
                                if text.len() > messages[index].text.len() {
                                    messages[index].text = text;
                                }
                                continue;
                            }
                            claude_seen.insert(id.to_string(), messages.len());
                        }
                        messages.push(Message {
                            model: (value["type"] == "assistant")
                                .then(|| current_model.clone())
                                .flatten(),
                            text,
                        });
                    }
                }
            }
            Source::Pi => {
                if value["type"] == "model_change" {
                    current_model = str_at(&value["modelId"]).or_else(|| str_at(&value["model"]));
                } else if value["type"] == "message" {
                    let payload = &value["message"];
                    if !is_chat_role(&payload["role"]) {
                        continue;
                    }
                    let model = str_at(&payload["model"]).or_else(|| current_model.clone());
                    if payload["role"] == "assistant" {
                        current_model = model.clone();
                    }
                    let text = if let Some(text) = payload["content"].as_str() {
                        text.to_string()
                    } else {
                        block_text(&payload["content"])
                    };
                    if !text.is_empty() {
                        messages.push(Message {
                            model: (payload["role"] == "assistant").then_some(model).flatten(),
                            text,
                        });
                    }
                }
            }
        }
    }
    // A user message belongs to the following assistant turn, including after model switches.
    let mut next_model = None;
    for message in messages.iter_mut().rev() {
        if message.model.is_none() {
            message.model = next_model.clone();
        } else {
            next_model = message.model.clone();
        }
    }
    Ok(messages)
}

fn str_at(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}
fn is_chat_role(value: &Value) -> bool {
    value == "user" || value == "assistant"
}
fn block_text(value: &Value) -> String {
    let Some(blocks) = value.as_array() else {
        return String::new();
    };
    blocks
        .iter()
        .filter(|block| {
            matches!(
                block["type"].as_str(),
                Some("text" | "input_text" | "output_text")
            )
        })
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn encoding_for(model: &str, assume_gpt6: bool) -> Option<&'static str> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("claude-") {
        return None;
    }
    if m.starts_with("gpt-6") {
        return assume_gpt6.then_some("o200k_base");
    }
    if m.starts_with("gpt-5")
        || m.starts_with("gpt-4o")
        || m.starts_with("gpt-4.1")
        || m.starts_with("gpt-4.5")
        || m.starts_with("chatgpt-4o")
        || m.starts_with("codex-mini")
        || m == "o1"
        || m.starts_with("o1-")
        || m == "o3"
        || m.starts_with("o3-")
        || m == "o4-mini"
        || m.starts_with("o4-mini-")
    {
        return Some("o200k_base");
    }
    if m == "gpt-4"
        || m.starts_with("gpt-4-")
        || m.starts_with("gpt-4-turbo")
        || m.starts_with("gpt-3.5")
        || m.starts_with("gpt-35-")
    {
        return Some("cl100k_base");
    }
    None
}

fn print_report(report: &Report) {
    println!("Scanned {} session files", report.files_scanned);
    for m in &report.models {
        println!("\n{} — {}", m.model, m.status);
        println!("  messages: {}", m.messages);
        if let (Some(encoding), Some(unique), Some(size), Some(percent), Some(tokens)) = (
            &m.encoding,
            m.unique_tokens,
            m.vocabulary_size,
            m.vocabulary_percent,
            m.tokens,
        ) {
            println!("  encoding: {encoding}; text tokens: {tokens}");
            println!("  unique: {unique}/{size} ({percent:.2}% of ordinary vocabulary)");
            if let Some(token) = &m.highest_id_token {
                println!(
                    "  highest ID: {} {}",
                    token.id,
                    serde_json::to_string(&token.text).unwrap()
                );
            }
            if let Some(token) = &m.most_common_token {
                println!(
                    "  most common: {} {} ({} uses)",
                    token.id,
                    serde_json::to_string(&token.text).unwrap(),
                    token.count
                );
            }
        }
    }
    if report.unattributed_messages > 0 {
        println!("\nUnattributed messages: {}", report.unattributed_messages);
    }
    println!("\n{}", report.notes[0]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(lines: &[&str]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tokedex-test-{}-{:?}.jsonl",
            std::process::id(),
            std::thread::current().id()
        ));
        let mut file = File::create(&path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        path
    }

    #[test]
    fn codex_user_follows_next_model_after_switch() {
        let path = fixture(&[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"first"}]}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-4o"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"one"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"second"}]}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-5"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"two"}]}}"#,
        ]);
        let messages = read_messages(Source::Codex, &path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            messages
                .iter()
                .map(|m| m.model.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("gpt-4o"), Some("gpt-4o"), Some("gpt-5"), Some("gpt-5")]
        );
    }

    #[test]
    fn claude_duplicate_snapshots_count_once() {
        let path = fixture(&[
            r#"{"type":"user","message":{"content":"hello"}}"#,
            r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"text","text":"hi"}]}}"#,
            r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-sonnet-4-5","content":[{"type":"text","text":"hi there"}]}}"#,
        ]);
        let messages = read_messages(Source::Claude, &path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(messages[1].text, "hi there");
    }

    #[test]
    fn pi_user_uses_following_assistant_model() {
        let path = fixture(&[
            r#"{"type":"model_change","modelId":"gpt-4o"}"#,
            r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"question"}]}}"#,
            r#"{"type":"message","message":{"role":"assistant","model":"gpt-5","content":[{"type":"text","text":"answer"}]}}"#,
        ]);
        let messages = read_messages(Source::Pi, &path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.model.as_deref() == Some("gpt-5")));
    }
    #[test]
    fn mapping_is_conservative() {
        assert_eq!(encoding_for("gpt-5.6-sol", false), Some("o200k_base"));
        assert_eq!(encoding_for("gpt-4-turbo", false), Some("cl100k_base"));
        assert_eq!(encoding_for("claude-opus-4-6", false), None);
        assert_eq!(encoding_for("gpt-6-astra", false), None);
        assert_eq!(encoding_for("gpt-6-astra", true), Some("o200k_base"));
    }
    #[test]
    fn only_visible_text_blocks() {
        let blocks = serde_json::json!([{"type":"text","text":"hello"}, {"type":"thinking","thinking":"secret"}, {"type":"tool_use","input":{"x":1}}]);
        assert_eq!(block_text(&blocks), "hello");
    }
}
