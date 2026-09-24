//! Format-oriented history generators. Every generated case is a complete JSONL file, not
//! an isolated message. Expected bodies are derived from the generated events, not from the
//! production content extractor. Formats: Codex rollout `ResponseItem`/`turn_context`,
//! Claude Code `user`/`assistant` streaming snapshots, pi v3 session entries.
use super::{Harness, Message};
use crate::harness::{claude::ClaudeCode, codex::Codex, pi::Pi};
use proptest::prelude::*;
use proptest::test_runner::TestCaseResult;
use serde_json::{Value, json};
use std::io::Write as _;
use tempfile::NamedTempFile;

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ]{1,24}",
        Just("\t\nΩ\\\"".to_string()),
        Just("こんにちは 🦀".to_string()),
    ]
}

fn model() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "gpt-4o".to_string(),
        "gpt-5".to_string(),
        "claude-sonnet-4-5".to_string(),
    ])
}

fn parse(harness: &dyn Harness, records: &[Value]) -> Vec<Message> {
    let mut file = NamedTempFile::new().expect("create generated history");
    for record in records {
        writeln!(file, "{record}").expect("write generated record");
    }
    harness
        .parse_file(file.path())
        .expect("parse generated history")
}

#[derive(Clone, Debug)]
enum CodexStep {
    Model(String),
    User(String),
    Assistant(String),
    Reasoning(String),
    Call(String),
    Shell(String),
    Result(String),
    StructuredResult(String),
    AgentMessage(String),
    Metadata(u8),
}

fn codex_step() -> impl Strategy<Value = CodexStep> {
    prop_oneof![
        2 => model().prop_map(CodexStep::Model),
        3 => text().prop_map(CodexStep::User),
        3 => text().prop_map(CodexStep::Assistant),
        2 => text().prop_map(CodexStep::Reasoning),
        2 => text().prop_map(CodexStep::Call),
        1 => text().prop_map(CodexStep::Shell),
        2 => text().prop_map(CodexStep::Result),
        1 => text().prop_map(CodexStep::StructuredResult),
        1 => text().prop_map(CodexStep::AgentMessage),
        1 => (0_u8..3).prop_map(CodexStep::Metadata),
    ]
}

fn codex_history(steps: &[CodexStep]) -> (Vec<Value>, Vec<(String, Option<String>)>) {
    let mut records = vec![json!({"type":"session_meta","payload":{"id":"session"}})];
    let mut expected = Vec::new();
    let mut current = None;
    for step in steps {
        match step {
            CodexStep::Model(model) => {
                current = Some(model.clone());
                records.push(json!({"type":"turn_context","payload":{"model":model}}));
            }
            CodexStep::User(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":text},{"type":"input_image","image_url":"data:image/png;base64,AA=="}]}}));
                expected.push((text.clone(), None));
            }
            CodexStep::Assistant(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}}));
                expected.push((text.clone(), current.clone()));
            }
            CodexStep::Reasoning(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":text}],"encrypted_content":"opaque"}}));
                expected.push((text.clone(), current.clone()));
            }
            CodexStep::Call(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"function_call","name":"read","arguments":text}}));
                expected.push((format!("read\n{text}"), current.clone()));
            }
            CodexStep::Shell(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"local_shell_call","action":{"type":"exec","command":text}}}));
                expected.push((json!({"command":text,"type":"exec"}).to_string(), current.clone()));
            }
            CodexStep::Result(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"function_call_output","output":text}}));
                expected.push((text.clone(), current.clone()));
            }
            CodexStep::StructuredResult(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"function_call_output","output":[{"type":"input_text","text":text},{"type":"input_image","image_url":"opaque"}]}}));
                expected.push((text.clone(), current.clone()));
            }
            CodexStep::AgentMessage(text) => {
                records.push(json!({"type":"response_item","payload":{"type":"agent_message","author":"agent","recipient":"parent","content":[{"type":"input_text","text":text}]}}));
                expected.push((text.clone(), current.clone()));
            }
            CodexStep::Metadata(kind) => records.push(match kind {
                0 => json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":999}}}}),
                1 => json!({"type":"compacted","payload":{"message":"snapshot"}}),
                _ => json!({"type":"world_state","payload":{"cwd":"/project"}}),
            }),
        }
    }
    backfill(&mut expected);
    (records, expected)
}

#[derive(Clone, Debug)]
enum ClaudeStep {
    User(String),
    Assistant(String, String),
    Thinking(String, String),
    ToolCall(String, String),
    ToolResult(String),
    System(String),
    Attachment(String),
    Streaming(String, String),
    Metadata,
}

