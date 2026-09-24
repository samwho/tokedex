use crate::{ModelReport, Report, TokenReport};
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const OPENAI: &str = "\x1b[96m";
const ANTHROPIC: &str = "\x1b[38;5;208m";
const YELLOW: &str = "\x1b[93m";
const LOGO_WIDTH: usize = 26;

// Text renderings of OpenAI's blossom and Anthropic's A/slash mark.
// The blossom was traced from OpenAI's 2025 monochrome brand asset.
const OPENAI_LOGO: &[&str] = &[
    "       #######",
    "     ##     ###  ###",
    "    ##  ###         ##",
    " ## ##  #   ### ###  #",
    "##  ##  #######     ##",
    "##  ##  #      ####   #",
    " #   ####      #  ##   #",
    "  ##     #######  ##  ##",
    "  #  ### ###   #  ## ##",
    "  ##         ###  ##",
    "    ###  ###     ##",
    "          #######",
    "          OPENAI",
];
const ANTHROPIC_LOGO: &[&str] = &[
    r"    /\          //",
    r"   /  \        //",
    r"  /    \      //",
    r" /  /\  \    //",
    r"/  /  \  \  //",
    r"|  |  |  |  //",
    r"|  |__|  | //",
    r"|        |//",
    r"|  |  |  |/",
    r"|__|  |__|",
    "",
    "  A N T H R O P I C",
];

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

fn percentage(value: f64) -> String {
    if value > 0.0 && value < 0.01 {
        format!("{value:.4}%")
    } else {
        format!("{value:.2}%")
    }
}

fn token(token: &TokenReport) -> String {
    let mut short: String = token.text.chars().take(14).collect();
    if token.text.chars().count() > 14 {
        short.push('…');
    }
    format!("#{} {}", token.id, serde_json::to_string(&short).unwrap())
}

fn fact(key: &str, value: &str, accent: &str, color: bool) -> String {
    format!("{}: {value}", paint(key, accent, color))
}

fn color_bars(color: bool) -> String {
    [
        "\x1b[30m", "\x1b[31m", "\x1b[32m", "\x1b[33m", "\x1b[34m", "\x1b[35m", "\x1b[36m",
        "\x1b[37m",
    ]
    .iter()
    .map(|code| paint("██", code, color))
    .collect::<Vec<_>>()
    .join("")
}

fn featured_model(report: &Report) -> Option<&ModelReport> {
    report
        .models
        .iter()
        .filter(|model| model.tokens.is_some())
        .max_by_key(|model| model.tokens.unwrap_or_default())
}

pub fn render_report(report: &Report, color: bool) -> String {
    let Some(model) = featured_model(report) else {
        return "\n  tokedex\n  No tokenized chat history found.\n".to_string();
    };
    let anthropic = model.model.starts_with("claude-");
    let (logo, provider, accent) = if anthropic {
        (ANTHROPIC_LOGO, "Anthropic", ANTHROPIC)
    } else {
        (OPENAI_LOGO, "OpenAI", OPENAI)
    };
    let mut info = vec![
        paint(&format!("tokedex@{provider}"), accent, color),
        paint(&"-".repeat(8 + 1 + provider.len()), DIM, color),
        fact("Model", &model.model, accent, color),
        fact("Messages", &number(model.messages), accent, color),
        fact(
            "Text tokens",
            &number(model.tokens.unwrap_or_default()),
            accent,
            color,
        ),
        fact(
            "Unique",
            &format!(
                "{} / {}",
                number(model.unique_tokens.unwrap_or_default()),
                number(model.vocabulary_size.unwrap_or_default())
            ),
            accent,
            color,
        ),
        fact(
            "Coverage",
            &format!(
                "{} of vocabulary",
                percentage(model.vocabulary_percent.unwrap_or_default())
            ),
            accent,
            color,
        ),
    ];
    if let Some(highest) = &model.highest_id_token {
        info.push(fact("Highest ID", &token(highest), accent, color));
    }
    if let Some(common) = &model.most_common_token {
        info.push(fact(
            "Most common",
            &format!("{} x{}", token(common), number(common.count)),
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
    if anthropic {
        info.push(paint("WARNING: old Claude tokenizer", YELLOW, color));
        info.push("Claude 3+ results are approximate".into());
    } else if model.model.starts_with("gpt-6") {
        info.push(paint("ASSUMED: GPT-5.6 tokenizer", YELLOW, color));
    }
    info.push(color_bars(color));

    let mut out = String::from("\n");
    for row in 0..logo.len().max(info.len()) {
        let art = logo.get(row).copied().unwrap_or("");
        writeln!(
            out,
            "  {}   {}",
            paint(&format!("{art:<LOGO_WIDTH$}"), accent, color),
            info.get(row).map(String::as_str).unwrap_or("")
        )
        .unwrap();
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(name: &str, tokens: usize) -> ModelReport {
        ModelReport {
            model: name.into(),
            status: "exact for visible text".into(),
            encoding: Some("o200k_base".into()),
            messages: 2,
            tokens: Some(tokens),
            unique_tokens: Some(2),
            vocabulary_size: Some(199_998),
            vocabulary_percent: Some(2.0 / 199_998.0 * 100.0),
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
        }
    }

    #[test]
    fn one_view_picks_most_tokens_and_openai_logo() {
        let report = Report {
            files_scanned: 2,
            unattributed_messages: 0,
            notes: vec![],
            models: vec![model("gpt-5.6-sol", 10), model("gpt-6-astra", 20)],
        };
        let output = render_report(&report, false);
        assert!(output.contains("OPENAI"));
        assert!(output.contains("gpt-6-astra"));
        assert!(!output.contains("gpt-5.6-sol"));
        assert!(output.contains("ASSUMED: GPT-5.6 tokenizer"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn claude_logo_and_warning() {
        let mut claude = model("claude-sonnet-4-5", 20);
        claude.encoding = Some("claude_legacy".into());
        let report = Report {
            files_scanned: 1,
            unattributed_messages: 0,
            notes: vec![],
            models: vec![claude, model("gpt-5.6-sol", 10)],
        };
        let output = render_report(&report, false);
        assert!(output.contains("A N T H R O P I C"));
        assert!(output.contains("WARNING: old Claude tokenizer"));
        assert!(output.contains("Claude 3+ results are approximate"));
        assert!(!output.contains("gpt-5.6-sol"));
        assert!(output.contains("0.0010% of vocabulary"));
    }
}
