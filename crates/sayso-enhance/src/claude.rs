//! Claude Code CLI provider: `claude -p` with a JSON schema.
//!
//! Measured on this Mac (research §9): 1.6 to 1.9 s for one sentence with
//! `--effort low`. The thinking level of the provider sets `--effort`. The
//! tool reports the levels of each model with its model list (see `models`). With
//! Haiku 5.5 (6 runs of two sentences for each level, tool 2.1.296) the model
//! thought in 0 runs at `low`, 2 at `medium`, and 5 at `high`, and a run with
//! thinking wrote 200 to 450 tokens more and took about 0.5 to 1 s longer.
//! `MAX_THINKING_TOKENS=0` changed nothing there: `medium` thought in 4 of 6
//! runs with it. Sayso sets it at `low` only, for a model that still reads
//! it. No setting stops all thinking: `low` thought in about 1 run of 10 with
//! other prompts. Without `--strict-mcp-config`
//! the CLI loads the user's MCP servers and plugins: one run read 160K tokens
//! of tool definitions, took three turns and 5.7 s, and answered in prose. The CLI uses the user's own
//! login. Sayso never reads Claude credentials, and it never passes any.
//!
//! The transcript goes in through standard input. `claude -p` reads its prompt
//! from stdin when no prompt argument exists. This also keeps the text out of
//! the process list, and `--tools` takes a list, so a trailing prompt argument
//! could be read as a tool name.

use crate::auth::{Tool, config_env, push_config_env};
use crate::cli::{self, Invocation, looks_like_login_problem, tail};
use crate::detect::detect_claude_cli;
use sayso_core::config::DEFAULT_CLI_THINKING;
use sayso_core::enhance::{
    EnhanceError, EnhanceRequest, EnhanceResponse, Enhancer, ProviderReport, output_schema, parse_value, user_message,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct ClaudeCli {
    id: String,
    /// None when no binary was found. `enhance` then fails with `NotConfigured`.
    program: Option<PathBuf>,
    model: String,
    default_timeout: Duration,
    config_dir: Option<PathBuf>,
    /// A name from the `supportedEffortLevels` of the model.
    thinking: String,
}

impl ClaudeCli {
    /// `path` is the binary from the provider config. None searches for it.
    pub fn new(id: impl Into<String>, path: Option<PathBuf>, model: impl Into<String>) -> Self {
        let program = path.or_else(detect_claude_cli);
        Self {
            id: id.into(),
            program,
            model: model.into(),
            default_timeout: Duration::from_secs(8),
            config_dir: None,
            thinking: DEFAULT_CLI_THINKING.into(),
        }
    }

    /// The timeout used when a request has a zero timeout.
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// The folder with the login of the tool (`CLAUDE_CONFIG_DIR`). A leading `~/` is the home folder.
    pub fn with_config_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.config_dir = dir;
        self
    }

    /// The thinking level. None keeps the default, `low`.
    pub fn with_thinking(mut self, level: Option<&str>) -> Self {
        self.thinking = level.unwrap_or(DEFAULT_CLI_THINKING).into();
        self
    }

    /// The command line arguments, without the transcript.
    fn args(&self, model: &str, system_prompt: &str) -> Vec<std::ffi::OsString> {
        let schema = output_schema().to_string();
        [
            "-p",
            "--model",
            model,
            "--effort",
            &self.thinking,
            "--output-format",
            "json",
            "--json-schema",
            &schema,
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
            "--disable-slash-commands",
            "--setting-sources",
            "",
            "--system-prompt",
            system_prompt,
        ]
        .iter()
        .map(Into::into)
        .collect()
    }
}

impl Enhancer for ClaudeCli {
    fn id(&self) -> &str {
        &self.id
    }

    /// The CLI sends text to Anthropic.
    fn is_cloud(&self) -> bool {
        true
    }

    fn enhance(&self, request: &EnhanceRequest) -> Result<EnhanceResponse, EnhanceError> {
        let program = self
            .program
            .as_deref()
            .ok_or_else(|| EnhanceError::NotConfigured("the claude command was not found".into()))?;
        let timeout = if request.timeout.is_zero() { self.default_timeout } else { request.timeout };
        let model = request.model.clone().unwrap_or_else(|| self.model.clone());
        let workdir = cli::workdir()?;
        let started = Instant::now();
        let dir_env = config_env(Tool::Claude, self.config_dir.as_deref().and_then(|d| d.to_str()));
        // A limit of 0 at a higher level would work against the choice of the user.
        let mut envs = Vec::new();
        if self.thinking == DEFAULT_CLI_THINKING {
            envs.push(("MAX_THINKING_TOKENS", "0"));
        }
        push_config_env(&mut envs, &dir_env);
        let output = cli::run(&Invocation {
            program,
            args: self.args(&model, &request.system_prompt),
            envs,
            cwd: workdir.path(),
            stdin: Some(user_message(&request.transcript)),
            timeout,
        })?;
        let (text, report) = parse_envelope(&output.stdout).map_err(|e| match e {
            // No usable envelope: the tool failed before it could write one.
            EnhanceError::InvalidOutput(_) if !output.status.success() => {
                EnhanceError::Cli(format!("claude exited with {}: {}", output.status, tail(&output.stderr)))
            }
            other => other,
        })?;
        Ok(EnhanceResponse {
            text,
            provider_id: self.id.clone(),
            model,
            elapsed_ms: started.elapsed().as_millis() as u64,
            mode: "claude_cli".into(),
            report: Some(report),
        })
    }
}

