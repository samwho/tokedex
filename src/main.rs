use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

mod harness;
mod providers;
mod render;
mod tokenizers;

use harness::{Harness, Message};
use providers::{PROVIDERS, Provider};
use tokenizers::{TOKENIZERS, Tokenizer};

#[derive(Parser)]
#[command(about = "Measure text token vocabulary coverage in Codex, Claude Code, and pi histories")]
struct Args {
    /// Only scan this harness; repeat to select more than one
    #[arg(long, value_name = "HARNESS")]
    harness: Vec<String>,
    /// Override the default ~/.codex directory
    #[arg(long)]
    codex_dir: Option<PathBuf>,
    /// Override the default ~/.claude/projects directory
    #[arg(long)]
    claude_dir: Option<PathBuf>,
    /// Override the default ~/.pi/agent/sessions directory
    #[arg(long)]
    pi_dir: Option<PathBuf>,
    /// Only tokenize models from this provider; repeat to select more than one
    #[arg(long, value_name = "PROVIDER")]
    provider: Vec<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
    /// Disable terminal colors
    #[arg(long)]
    no_color: bool,
}

#[derive(Default)]
struct Counts {
    messages: usize,
    tokens: usize,
    by_id: HashMap<u32, usize>,
    by_model: HashMap<String, usize>,
}

impl Counts {
    fn merge(&mut self, other: Self) {
        self.messages += other.messages;
        self.tokens += other.tokens;
        for (id, count) in other.by_id {
            *self.by_id.entry(id).or_default() += count;
        }
        for (model, count) in other.by_model {
            *self.by_model.entry(model).or_default() += count;
        }
    }
}

#[derive(Serialize)]
struct TokenReport {
    text: String,
    count: usize,
}

#[derive(Serialize)]
struct ModelUsageReport {
    model: String,
}

#[derive(Serialize)]
struct HighestIdReport {
    tokenizer: String,
    id: u32,
    text: String,
}

#[derive(Serialize)]
struct TokenizerReport {
    tokenizer: String,
    provider: String,
    #[serde(skip)]
    ascii_art_logo: &'static [&'static str],
    messages: usize,
    tokens: usize,
    unique_tokens: usize,
    vocabulary_size: usize,
    vocabulary_percent: f64,
}

#[derive(Serialize)]
struct Report {
    files_scanned: usize,
    messages: usize,
    tokens: usize,
    tokenizers: Vec<TokenizerReport>,
    most_common_model: Option<ModelUsageReport>,
    longest_token: Option<TokenReport>,
    most_common_token: Option<TokenReport>,
    token_with_highest_id: Option<HighestIdReport>,
    notes: Vec<&'static str>,
}

const PLAIN_BAR_TEMPLATE: &str = "{msg} [{bar:30}] {pos}/{len} ({percent}%)";
const COLOR_BAR_TEMPLATE: &str = "{msg} [{bar:30.cyan/blue}] {pos}/{len} ({percent}%)";

struct ConversationFile<'a> {
    harness: &'a dyn Harness,
    path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let home = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?);
    let selected_providers = select_providers(&args.provider)?;
    let selected_tokenizers: Vec<&'static dyn Tokenizer> = TOKENIZERS
        .iter()
        .copied()
        .filter(|tokenizer| {
            selected_providers
                .iter()
                .any(|provider| provider.name() == tokenizer.provider().name())
        })
        .collect();

    let harnesses = harness::harnesses();
    let selected_harnesses = select_harnesses(&args.harness, &harnesses)?;
    let progress = scan_progress(args.json, args.no_color);
    let (parsed, files_scanned) = scan_histories(
        &args,
        &home,
        selected_harnesses,
        &selected_tokenizers,
        &progress,
    );

    let tokenizer_objects: HashMap<String, &'static dyn Tokenizer> = parsed
        .iter()
        .flatten()
        .filter_map(|message| message.model.as_ref())
        .filter_map(|name| {
            selected_tokenizers
                .iter()
                .copied()
                .find(|tokenizer| tokenizer.eligible(name))
                .map(|tokenizer| (name.clone(), tokenizer))
        })
        .collect();
    progress.set_style(spinner_style(args.no_color));
    progress.set_message("Preparing tokenizers…");
    for tokenizer in tokenizer_objects.values() {
        progress.set_message(format!("Preparing {}…", tokenizer.name()));
        tokenizer.prepare()?;
    }

    let file_count = parsed.len();
    start_bar(&progress, file_count, "Tokenizing files", args.no_color);
    let grouped = parsed
        .into_par_iter()
        .map(|messages| {
            let result = aggregate_messages(messages, &tokenizer_objects);
            progress.inc(1);
            result
        })
        .try_reduce(BTreeMap::new, |mut left, right| {
            for (tokenizer, counts) in right {
                left.entry(tokenizer).or_default().merge(counts);
            }
            Ok(left)
        })?;
    let report = build_report(
        grouped,
        files_scanned,
        &selected_tokenizers,
        &progress,
        args.no_color,
    )?;
    progress.finish_and_clear();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render::print_report(&report, !args.no_color);
    }
    Ok(())
}

