use super::TiktokenTokenizer;
use crate::providers::OPENAI;

pub(super) static OPENAI_O200K: TiktokenTokenizer = TiktokenTokenizer::new(
    "o200k_base",
    &OPENAI,
    eligible,
    "o200k_base",
    199_998,
    false,
);

fn eligible(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.starts_with("gpt-6")
        || model.starts_with("gpt-5")
        || model.starts_with("gpt-4o")
        || model.starts_with("gpt-4.1")
        || model.starts_with("gpt-4.5")
        || model.starts_with("chatgpt-4o")
        || model.starts_with("codex-mini")
        || model == "o1"
        || model.starts_with("o1-")
        || model == "o3"
        || model.starts_with("o3-")
        || model == "o4-mini"
        || model.starts_with("o4-mini-")
}
