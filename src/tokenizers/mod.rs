mod claude;
mod openai_cl100k;
mod openai_o200k;

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base};
use unicode_normalization::UnicodeNormalization;

use crate::providers::Provider;
use claude::CLAUDE;
use openai_cl100k::OPENAI_CL100K;
use openai_o200k::OPENAI_O200K;

pub static TOKENIZERS: [&dyn Tokenizer; 3] = [&OPENAI_O200K, &OPENAI_CL100K, &CLAUDE];

pub trait Tokenizer: Send + Sync {
    fn name(&self) -> &'static str;
    fn provider(&self) -> &'static dyn Provider;
    fn eligible(&self, model: &str) -> bool;
    fn vocabulary_size(&self) -> usize;
    fn prepare(&self) -> Result<()>;
    fn tokenize(&self, text: &str) -> Result<Vec<u32>>;
    fn decode(&self, id: u32) -> Result<String>;
}

static TOKENIZER_CACHE: LazyLock<TokenizerCache> = LazyLock::new(TokenizerCache::default);

#[derive(Default)]
struct TokenizerCache {
    values: Mutex<HashMap<&'static str, Arc<CoreBPE>>>,
}

impl TokenizerCache {
    fn get(&self, encoding: &'static str) -> Result<Arc<CoreBPE>> {
        let cached = self
            .values
            .lock()
            .expect("tokenizer cache lock poisoned")
            .get(encoding)
            .cloned();
        if let Some(value) = cached {
            return Ok(value);
        }
        let value = Arc::new(match encoding {
            "cl100k_base" => cl100k_base()?,
            "o200k_base" => o200k_base()?,
            "claude_legacy" => claude_legacy()?,
            _ => anyhow::bail!("unknown tokenizer {encoding}"),
        });
        self.values
            .lock()
            .expect("tokenizer cache lock poisoned")
            .insert(encoding, Arc::clone(&value));
        Ok(value)
    }
}

pub struct TiktokenTokenizer {
    name: &'static str,
    provider: &'static dyn Provider,
    eligible: fn(&str) -> bool,
    encoding: &'static str,
    vocabulary_size: usize,
    normalize_nfkc: bool,
}

impl TiktokenTokenizer {
    pub const fn new(
        name: &'static str,
        provider: &'static dyn Provider,
        eligible: fn(&str) -> bool,
        encoding: &'static str,
        vocabulary_size: usize,
        normalize_nfkc: bool,
    ) -> Self {
        Self {
            name,
            provider,
            eligible,
            encoding,
            vocabulary_size,
            normalize_nfkc,
        }
    }
}

impl Tokenizer for TiktokenTokenizer {
    fn name(&self) -> &'static str {
        self.name
    }

    fn provider(&self) -> &'static dyn Provider {
        self.provider
    }

    fn eligible(&self, model: &str) -> bool {
        (self.eligible)(model)
    }

    fn vocabulary_size(&self) -> usize {
        self.vocabulary_size
    }

    fn prepare(&self) -> Result<()> {
        TOKENIZER_CACHE.get(self.encoding).map(|_| ())
    }

    fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        let bpe = TOKENIZER_CACHE.get(self.encoding)?;
        Ok(if self.normalize_nfkc {
            bpe.encode_with_special_tokens(&text.nfkc().collect::<String>())
        } else {
            bpe.encode_ordinary(text)
        })
    }

    fn decode(&self, id: u32) -> Result<String> {
        let bytes = TOKENIZER_CACHE
            .get(self.encoding)?
            .decode_bytes(&[id])
            .unwrap_or_default();
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[derive(Deserialize)]
struct ClaudeVocabulary {
    pat_str: String,
    bpe_ranks: String,
    special_tokens: FxHashMap<String, u32>,
}

fn claude_legacy() -> Result<CoreBPE> {
    let data: ClaudeVocabulary = serde_json::from_str(include_str!("../../assets/claude.json"))?;
    let mut encoder = FxHashMap::default();
    for line in data.bpe_ranks.lines() {
        let mut words = line.split_whitespace();
        words.next().context("missing Claude vocabulary prefix")?;
        let offset: u32 = words
            .next()
            .context("missing Claude rank offset")?
            .parse()?;
        for (index, token) in words.enumerate() {
            encoder.insert(
                STANDARD.decode(token)?,
                offset + u32::try_from(index).context("Claude vocabulary index exceeds u32")?,
            );
        }
    }
    CoreBPE::new(encoder, data.special_tokens, &data.pat_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ANTHROPIC, OPENAI};

    #[test]
    fn tokenizers_determine_model_eligibility() {
        assert_eq!(TOKENIZERS.len(), 3);
        assert_eq!(OPENAI_O200K.provider().name(), OPENAI.name());
        assert_eq!(CLAUDE.provider().name(), ANTHROPIC.name());
        assert!(OPENAI_O200K.eligible("gpt-6-astra"));
        assert!(OPENAI_O200K.eligible("gpt-5.6-sol"));
        assert!(CLAUDE.eligible("claude-opus-4-6"));
    }

    #[test]
    fn claude_tokenizer_normalizes_nfkc() {
        assert_eq!(
            CLAUDE.tokenize("Ａ").expect("full-width A should tokenize"),
            CLAUDE.tokenize("A").expect("ASCII A should tokenize")
        );
        assert_eq!(
            CLAUDE
                .tokenize("<META>")
                .expect("special token should tokenize"),
            vec![1]
        );
    }
}