fn scan_histories(
    args: &Args,
    home: &Path,
    harnesses: Vec<&dyn Harness>,
    tokenizers: &[&dyn Tokenizer],
    progress: &ProgressBar,
) -> (Vec<Vec<Message>>, usize) {
    let mut files = Vec::new();
    for harness in harnesses {
        progress.set_message(format!("Finding {} histories…", harness.label()));
        let root = harness_root_override(harness.name(), args)
            .unwrap_or_else(|| harness.default_root(home));
        files.extend(
            harness
                .conversation_files(&root)
                .into_iter()
                .map(|path| ConversationFile { harness, path }),
        );
    }
    let total_files = files.len();
    start_bar(progress, total_files, "Scanning files", args.no_color);
    let files_scanned = AtomicUsize::new(0);
    let parsed = files
        .par_iter()
        .filter_map(|file| {
            let result = match file.harness.parse_file(&file.path) {
                Ok(mut messages) => {
                    messages.retain(|message| {
                        message.model.as_deref().is_some_and(|name| {
                            tokenizers.iter().any(|tokenizer| tokenizer.eligible(name))
                        })
                    });
                    Some(messages)
                }
                Err(err) => {
                    eprintln!("warning: {}: {err:#}", file.path.display());
                    None
                }
            };
            files_scanned.fetch_add(1, Ordering::Relaxed);
            progress.inc(1);
            result
        })
        .collect();
    (parsed, files_scanned.load(Ordering::Relaxed))
}

fn build_report(
    mut grouped: BTreeMap<String, Counts>,
    files_scanned: usize,
    selected_tokenizers: &[&dyn Tokenizer],
    progress: &ProgressBar,
    no_color: bool,
) -> Result<Report> {
    let unique_ids = grouped.values().map(|counts| counts.by_id.len()).sum();
    start_bar(progress, unique_ids, "Decoding token IDs", no_color);
    let mut highest_id: Option<HighestIdReport> = None;
    let mut text_counts = HashMap::<String, usize>::new();
    let mut model_counts = HashMap::<String, usize>::new();
    let mut messages = 0;
    let mut tokens = 0;
    let mut tokenizers = Vec::new();
    for tokenizer in selected_tokenizers {
        let counts = grouped.remove(tokenizer.name()).unwrap_or_default();
        if counts.tokens == 0 {
            continue;
        }
        messages += counts.messages;
        tokens += counts.tokens;
        for (model, &count) in &counts.by_model {
            *model_counts.entry(model.clone()).or_default() += count;
        }
        for (&id, &count) in &counts.by_id {
            let text = tokenizer.decode(id)?;
            if highest_id.as_ref().is_none_or(|highest| id > highest.id) {
                highest_id = Some(HighestIdReport {
                    tokenizer: tokenizer.name().to_string(),
                    id,
                    text: text.clone(),
                });
            }
            *text_counts.entry(text).or_default() += count;
            progress.inc(1);
        }
        let vocabulary_size = tokenizer.vocabulary_size();
        let unique_tokens = counts.by_id.len();
        tokenizers.push(TokenizerReport {
            tokenizer: tokenizer.name().to_string(),
            provider: tokenizer.provider().name().to_string(),
            ascii_art_logo: tokenizer.provider().ascii_art_logo(),
            messages: counts.messages,
            tokens: counts.tokens,
            unique_tokens,
            vocabulary_size,
            vocabulary_percent: percentage(unique_tokens, vocabulary_size),
        });
    }
    let most_common_model = model_counts
        .into_iter()
        .max_by(|(a_model, a_count), (b_model, b_count)| {
            a_count.cmp(b_count).then_with(|| b_model.cmp(a_model))
        })
        .map(|(model, _)| ModelUsageReport { model });
    let longest_token = text_counts
        .iter()
        .max_by(|(a, _), (b, _)| {
            a.chars()
                .count()
                .cmp(&b.chars().count())
                .then_with(|| a.cmp(b))
        })
        .map(|(text, &count)| TokenReport {
            text: text.clone(),
            count,
        });
    let most_common_token = text_counts
        .into_iter()
        .max_by(|(a_text, a_count), (b_text, b_count)| {
            a_count.cmp(b_count).then_with(|| b_text.cmp(a_text))
        })
        .map(|(text, count)| TokenReport { text, count });
    let notes = vec![
        "Counts aggregate all saved textual model input and output across selected harnesses, including system content, reasoning, tool calls, tool results, and saved tool schemas. Binary media and unavailable encrypted content are excluded.",
        "Tokens with identical decoded text are merged for the overall most-common result.",
    ];
    Ok(Report {
        files_scanned,
        messages,
        tokens,
        tokenizers,
        most_common_model,
        longest_token,
        most_common_token,
        token_with_highest_id: highest_id,
        notes,
    })
}

