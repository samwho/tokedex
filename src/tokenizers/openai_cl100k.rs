use super::TiktokenTokenizer;
use crate::providers::OPENAI;

pub(super) static OPENAI_CL100K: TiktokenTokenizer = TiktokenTokenizer::new(
    "cl100k_base",
    &OPENAI,
    eligible,
    "cl100k_base",
    100_256,
    false,
);

fn eligible(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model == "gpt-4"
        || model.starts_with("gpt-4-")
        || model.starts_with("gpt-4-turbo")
        || model.starts_with("gpt-3.5")
        || model.starts_with("gpt-35-")
}