fn claude_step() -> impl Strategy<Value = ClaudeStep> {
    prop_oneof![
        3 => text().prop_map(ClaudeStep::User),
        3 => (model(), text()).prop_map(|(model, text)| ClaudeStep::Assistant(model, text)),
        2 => (model(), text()).prop_map(|(model, text)| ClaudeStep::Thinking(model, text)),
        2 => (model(), text()).prop_map(|(model, text)| ClaudeStep::ToolCall(model, text)),
        2 => text().prop_map(ClaudeStep::ToolResult),
        1 => text().prop_map(ClaudeStep::System),
        1 => text().prop_map(ClaudeStep::Attachment),
        2 => (model(), text()).prop_map(|(model, text)| ClaudeStep::Streaming(model, text)),
        1 => Just(ClaudeStep::Metadata),
    ]
}

fn claude_history(steps: &[ClaudeStep]) -> (Vec<Value>, Vec<(String, Option<String>)>) {
    let mut records = Vec::new();
    let mut expected = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        match step {
            ClaudeStep::User(text) => {
                records.push(json!({"type":"user","uuid":format!("u{index}"),"message":{"role":"user","content":text}}));
                expected.push((text.clone(), None));
            }
            ClaudeStep::Assistant(model, text) => {
                records.push(json!({"type":"assistant","uuid":format!("a{index}"),"message":{"id":format!("msg_{index}"),"model":model,"content":[{"type":"text","text":text}]}}));
                expected.push((text.clone(), Some(model.clone())));
            }
            ClaudeStep::Thinking(model, text) => {
                records.push(json!({"type":"assistant","message":{"id":format!("msg_{index}"),"model":model,"content":[{"type":"thinking","thinking":text}]}}));
                expected.push((text.clone(), Some(model.clone())));
            }
            ClaudeStep::ToolCall(model, text) => {
                records.push(json!({"type":"assistant","message":{"id":format!("msg_{index}"),"model":model,"content":[{"type":"tool_use","name":"Search","input":{"query":text}}]}}));
                expected.push((
                    format!("Search\n{}", json!({"query":text})),
                    Some(model.clone()),
                ));
            }
            ClaudeStep::ToolResult(text) => {
                records.push(json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":text}]}]}}));
                expected.push((text.clone(), None));
            }
            ClaudeStep::System(text) => {
                records.push(json!({"type":"system","subtype":"hook_response","content":text}));
                expected.push((text.clone(), None));
            }
            ClaudeStep::Attachment(text) => {
                records.push(json!({"type":"attachment","attachment":{"type":"task_reminder","content":text}}));
                expected.push((text.clone(), None));
            }
            ClaudeStep::Streaming(model, text) => {
                let id = format!("msg_{index}");
                records.push(json!({"type":"assistant","message":{"id":id,"model":model,"content":[{"type":"text","text":"start"}]}}));
                records.push(json!({"type":"assistant","message":{"id":id,"model":model,"content":[{"type":"text","text":format!("start{text}end")}]}}));
                expected.push((format!("start{text}end"), Some(model.clone())));
            }
            ClaudeStep::Metadata => {
                records
                    .push(json!({"type":"progress","data":{"message":"not conversation content"}}));
                records.push(
                    json!({"type":"file-history-snapshot","snapshot":{"trackedFileBackups":{}}}),
                );
            }
        }
    }
    for (index, record) in records.iter_mut().enumerate() {
        record["sessionId"] = json!("session");
        record["uuid"] = json!(format!("uuid_{index}"));
        record["parentUuid"] = if index < 2 || index % 3 == 0 {
            Value::Null
        } else {
            json!(format!("uuid_{}", index - 1))
        };
    }
    backfill(&mut expected);
    (records, expected)
}

#[derive(Clone, Debug)]
enum PiStep {
    Model(String),
    User(String),
    Assistant(String, String),
    InheritedAssistant(String),
    ToolCall(String, String),
    ToolResult(String),
    System(String),
    Bash(String),
    Custom(String),
    Metadata(u8),
}

fn pi_step() -> impl Strategy<Value = PiStep> {
    prop_oneof![
        2 => model().prop_map(PiStep::Model),
        3 => text().prop_map(PiStep::User),
        3 => (model(), text()).prop_map(|(model, text)| PiStep::Assistant(model, text)),
        2 => text().prop_map(PiStep::InheritedAssistant),
        2 => (model(), text()).prop_map(|(model, text)| PiStep::ToolCall(model, text)),
        2 => text().prop_map(PiStep::ToolResult),
        2 => text().prop_map(PiStep::System),
        2 => text().prop_map(PiStep::Bash),
        2 => text().prop_map(PiStep::Custom),
        1 => (0_u8..6).prop_map(PiStep::Metadata),
    ]
}