/// Read the `--output-format json` result and return the validated text, with
/// the numbers of the run.
///
/// Fails on `is_error`, on a `subtype` other than `success`, and on a missing
/// or invalid `structured_output`.
pub(crate) fn parse_envelope(stdout: &str) -> Result<(String, ProviderReport), EnhanceError> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| EnhanceError::InvalidOutput(format!("claude output is not JSON: {e}")))?;
    // Some versions print the whole message list. The result is the last "result" item.
    let envelope = match &value {
        serde_json::Value::Array(items) => items.iter().rev().find(|i| i["type"] == "result"),
        other => Some(other),
    }
    .ok_or_else(|| EnhanceError::InvalidOutput("claude output has no result message".into()))?;

    let message = envelope["result"].as_str().unwrap_or_default();
    if envelope["is_error"].as_bool().unwrap_or(false) {
        return Err(if looks_like_login_problem(message) {
            EnhanceError::NotConfigured(format!("claude is not signed in: {message}"))
        } else {
            EnhanceError::Cli(format!("claude reported an error: {message}"))
        });
    }
    if let Some(subtype) = envelope["subtype"].as_str().filter(|s| *s != "success") {
        return Err(EnhanceError::Cli(format!("claude ended with \"{subtype}\": {message}")));
    }
    let structured = envelope
        .get("structured_output")
        .filter(|v| !v.is_null())
        .ok_or_else(|| EnhanceError::InvalidOutput("claude returned no structured_output".into()))?;
    Ok((parse_value(structured)?, report(envelope)))
}

/// The numbers of the run in the result. A version of the tool that leaves
/// one out gives None for it.
fn report(envelope: &serde_json::Value) -> ProviderReport {
    let usage = &envelope["usage"];
    ProviderReport {
        run_ms: envelope["duration_ms"].as_u64(),
        api_ms: envelope["duration_api_ms"].as_u64(),
        turns: envelope["num_turns"].as_u64(),
        input_tokens: usage["input_tokens"].as_u64(),
        output_tokens: usage["output_tokens"].as_u64(),
        thinking_tokens: usage["output_tokens_details"]["thinking_tokens"].as_u64(),
        cache_read_tokens: usage["cache_read_input_tokens"].as_u64(),
        cache_write_tokens: usage["cache_creation_input_tokens"].as_u64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_structured_output() {
        let out = r#"{"type":"result","subtype":"success","is_error":false,"result":"","structured_output":{"text":"Hello."}}"#;
        assert_eq!(parse_envelope(out).unwrap(), ("Hello.".to_string(), ProviderReport::default()));
    }

    #[test]
    fn reads_the_numbers_of_the_run() {
        let out = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":5210,"duration_api_ms":4870,"num_turns":3,
            "usage":{"input_tokens":12,"cache_creation_input_tokens":160000,"cache_read_input_tokens":0,"output_tokens":41,"output_tokens_details":{"thinking_tokens":30}},
            "structured_output":{"text":"Hello."}}"#;
        let (_, report) = parse_envelope(out).unwrap();
        assert_eq!((report.run_ms, report.api_ms, report.turns), (Some(5210), Some(4870), Some(3)));
        assert_eq!((report.input_tokens, report.output_tokens, report.thinking_tokens), (Some(12), Some(41), Some(30)));
        assert_eq!((report.cache_read_tokens, report.cache_write_tokens), (Some(0), Some(160_000)));
    }

    #[test]
    fn reads_the_result_item_of_a_message_list() {
        let out = r#"[{"type":"system"},{"type":"result","subtype":"success","structured_output":{"text":"Hi"}}]"#;
        assert_eq!(parse_envelope(out).unwrap().0, "Hi");
    }

    #[test]
    fn errors_map_to_the_right_variants() {
        let is_error = r#"{"type":"result","subtype":"success","is_error":true,"result":"boom"}"#;
        assert!(matches!(parse_envelope(is_error), Err(EnhanceError::Cli(m)) if m.contains("boom")));
        let login = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in. Run /login"}"#;
        assert!(matches!(parse_envelope(login), Err(EnhanceError::NotConfigured(_))));
        let subtype = r#"{"type":"result","subtype":"error_max_turns","is_error":false}"#;
        assert!(matches!(parse_envelope(subtype), Err(EnhanceError::Cli(m)) if m.contains("error_max_turns")));
        let missing = r#"{"type":"result","subtype":"success","is_error":false,"result":"plain text"}"#;
        assert!(matches!(parse_envelope(missing), Err(EnhanceError::InvalidOutput(_))));
        let wrong = r#"{"type":"result","subtype":"success","structured_output":{"txt":"x"}}"#;
        assert!(matches!(parse_envelope(wrong), Err(EnhanceError::InvalidOutput(_))));
        assert!(matches!(parse_envelope("not json"), Err(EnhanceError::InvalidOutput(_))));
    }

    #[test]
    fn command_line_has_the_hygiene_flags() {
        let cli = ClaudeCli::new("c", Some("/bin/claude".into()), "haiku");
        let args: Vec<String> = cli.args("haiku", "SYSTEM").iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let has_pair = |a: &str, b: &str| args.windows(2).any(|w| w[0] == a && w[1] == b);
        assert!(has_pair("--model", "haiku"));
        assert!(has_pair("--effort", "low"));
        let high = cli.with_thinking(Some("high")).args("haiku", "SYSTEM");
        assert!(high.windows(2).any(|w| w[0] == "--effort" && w[1] == "high"));
        assert!(has_pair("--output-format", "json"));
        assert!(has_pair("--tools", ""));
        assert!(has_pair("--setting-sources", ""));
        assert!(has_pair("--system-prompt", "SYSTEM"));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()), "no MCP servers or plugins from the user's setup");
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert!(!args.contains(&"--bare".to_string()), "--bare blocks the user's login");
    }
}
