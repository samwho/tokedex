# tokedex

`tokedex` scans local Codex, Claude Code, and pi JSONL conversations and measures how much of each tokenizer's vocabulary appears in the saved user and assistant text.

For each measured model, it shows:

- Unique token IDs seen, divided by the tokenizer's vocabulary size (ordinary text IDs for OpenAI; all loaded IDs for the legacy Claude tokenizer).
- The largest token ID seen (the “higher ID token”), with its decoded text.
- The most frequent token ID, with its decoded text and count.

Token IDs are counted separately per model, even when models share an encoding. Messages are encoded independently. Only saved user and assistant text is included; hidden reasoning, image/audio payloads, tools, protocol tokens, and repeated prompt context are excluded. This is a vocabulary exploration tool, not a billing or total context counter. Output is local by default; the CLI makes no API requests.

The default output is a compact terminal card inspired by neofetch, suitable for a screenshot. Colors appear when stdout is a terminal; use `--no-color` for plain text or `--json` for scripts.

```text
     ▄▄▄▄▄▄▄       TOKEDEX
   ▄██ 01 ██▄     ──────────────
  ██ 10 11 ██      41 sessions  ·  3 models
   ▀██ 00 ██▀      3 measured  ·  saved chat text
     ▀▀▀▀▀▀       ██  ██  ██  ██

  ┌ claude-sonnet-4-5  [OLD CLAUDE ≈]
  │ vocab    2.40%  1,560 / 65,000
  │ 42 messages  ·  8,000 text tokens
  │ high   #64900 "example"
  └ top    #123 " the"  ×340

  ⚠ Claude uses Anthropic's OLD tokenizer.
    Claude 3+ results are approximate.
```

The figures in this example are illustrative.

## Install and run

```sh
cargo install --path .
tokedex
```

Defaults: `~/.codex/sessions` and `~/.codex/archived_sessions`, `~/.claude/projects`, and `~/.pi/agent/sessions`. Missing directories are skipped. To inspect different copies:

```sh
tokedex --codex-dir /path/to/.codex --claude-dir /path/to/projects --pi-dir /path/to/sessions
```

Use `--json` for structured output. Use `--assume-gpt6-o200k` only if you want **provisional** GPT-6 figures using `o200k_base`; the output marks them as assumed. The published `tiktoken` model table does not yet verify this mapping.

## Tokenizer coverage

| Model family | Encoding | Status |
| --- | --- | --- |
| GPT-5 and 5.x, GPT-4o, GPT-4.1, GPT-4.5, o1/o3/o4-mini, codex-mini | `o200k_base` | Text token IDs available in Rust |
| GPT-4, GPT-4 Turbo, GPT-3.5 | `cl100k_base` | Text token IDs available in Rust |
| GPT-6 | Unknown in published `tiktoken` mapping | Excluded by default; optional assumed mapping |
| Claude 3 through current Claude | Anthropic's archived Claude tokenizer | Approximate only; prominent warning in terminal and JSON output |

The [OpenAI `tiktoken` model table](https://github.com/openai/tiktoken/blob/main/tiktoken/model.py) documents OpenAI mappings; the Rust implementation uses [`tiktoken-rs`](https://docs.rs/tiktoken-rs/latest/tiktoken_rs/). The ordinary text vocabulary sizes are 100,256 for `cl100k_base` and 199,998 for `o200k_base`, as defined by [OpenAI's encoding data](https://github.com/openai/tiktoken/blob/main/tiktoken_ext/openai_public.py). OpenAI's [token counting guide](https://developers.openai.com/api/docs/guides/token-counting) explains why plain text tokenization differs from full request usage.

Anthropic's [archived public tokenizer](https://github.com/anthropics/anthropic-tokenizer-typescript) explicitly says it is inaccurate from Claude 3 onward. `tokedex` embeds its [Claude vocabulary](https://github.com/anthropics/anthropic-tokenizer-typescript/blob/main/claude.json), applies the package's NFKC normalization, and computes approximate Claude statistics in Rust. The embedded file is covered by [Anthropic's license](assets/CLAUDE_TOKENIZER_LICENSE). Its `explicit_n_vocab` metadata says 64,739, but the actual table has 64,995 ordinary IDs plus 5 special IDs. The denominator is therefore **65,000 loaded IDs**. Anthropic's [token counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) returns a count, not token IDs or a vocabulary; its documentation also says Claude 4.7 and later use a newer tokenizer. The Claude percentages and token identities shown by `tokedex` are those of the *old* tokenizer, not the current Claude models.

## History formats

- Codex: `response_item` messages, attributed by `turn_context.model`.
- Claude Code: user and assistant records under `projects`; duplicate assistant snapshots with the same message ID are counted once.
- pi: `message` and `model_change` records under `agent/sessions`.

A user message is attributed to the next assistant model in its session. Messages without a subsequent known model appear in `unattributed_messages`. Non-text content is ignored. Session formats can evolve, so unknown models and records are skipped instead of guessed.