fn pi_history(steps: &[PiStep]) -> (Vec<Value>, Vec<(String, Option<String>)>) {
    let mut records = vec![json!({"type":"session","version":3,"id":"session","cwd":"/tmp"})];
    let mut expected = Vec::new();
    let mut current = None;
    for (index, step) in steps.iter().enumerate() {
        let mut record = match step {
            PiStep::Model(model) => {
                current = Some(model.clone());
                json!({"type":"model_change","modelId":model,"provider":"openai"})
            }
            PiStep::User(text) => {
                expected.push((text.clone(), None));
                json!({"type":"message","message":{"role":"user","content":text}})
            }
            PiStep::Assistant(model, text) => {
                current = Some(model.clone());
                expected.push((text.clone(), current.clone()));
                json!({"type":"message","message":{"role":"assistant","model":model,"content":[{"type":"text","text":text}]}})
            }
            PiStep::InheritedAssistant(text) => {
                expected.push((text.clone(), current.clone()));
                json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":text}]}})
            }
            PiStep::ToolCall(model, text) => {
                current = Some(model.clone());
                expected.push((format!("read\n{}", json!({"path":text})), current.clone()));
                json!({"type":"message","message":{"role":"assistant","model":model,"content":[{"type":"toolCall","name":"read","arguments":{"path":text}}]}})
            }
            PiStep::ToolResult(text) => {
                expected.push((format!("read\n{text}"), None));
                json!({"type":"message","message":{"role":"toolResult","toolName":"read","content":[{"type":"text","text":text}]}})
            }
            PiStep::System(text) => {
                expected.push((json!({"preamble":text}).to_string(), None));
                json!({"type":"message","message":{"role":"system","content":"","sections":{"preamble":text}}})
            }
            PiStep::Bash(text) => {
                expected.push((format!("echo\n{text}"), None));
                json!({"type":"message","message":{"role":"bashExecution","command":"echo","output":text}})
            }
            PiStep::Custom(text) => {
                expected.push((text.clone(), None));
                json!({"type":"custom_message","content":text,"customType":"extension"})
            }
            PiStep::Metadata(kind) => match kind {
                0 => json!({"type":"usage","kind":"cache_warm","usage":{"totalTokens":999}}),
                1 => {
                    json!({"type":"compaction","summary":"past context","firstKeptEntryId":"entry_0"})
                }
                2 => json!({"type":"branch_summary","summary":"another branch","fromId":"entry_0"}),
                3 => json!({"type":"context_edit","targetId":"entry_0","replacement":null}),
                4 => {
                    json!({"type":"custom","customType":"state","data":{"text":"not model content"}})
                }
                _ => json!({"type":"thinking_level_change","thinkingLevel":"high"}),
            },
        };
        record["id"] = json!(format!("entry_{index}"));
        record["parentId"] = if index == 0 || index % 3 == 0 {
            Value::Null
        } else {
            json!(format!("entry_{}", index - 1 - usize::from(index % 3 == 2)))
        };
        records.push(record);
    }
    backfill(&mut expected);
    (records, expected)
}

fn backfill(expected: &mut [(String, Option<String>)]) {
    let mut next = None;
    for (_, model) in expected.iter_mut().rev() {
        if model.is_none() {
            model.clone_from(&next);
        } else {
            next.clone_from(model);
        }
    }
}

fn check(
    messages: &[Message],
    expected: &[(String, Option<String>)],
    harness: &str,
) -> TestCaseResult {
    prop_assert_eq!(messages.len(), expected.len());
    for (message, (body, model)) in messages.iter().zip(expected) {
        prop_assert_eq!(message.harness, harness);
        prop_assert_eq!(&message.body, body);
        prop_assert_eq!(&message.model, model);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn codex_whole_histories(steps in prop::collection::vec(codex_step(), 0..35)) {
        let (records, expected) = codex_history(&steps);
        check(&parse(&Codex, &records), &expected, "codex")?;
    }

    #[test]
    fn claude_whole_histories(steps in prop::collection::vec(claude_step(), 0..35)) {
        let (records, expected) = claude_history(&steps);
        check(&parse(&ClaudeCode, &records), &expected, "claude")?;
    }

    #[test]
    fn pi_whole_histories(steps in prop::collection::vec(pi_step(), 0..35)) {
        let (records, expected) = pi_history(&steps);
        check(&parse(&Pi, &records), &expected, "pi")?;
    }

    #[test]
    fn content_extraction_ignores_binary_data(text in text()) {
        let content = json!([
            {"type":"text", "text":text},
            {"type":"image", "data":"secret-image-bytes", "mimeType":"image/png"},
            {"type":"thinking", "thinking":"thought"}
        ]);
        let actual = super::content_text(&content);
        prop_assert_eq!(&actual, &format!("{text}\nthought"));
        prop_assert!(!actual.contains("secret-image-bytes"));
    }
}
