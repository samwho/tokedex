use crate::{ModelReport, Report, TokenReport};
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

const CYAN: &str = "\x1b[96m";
const PURPLE: &str = "\x1b[95m";
const YELLOW: &str = "\x1b[93m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn print_report(report: &Report, allow_color: bool) {
    let color = allow_color && io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    print!("{}", render_report(report, color));
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

fn number(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn quantity(value: usize, singular: &str) -> String {
    format!(
        "{} {}{}",
        number(value),
        singular,
        if value == 1 { "" } else { "s" }
    )
}

fn token(token: &TokenReport) -> String {
    let mut short: String = token.text.chars().take(18).collect();
    if token.text.chars().count() > 18 {
        short.push('…');
    }
    format!("#{} {}", token.id, serde_json::to_string(&short).unwrap())
}

fn label(model: &ModelReport) -> (&'static str, &'static str) {
    if model.encoding.as_deref() == Some("claude_legacy") {
        ("OLD CLAUDE ≈", YELLOW)
    } else if model.status.starts_with("assumed:") {
        ("ASSUMED ≈", YELLOW)
    } else if model.encoding.is_some() {
        ("TEXT", CYAN)
    } else {
        ("UNKNOWN", DIM)
    }
}

pub fn render_report(report: &Report, color: bool) -> String {
    let mut out = String::new();
    let model_count = report.models.len();
    let measured = report
        .models
        .iter()
        .filter(|m| m.unique_tokens.is_some())
        .count();
    let art = [
        "     ▄▄▄▄▄▄▄     ",
        "   ▄██ 01 ██▄   ",
        "  ██ 10 11 ██   ",
        "   ▀██ 00 ██▀   ",
        "     ▀▀▀▀▀▀     ",
    ];
    let facts = [
        paint("TOKEDEX", PURPLE, color),
        paint("──────────────", DIM, color),
        format!(
            "{}  ·  {}",
            quantity(report.files_scanned, "session"),
            quantity(model_count, "model")
        ),
        format!("{} measured  ·  saved chat text", number(measured)),
        format!(
            "{}  {}  {}  {}",
            paint("██", CYAN, color),
            paint("██", PURPLE, color),
            paint("██", YELLOW, color),
            paint("██", DIM, color)
        ),
    ];
    out.push('\n');
    for (art_line, fact) in art.iter().zip(facts) {
        writeln!(out, "{}  {fact}", paint(art_line, CYAN, color)).unwrap();
    }
    for model in &report.models {
        let (tag, tone) = label(model);
        writeln!(
            out,
            "\n  {} {}  {}",
            paint("┌", DIM, color),
            paint(&model.model, PURPLE, color),
            paint(&format!("[{tag}]"), tone, color)
        )
        .unwrap();
        if let (Some(unique), Some(size), Some(percent)) = (
            model.unique_tokens,
            model.vocabulary_size,
            model.vocabulary_percent,
        ) {
            writeln!(
                out,
                "  {} vocab  {:>6.2}%  {} / {}",
                paint("│", DIM, color),
                percent,
                number(unique),
                number(size)
            )
            .unwrap();
            writeln!(
                out,
                "  {} {}  ·  {} text tokens",
                paint("│", DIM, color),
                quantity(model.messages, "message"),
                number(model.tokens.unwrap_or_default())
            )
            .unwrap();
            if let Some(highest) = &model.highest_id_token {
                writeln!(
                    out,
                    "  {} high   {}",
                    paint("│", DIM, color),
                    token(highest)
                )
                .unwrap();
            }
            if let Some(common) = &model.most_common_token {
                writeln!(
                    out,
                    "  {} top    {}  ×{}",
                    paint("└", DIM, color),
                    token(common),
                    number(common.count)
                )
                .unwrap();
            }
        } else {
            writeln!(
                out,
                "  {} {}  ·  tokenizer unavailable",
                paint("└", DIM, color),
                quantity(model.messages, "message")
            )
            .unwrap();
        }
    }
    if report
        .models
        .iter()
        .any(|m| m.encoding.as_deref() == Some("claude_legacy"))
    {
        writeln!(
            out,
            "\n  {} Claude uses Anthropic's OLD tokenizer.",
            paint("⚠", YELLOW, color)
        )
        .unwrap();
        writeln!(out, "    Claude 3+ results are approximate.").unwrap();
    }
    if report.unattributed_messages > 0 {
        writeln!(
            out,
            "  {} messages lacked a model.",
            number(report.unattributed_messages)
        )
        .unwrap();
    }
    writeln!(
        out,
        "  {}",
        paint(
            "Text only · no hidden reasoning, tools, or images",
            DIM,
            color
        )
    )
    .unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn banner_warns_about_legacy_claude() {
        let report = Report {
            files_scanned: 1,
            unattributed_messages: 0,
            notes: vec![],
            models: vec![ModelReport {
                model: "claude-sonnet-4-5".into(),
                status: "approximate".into(),
                encoding: Some("claude_legacy".into()),
                messages: 2,
                tokens: Some(3),
                unique_tokens: Some(2),
                vocabulary_size: Some(65_000),
                vocabulary_percent: Some(2.0 / 65_000.0 * 100.0),
                highest_id_token: Some(TokenReport {
                    id: 100,
                    text: "hello".into(),
                    count: 1,
                }),
                most_common_token: Some(TokenReport {
                    id: 5,
                    text: "!".into(),
                    count: 2,
                }),
            }],
        };
        let output = render_report(&report, false);
        assert!(output.contains("Claude 3+ results are approximate"));
        assert!(output.contains("#100 \"hello\""));
        assert!(!output.contains("\x1b["));
    }
}
