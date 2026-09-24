use super::TiktokenTokenizer;
use crate::providers::ANTHROPIC;

pub(super) static CLAUDE: TiktokenTokenizer = TiktokenTokenizer::new(
    "claude_legacy",
    &ANTHROPIC,
    eligible,
    "claude_legacy",
    65_000,
    true,
);

fn eligible(model: &str) -> bool {
    model.to_ascii_lowercase().starts_with("claude-")
}
