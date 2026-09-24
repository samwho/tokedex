# tokedex

`tokedex` scans local Codex, Claude Code, and pi JSONL conversations and measures how much of each tokenizer's vocabulary appears in the saved user and assistant text.

The default terminal view selects the model with the most counted text tokens and shows:

- Unique token IDs seen, divided by the tokenizer's vocabulary size (ordinary text IDs for OpenAI; all loaded IDs for the legacy Claude tokenizer).
- The largest token ID seen (the “higher ID token”), with its decoded text.
- The most frequent token ID, with its decoded text and count.

Token IDs are counted separately per model, even when models share an encoding. Messages are encoded independently. Only saved user and assistant text is included; hidden reasoning, image/audio payloads, tools, protocol tokens, and repeated prompt context are excluded. This is a vocabulary exploration tool, not a billing or total context counter. Output is local by default; the CLI makes no API requests.

The default output follows neofetch's layout: a provider ASCII logo on the left and labelled statistics on the right. It shows only the leading model. Colors appear when stdout is a terminal; use `--no-color` for plain text or `--json` for the complete per-model data.

```text
         #######             tokedex@OpenAI
       ##     ###  ###       ---------------
      ##  ###         ##     Model: gpt-6-astra
   ## ##  #   ### ###  #     Messages: 42
  ##  ##  #######     ##     Text tokens: 8,000
  ##  ##  #      ####   #    Unique: 1,560 / 199,998
   #   ####      #  ##   #   Coverage: 0.78% of vocabulary
    ##     #######  ##  ##   Highest ID: #199000 "example"
    #  ### ###   #  ## ##   Most common: #123 " the" x340
    ##         ###  ##      Files scanned: 41
      ###  ###     ##       ASSUMED: GPT-5.6 tokenizer
            #######         ████████████████
            OPENAI
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

Use `--json` for structured output containing every model. GPT-6 uses the GPT-5.6 `o200k_base` tokenizer by default as an **unverified assumption**, marked in both output formats. The published `tiktoken` model table does not yet verify this mapping.

## Tokenizer coverage

| Model family | Encoding | Status |
| --- | --- | --- |
| GPT-5 and 5.x, GPT-4o, GPT-4.1, GPT-4.5, o1/o3/o4-mini, codex-mini | `o200k_base` | Text token IDs available in Rust |
| GPT-4, GPT-4 Turbo, GPT-3.5 | `cl100k_base` | Text token IDs available in Rust |
| GPT-6 | Assumed `o200k_base`, like GPT-5.6 | Included by default; visibly marked as assumed |
| Claude 3 through current Claude | Anthropic's archived Claude tokenizer | Approximate only; prominent warning in terminal and JSON output |

The [OpenAI `tiktoken` model table](https://github.com/openai/tiktoken/blob/main/tiktoken/model.py) documents OpenAI mappings; the Rust implementation uses [`tiktoken-rs`](https://docs.rs/tiktoken-rs/latest/tiktoken_rs/). The ordinary text vocabulary sizes are 100,256 for `cl100k_base` and 199,998 for `o200k_base`, as defined by [OpenAI's encoding data](https://github.com/openai/tiktoken/blob/main/tiktoken_ext/openai_public.py). OpenAI's [token counting guide](https://developers.openai.com/api/docs/guides/token-counting) explains why plain text tokenization differs from full request usage.

Anthropic's [archived public tokenizer](https://github.com/anthropics/anthropic-tokenizer-typescript) explicitly says it is inaccurate from Claude 3 onward. `tokedex` embeds its [Claude vocabulary](https://github.com/anthropics/anthropic-tokenizer-typescript/blob/main/claude.json), applies the package's NFKC normalization, and computes approximate Claude statistics in Rust. The embedded file is covered by [Anthropic's license](assets/CLAUDE_TOKENIZER_LICENSE). Its `explicit_n_vocab` metadata says 64,739, but the actual table has 64,995 ordinary IDs plus 5 special IDs. The denominator is therefore **65,000 loaded IDs**. Anthropic's [token counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) returns a count, not token IDs or a vocabulary; its documentation also says Claude 4.7 and later use a newer tokenizer. The Claude percentages and token identities shown by `tokedex` are those of the *old* tokenizer, not the current Claude models.

## History formats

- Codex: `response_item` messages, attributed by `turn_context.model`.
- Claude Code: user and assistant records under `projects`; duplicate assistant snapshots with the same message ID are counted once.
- pi: `message` and `model_change` records under `agent/sessions`.

A user message is attributed to the next assistant model in its session. Messages without a subsequent known model appear in `unattributed_messages`. Non-text content is ignored. Session formats can evolve, so unknown models and records are skipped instead of guessed.
