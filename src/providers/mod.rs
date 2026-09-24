mod anthropic;
mod openai;

pub use anthropic::ANTHROPIC;
pub use openai::OPENAI;

pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn ascii_art_logo(&self) -> &'static [&'static str];
}

pub static PROVIDERS: [&dyn Provider; 2] = [&OPENAI, &ANTHROPIC];
