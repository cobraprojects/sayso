# sayso-enhance

`Enhancer` implementations for `sayso_core::enhance`. All calls block.

## API

- `build_enhancer(&Provider, &dyn SecretStore, &Ai, Option<Arc<dyn LanguageModel>>) -> Box<dyn Enhancer>`. Never fails. A provider that cannot work (missing API key, missing binary) returns `NotConfigured` from `enhance`. `Ai` timeouts are the default for requests with a zero timeout.
- `SecretStore` with `KeychainStore` (service `dev.sayso.Sayso`) and `MemoryStore`.
- `OpenAiCompatible`: `POST {base_url}/chat/completions`. Mode ladder `json_schema`, `json_object`, `tool_call`. Next mode on HTTP 400/422 or a reply that does not match the schema. Working mode cached per `(base_url, model)` for the process (`clear_mode_cache()`). One whole-call deadline. OpenRouter extras: `provider.require_parameters` (json_schema and tool_call modes), `zdr` and `data_collection: "deny"` with `zero_data_retention`. Builder: `with_api_key`, `with_zero_data_retention`, `with_cloud`, `with_openrouter`, `with_default_timeout`.
- `ClaudeCli::new(id, path: Option<PathBuf>, model)`, `with_config_dir(Option<PathBuf>)`. Transcript on stdin, schema in `--json-schema`, `MAX_THINKING_TOKENS=0`, temp cwd, kill on timeout. Reads `structured_output`. Fails on `is_error` or a non-success `subtype`.
- `CodexCli::new(id, path, model)`, `with_config_dir(Option<PathBuf>)`. Prompt (instructions plus transcript) on stdin with `-`, schema and answer files in a temp dir, kill on timeout.
- `EngineModel::new(id, Option<Arc<dyn LanguageModel>>)`: the language model that the engine runs on this computer (Apple Intelligence on macOS, provider kind `apple_intelligence`). It sends the style prompt as instructions and the tagged transcript as the prompt, then applies the same checks to the text as the other providers. Not cloud. Without an engine it returns `NotConfigured`. This crate does not depend on the engine client: the app passes the handle in.
- Config folder: `config_dir` of a CLI provider sets `CLAUDE_CONFIG_DIR` or `CODEX_HOME` for every run of the tool (enhance, model list, status, sign-in). A leading `~/` is the home folder (`HOME`, `USERPROFILE` on Windows). Unset leaves the environment as it was. One helper in `auth.rs` does the env name and the expansion.
- `auth_status(&Provider, timeout) -> Option<AuthStatus>`: `claude auth status` (JSON on stdout) or `codex login status` (text, "not logged in" is checked first). None for kinds without a login. A configured folder that does not exist gives `SignedOut` without running the tool. Binary not found gives `NotInstalled`. A failed or unreadable status gives `Unknown`.
- `SignIn::start(&Provider)`: `claude auth login` (stdin piped, `takes_code()` true) or `codex login`. Creates the config folder. Both output pipes are read by helper threads. `next_event()` gives one `Url` (first http(s) token), then `Finished(AuthStatus)` once; the status is read again with `auth_status` (10 s). `send_code`, `cancel`. The limit is `SIGN_IN_LIMIT` (10 min); `start_with_limit` (hidden) is for tests. `Send + Sync`. Drop kills and waits for the tool.
- Detection: `detect_claude_cli`, `detect_codex_cli`, `detect_ollama(timeout)`, `detect_ollama_at(url, timeout)`, `list_models(base_url, key)`, `test_provider(&dyn Enhancer)`.

## Tested (no network, no keys)

Mock `tiny_http` server: strict json_schema, fallback after 400 and 422, fallback when the reply ignores the format, forced tool call, mode cache, invalid JSON, wrong schema, timeout (also across the ladder and with the default timeout), OpenRouter extras, no extras for other hosts, Authorization header from the secret store, 401 not retried, refusal, unreachable server, `list_models`, `detect_ollama_at`, `test_provider`.
Fake `claude` and `codex` scripts: arguments, environment, stdin, temp cwd removed, success, error envelope, non-success subtype, crash with stderr, invalid output, timeout with child killed, missing binary, a helper that keeps the pipe open.
Fake scripts for login: `auth status` and `login status` parsing (signed in, signed out, unreadable), URL then code then `Finished(SignedIn)` with the folder created and `CLAUDE_CONFIG_DIR` seen, Codex with `CODEX_HOME` and no code, cancel, the time limit, drop, missing folder (tool not run), missing binary, `config_dir` in the environment of enhance runs. Config: an old file without `config_dir` loads, an unset value is not written.
Unit tests: request bodies, reply parsing, envelope parsing, executable search, factory.

## Not tested

The real `claude auth login` and `codex login` (they open a browser; only the status commands ran by hand), the Flatpak path of the config folder (no existence check, no folder creation), the Windows path, Real providers, the real `claude` and `codex` commands (the `claude -p` stdin input and the `codex exec -` stdin input follow the CLI help and docs, but no model call was made), `KeychainStore` (it would prompt for Keychain access), HTTPS and TLS, streaming.
