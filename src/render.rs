use crate::{Report, TokenizerReport};
use console::Term;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};
use unicode_width::UnicodeWidthStr;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const OPENAI: &str = "\x1b[96m";
const ANTHROPIC: &str = "\x1b[38;5;208m";
const NEUTRAL: &str = "\x1b[97m";
const LOGO_WIDTH: usize = 26;
const INFO_COLUMN: usize = 2 + LOGO_WIDTH + 3;

pub fn print_report(report: &Report, allow_color: bool) {
    let color = allow_color && io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let terminal_width = usize::from(Term::stdout().size().1);
    print!("{}", render_report_width(report, color, terminal_width));
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

fn percentage(value: f64) -> String {
    if value > 0.0 && value < 0.01 {
        format!("{value:.4}%")
    } else {
        format!("{value:.2}%")
    }
}

fn escaped_chunks(text: &str) -> Vec<String> {
    let mut chunks = vec!["\"".to_string()];
    chunks.extend(text.chars().map(|character| match character {
        ' ' => " ".to_string(),
        '\t' => "\\t".to_string(),
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        character if character.is_whitespace() || character.is_control() => {
            character.escape_default().to_string()
        }
        character => character.to_string(),
    }));
    chunks.push("\"".to_string());
    chunks
}

fn token(text: &str, max_width: usize) -> String {
    let chunks = escaped_chunks(text);
    let width = chunks.iter().map(|chunk| chunk.width()).sum::<usize>();
    if width <= max_width {
        return chunks.concat();
    }
    if max_width <= 1 {
        return "…".to_string();
    }
    let available = max_width - 1;
    let left_width = available / 2;
    let right_width = available - left_width;
    let mut left = Vec::new();
    let mut used = 0;
    for chunk in &chunks {
        let width = chunk.width();
        if used + width > left_width {
            break;
        }
        left.push(chunk.as_str());
        used += width;
    }
    let mut right = Vec::new();
    used = 0;
    for chunk in chunks.iter().rev() {
        let width = chunk.width();
        if used + width > right_width {
            break;
        }
        right.push(chunk.as_str());
        used += width;
    }
    right.reverse();
    format!("{}…{}", left.concat(), right.concat())
}

fn fact(key: &str, value: &str, accent: &str, color: bool) -> String {
    format!("{}: {value}", paint(key, accent, color))
}

fn token_width(terminal_width: usize, key: &str, suffix: &str) -> usize {
    terminal_width
        .saturating_sub(INFO_COLUMN + key.width() + 2 + suffix.width())
        .max(1)
}

fn featured_tokenizer(report: &Report) -> Option<&TokenizerReport> {
    report
        .tokenizers
        .iter()
        .max_by_key(|tokenizer| tokenizer.tokens)
}

#[cfg(test)]
fn render_report(report: &Report, color: bool) -> String {
    render_report_width(report, color, 120)
}

fn render_report_width(report: &Report, color: bool, terminal_width: usize) -> String {
    let Some(featured) = featured_tokenizer(report) else {
        return "\n  tokedex\n  No tokenized chat history found.\n".to_string();
    };
    let anthropic = featured.provider == "anthropic";
    let logo = featured.ascii_art_logo;
    let accent = if anthropic { ANTHROPIC } else { OPENAI };
    let mut info = vec![
        paint("tokedex", NEUTRAL, color),
        paint("-------", DIM, color),
        fact("Total messages", &number(report.messages), accent, color),
        fact("Text tokens", &number(report.tokens), accent, color),
    ];
    if let Some(model) = &report.most_common_model {
        info.push(fact("Most common model", &model.model, accent, color));
    }
    if let Some(longest) = &report.longest_token {
        let key = "Longest seen token";
        info.push(fact(
            key,
            &token(&longest.text, token_width(terminal_width, key, "")),
            accent,
            color,
        ));
    }
    if let Some(highest) = &report.token_with_highest_id {
        let key = "Token with highest ID";
        let prefix = format!("{} #{} ", highest.tokenizer, highest.id);
        info.push(fact(
            key,
            &format!(
                "{}{}",
                prefix,
                token(&highest.text, token_width(terminal_width, key, &prefix))
            ),
            accent,
            color,
        ));
    }
    if let Some(common) = &report.most_common_token {
        let key = "Most seen token";
        let suffix = format!(" x{}", number(common.count));
        info.push(fact(
            key,
            &format!(
                "{}{}",
                token(&common.text, token_width(terminal_width, key, &suffix)),
                suffix
            ),
            accent,
            color,
        ));
    }
    info.push(fact(
        "Files scanned",
        &number(report.files_scanned),
        accent,
        color,
    ));
    info.push(String::new());
    info.push(paint("Tokenizers", NEUTRAL, color));
    info.push(paint("----------", DIM, color));
    for tokenizer in &report.tokenizers {
        info.push(fact(
            &tokenizer.tokenizer,
            &format!(
                "{} ({} / {})",
                percentage(tokenizer.vocabulary_percent),
                number(tokenizer.unique_tokens),
                number(tokenizer.vocabulary_size)
            ),
            accent,
            color,
        ));
    }

    let mut out = String::from("\n");
    for row in 0..logo.len().max(info.len()) {
        let art = logo.get(row).copied().unwrap_or("");
        writeln!(
            out,
            "  {}   {}",
            paint(&format!("{art:<LOGO_WIDTH$}"), accent, color),
            info.get(row).map_or("", String::as_str)
        )
        .expect("writing to a String should be infallible");
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ANTHROPIC, OPENAI, Provider};
    use crate::{HighestIdReport, ModelUsageReport, TokenReport};

    fn tokenizer(name: &str, provider: &str, tokens: usize) -> TokenizerReport {
        let provider_impl: &dyn Provider = if provider == "anthropic" {
            &ANTHROPIC
        } else {
            &OPENAI
        };
        TokenizerReport {
            tokenizer: name.into(),
            provider: provider.into(),
            ascii_art_logo: provider_impl.ascii_art_logo(),
            messages: 2,
            tokens,
            unique_tokens: 2,
            vocabulary_size: 100,
            vocabulary_percent: 2.0,
        }
    }

    fn report() -> Report {
        Report {
            files_scanned: 2,
            messages: 4,
            tokens: 30,
            tokenizers: vec![
                tokenizer("o200k_base", "openai", 20),
                tokenizer("claude_legacy", "anthropic", 10),
            ],
            most_common_model: Some(ModelUsageReport {
                model: "gpt-5.6-sol".into(),
            }),
            longest_token: Some(TokenReport {
                text: "extraordinary".into(),
                count: 1,
            }),
            most_common_token: Some(TokenReport {
                text: " \t\n".into(),
                count: 12,
            }),
            token_with_highest_id: Some(HighestIdReport {
                tokenizer: "o200k_base".into(),
                id: 150_000,
                text: "example".into(),
            }),
            notes: vec![],
        }
    }

    #[test]
    fn renders_aggregate_tokenizer_stats() {
        let output = render_report(&report(), false);
        assert!(output.contains("⣠⣴⣾⠿⠿⣿⣶"));
        assert!(output.contains("Total messages: 4"));
        assert!(output.contains("Most common model: gpt-5.6-sol"));
        assert!(!output.contains("gpt-5.6-sol x"));
        assert!(output.contains("Longest seen token: \"extraordinary\""));
        assert!(output.contains("Most seen token: \" \\t\\n\" x12"));
        assert!(output.contains("Token with highest ID: o200k_base #150000 \"example\""));
        assert!(output.contains("Tokenizers\n"));
        assert!(output.contains("o200k_base: 2.00% (2 / 100)"));
        assert!(!output.contains("WARNING"));
        assert!(!output.contains("ASSUMED"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn headings_use_neutral_color() {
        let output = render_report(&report(), true);
        assert!(output.contains("\x1b[97mtokedex\x1b[0m"));
        assert!(output.contains("\x1b[97mTokenizers\x1b[0m"));
        assert!(output.contains("\x1b[96mTotal messages\x1b[0m"));
    }

    #[test]
    fn truncates_the_middle_of_long_tokens_to_terminal_width() {
        let mut report = report();
        report.longest_token = Some(TokenReport {
            text: "abcdefghij0123456789ABCDEFGHIJ".into(),
            count: 1,
        });
        let output = render_report_width(&report, false, 60);
        assert!(output.contains("Longest seen token: \"abc…HIJ\""));
        assert!(!output.contains("0123456789"));
    }
}
