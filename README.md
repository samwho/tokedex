# tokedex

`tokedex` scans local Codex, Claude Code, and pi JSONL conversations and measures how much of each tokenizer's vocabulary appears across all saved textual model input and output.

The terminal view aggregates all matching conversation text and shows:

- One vocabulary-coverage line for every selected tokenizer that observed at least one token.
- The most common model and the longest decoded token seen across all tokenizers.
- The highest observed token ID, its tokenizer, and its decoded text.
- The most frequently seen decoded token text across all tokenizers, merging identical text from different tokenizers.

Saved model input and output from Codex, Claude Code, and pi is combined by tokenizer. This includes user, assistant, and system content; saved reasoning; tool calls and results; and saved tool schemas. Binary image/audio data and unavailable encrypted content cannot be tokenized. Messages are encoded independently, so this remains a vocabulary exploration tool rather than a billing counter. Output is local by default; the CLI makes no API requests.

The default output follows neofetch's layout, using the logo of the provider whose tokenizer counted the most text. Use `--harness HARNESS` to restrict which histories are opened and `--provider PROVIDER` to restrict tokenizers by provider. Colors appear when stdout is a terminal; use `--no-color` for plain text or `--json` for structured data.

```text
        ⣠⣴⣾⠿⠿⣿⣶⣤⣀⣀⣀⣀⡀          tokedex
      ⢀⣾⡟⠉  ⢀⣠⣾⡿⠟⠛⠛⠻⢿⣷⣄        -------
    ⢀⣤⣾⣿ ⢀⣴⣾⠿⠛⠉ ⣀⣤⣀  ⠙⣿⣧       Total messages: 42
  ⢀⣴⡿⠛⣿⣿ ⢸⣿⡇⢀⣠⣴⣾⠿⠛⠻⣿⣦⣄⣸⣿       Text tokens: 8,000
  ⣾⡿⠁ ⣿⣿ ⢸⣿⣷⡿⠟⠻⢿⣶⣤⡀ ⠉⠻⢿⣿⣄      Most common model: gpt-5.6-sol
  ⣿⣇  ⣿⣿⡀⢸⣿⡇    ⢸⣿⡿⣷⣦⡄ ⠙⣿⣆     Longest seen token: "extraordinary"
  ⠹⣿⣄ ⠘⠻⢿⣾⣿⡇    ⢸⣿⡇⠈⣿⣿  ⢹⣿     Token with highest ID: o200k_base #199000 "example"
   ⠙⣿⣷⣦⣀ ⠈⠛⠿⣷⣦⣴⣾⢿⣿⡇ ⣿⣿ ⢀⣾⡿     Most seen token: " the" x340
    ⣿⡏⠙⠻⣿⣦⣤⣶⡿⠟⠋⠁⢸⣿⡇ ⣿⣿⣤⣾⠟⠁       Files scanned: 41
    ⢻⣷⣄  ⠉⠛⠉ ⣀⣤⣶⡿⠟⠁ ⣿⡿⠛⠁
     ⠙⢿⣷⣦⣤⣤⣴⣾⡿⠋⠁  ⣀⣼⡿⠁       Tokenizers
       ⠈⠉⠉⠉⠉⠛⠿⣿⣶⣶⡿⠟⠋         ----------
                               o200k_base: 0.78% (1,560 / 199,998)
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

Use `--json` for structured output. Harness and provider filters may be repeated; unselected harness directories are not traversed, and messages that match no selected tokenizer are discarded before tokenization:

```sh
tokedex --harness pi --provider openai
tokedex --harness codex --harness pi --provider anthropic
tokedex --provider openai --json
```

Conversation files are parsed and tokenized in parallel with Rayon. On an interactive terminal, discovery and tokenizer preparation display activity spinners; scanning files, tokenizing files, and decoding observed token IDs each have a measured progress bar. Progress is hidden for JSON and redirected output.

## Tokenizer coverage

| Model family | Encoding | Status |
| --- | --- | --- |
| GPT-5 and 5.x, GPT-4o, GPT-4.1, GPT-4.5, o1/o3/o4-mini, codex-mini | `o200k_base` | Text token IDs available in Rust |
| GPT-4, GPT-4 Turbo, GPT-3.5 | `cl100k_base` | Text token IDs available in Rust |
| GPT-6 | `o200k_base` | Included by default |
| Claude 3 through current Claude | Anthropic's archived Claude tokenizer | Approximate token IDs and coverage |

The [OpenAI `tiktoken` model table](https://github.com/openai/tiktoken/blob/main/tiktoken/model.py) documents OpenAI mappings; the Rust implementation uses [`tiktoken-rs`](https://docs.rs/tiktoken-rs/latest/tiktoken_rs/). The ordinary text vocabulary sizes are 100,256 for `cl100k_base` and 199,998 for `o200k_base`, as defined by [OpenAI's encoding data](https://github.com/openai/tiktoken/blob/main/tiktoken_ext/openai_public.py). OpenAI's [token counting guide](https://developers.openai.com/api/docs/guides/token-counting) explains why plain text tokenization differs from full request usage.

Anthropic's [archived public tokenizer](https://github.com/anthropics/anthropic-tokenizer-typescript) explicitly says it is inaccurate from Claude 3 onward. `tokedex` embeds its [Claude vocabulary](https://github.com/anthropics/anthropic-tokenizer-typescript/blob/main/claude.json), applies the package's NFKC normalization, and computes approximate Claude statistics in Rust. The embedded file is covered by [Anthropic's license](assets/CLAUDE_TOKENIZER_LICENSE). Its `explicit_n_vocab` metadata says 64,739, but the actual table has 64,995 ordinary IDs plus 5 special IDs. The denominator is therefore **65,000 loaded IDs**. Anthropic's [token counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) returns a count, not token IDs or a vocabulary; its documentation also says Claude 4.7 and later use a newer tokenizer. The Claude percentages and token identities shown by `tokedex` are those of the *old* tokenizer, not the current Claude models.

## Extension architecture

Shared harness types live in `src/harness/mod.rs`, with Codex, Claude Code, and pi implementations in separate sibling modules. A harness owns history discovery and parsing into generic messages containing a harness name, model, body, and timestamp. Adding another agent requires implementing `Harness` and registering it in `harnesses()`.

The `Provider` trait and global provider list live in `src/providers/mod.rs`; providers contain only their names and terminal logos. Tokenizer integrations live under `src/tokenizers/`. Each `Tokenizer` references its provider, determines whether it is eligible for a message's model name, and owns its vocabulary metadata, encoding, tokenization, and decoding behavior. Shared tokenizer instances are cached by encoding.

## History formats

- Codex: `response_item` messages, attributed by `turn_context.model`.
- Claude Code: user and assistant records under `projects`; duplicate assistant snapshots with the same message ID are counted once.
- pi: `message` and `model_change` records under `agent/sessions`.

A user message is attributed to the next assistant model in its session. Messages without a subsequent known model, and messages that match no selected tokenizer, are discarded. Non-text content is ignored. Session formats can evolve, so unknown models and records are skipped instead of guessed.
