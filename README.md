# tokedex

`tokedex` scans local Codex, Claude Code, and pi JSONL conversations and measures how much of each model's **ordinary text-token vocabulary** appears in the saved user and assistant text.

For each model with a known tokenizer, it shows:

- Unique token IDs seen, divided by the tokenizer's ordinary text vocabulary size.
- The largest token ID seen (the “higher ID token”), with its decoded text.
- The most frequent token ID, with its decoded text and count.

Token IDs are counted separately per model, even when models share an encoding. Messages are encoded independently. Only saved user and assistant text is included; hidden reasoning, image/audio payloads, tools, protocol tokens, and repeated prompt context are excluded. This is a vocabulary exploration tool, not a billing or total context counter. Output is local by default; the CLI makes no API requests.

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
| Claude 3 through current Claude | Unpublished | Scanned and reported as unsupported for token-ID statistics |

The [OpenAI `tiktoken` model table](https://github.com/openai/tiktoken/blob/main/tiktoken/model.py) documents OpenAI mappings; the Rust implementation uses [`tiktoken-rs`](https://docs.rs/tiktoken-rs/latest/tiktoken_rs/). The ordinary text vocabulary sizes are 100,256 for `cl100k_base` and 199,998 for `o200k_base`, as defined by [OpenAI's encoding data](https://github.com/openai/tiktoken/blob/main/tiktoken_ext/openai_public.py). OpenAI's [token counting guide](https://developers.openai.com/api/docs/guides/token-counting) explains why plain text tokenization differs from full request usage.

Anthropic's [archived public tokenizer](https://github.com/anthropics/anthropic-tokenizer-typescript) explicitly says it is inaccurate from Claude 3 onward. Anthropic's [token counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) returns a count, not token IDs or a vocabulary; its documentation also says Claude 4.7 and later use a newer tokenizer. Using another model's IDs would make the requested unique percentage, highest ID, and most common token misleading, so `tokedex` leaves those values empty for Claude. It still discovers and lists Claude model/message counts.

## History formats

- Codex: `response_item` messages, attributed by `turn_context.model`.
- Claude Code: user and assistant records under `projects`; duplicate assistant snapshots with the same message ID are counted once.
- pi: `message` and `model_change` records under `agent/sessions`.

A user message is attributed to the next assistant model in its session. Messages without a subsequent known model appear in `unattributed_messages`. Non-text content is ignored. Session formats can evolve, so unknown models and records are skipped instead of guessed.