fn percentage(value: usize, total: usize) -> f64 {
    let value = u32::try_from(value).expect("tokenizer vocabulary size must fit in u32");
    let total = u32::try_from(total).expect("tokenizer vocabulary size must fit in u32");
    f64::from(value) * 100.0 / f64::from(total)
}

fn select_harnesses<'a>(
    requested: &[String],
    harnesses: &'a [Box<dyn Harness>],
) -> Result<Vec<&'a dyn Harness>> {
    if requested.is_empty() {
        return Ok(harnesses.iter().map(Box::as_ref).collect());
    }
    let mut selected = Vec::new();
    for name in requested {
        let harness = harnesses
            .iter()
            .map(Box::as_ref)
            .find(|harness| harness.name().eq_ignore_ascii_case(name))
            .with_context(|| {
                format!(
                    "unknown harness {name:?}; available harnesses: {}",
                    harnesses
                        .iter()
                        .map(|harness| harness.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        if !selected
            .iter()
            .any(|existing: &&dyn Harness| existing.name() == harness.name())
        {
            selected.push(harness);
        }
    }
    Ok(selected)
}

fn select_providers(requested: &[String]) -> Result<Vec<&'static dyn Provider>> {
    if requested.is_empty() {
        return Ok(PROVIDERS.to_vec());
    }
    let mut selected = Vec::new();
    for name in requested {
        let provider = PROVIDERS
            .iter()
            .copied()
            .find(|provider| provider.name().eq_ignore_ascii_case(name))
            .with_context(|| {
                format!(
                    "unknown provider {name:?}; available providers: {}",
                    PROVIDERS
                        .iter()
                        .map(|provider| provider.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        if !selected
            .iter()
            .any(|existing: &&dyn Provider| existing.name() == provider.name())
        {
            selected.push(provider);
        }
    }
    Ok(selected)
}

fn harness_root_override(name: &str, args: &Args) -> Option<PathBuf> {
    match name {
        "codex" => args.codex_dir.clone(),
        "claude" => args.claude_dir.clone(),
        "pi" => args.pi_dir.clone(),
        _ => None,
    }
}

fn aggregate_messages(
    messages: Vec<Message>,
    tokenizers: &HashMap<String, &'static dyn Tokenizer>,
) -> Result<BTreeMap<String, Counts>> {
    let mut grouped = BTreeMap::<String, Counts>::new();
    for message in messages {
        debug_assert!(!message.harness.is_empty());
        let Some(model_name) = message.model else {
            continue;
        };
        let Some(tokenizer) = tokenizers.get(&model_name) else {
            continue;
        };
        let counts = grouped.entry(tokenizer.name().to_string()).or_default();
        counts.messages += 1;
        *counts.by_model.entry(model_name).or_default() += 1;
        for id in tokenizer.tokenize(&message.body)? {
            counts.tokens += 1;
            *counts.by_id.entry(id).or_default() += 1;
        }
    }
    Ok(grouped)
}

fn spinner_style(no_color: bool) -> ProgressStyle {
    let template = if no_color {
        "{spinner} {msg}"
    } else {
        "{spinner:.cyan} {msg}"
    };
    ProgressStyle::with_template(template)
        .expect("valid progress template")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
}

fn start_bar(progress: &ProgressBar, total: usize, label: &str, no_color: bool) {
    progress.set_style(
        ProgressStyle::with_template(if no_color {
            PLAIN_BAR_TEMPLATE
        } else {
            COLOR_BAR_TEMPLATE
        })
        .expect("valid progress template")
        .progress_chars("=>-"),
    );
    progress.set_length(u64::try_from(total).expect("work count must fit in u64"));
    progress.set_position(0);
    progress.set_message(label.to_string());
}

fn scan_progress(json: bool, no_color: bool) -> ProgressBar {
    if json || !io::stderr().is_terminal() {
        return ProgressBar::hidden();
    }
    let progress = ProgressBar::new_spinner();
    progress.set_style(spinner_style(no_color));
    progress.set_message("Finding conversation histories…");
    progress.enable_steady_tick(Duration::from_millis(80));
    progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_filter_accepts_repeated_values() {
        let selected = select_providers(&["OPENAI".into(), "anthropic".into()])
            .expect("known providers should resolve");
        assert_eq!(
            selected
                .iter()
                .map(|provider| provider.name())
                .collect::<Vec<_>>(),
            vec!["openai", "anthropic"]
        );
        assert!(select_providers(&["missing".into()]).is_err());
    }

    #[test]
    fn harness_filter_accepts_repeated_values() {
        let all = harness::harnesses();
        let selected = select_harnesses(&["pi".into(), "codex".into()], &all)
            .expect("known harnesses should resolve");
        assert_eq!(
            selected.iter().map(|h| h.name()).collect::<Vec<_>>(),
            vec!["pi", "codex"]
        );
        assert!(select_harnesses(&["missing".into()], &all).is_err());
    }
}
