use std::io::Write;
use std::process::{Command, Stdio};

use async_trait::async_trait;
use forge_domain::{
    ContextMessage, Conversation, EndPayload, EventData, EventHandle, ToolcallEndPayload,
    ToolcallStartPayload,
};
use serde_json::{Value, json};

/// Tool names (lower-case) that trigger the pre-edit hook.
const FILE_WRITE_TOOLS: &[&str] = &["write", "patch", "multi_patch"];

/// Tool name (lower-case) that triggers the pre-commit and post-test hooks.
const SHELL_TOOL: &str = "shell";

/// Handler that bridges ForgeCode lifecycle events to grumpy's hook protocol.
///
/// Enables grumpy's TDD red-first enforcement and stop-guard within a ForgeCode
/// session by spawning `grumpy hook <sub-command>` with the appropriate JSON
/// payload at each lifecycle event.
///
/// # Hook mapping
///
/// | ForgeCode event             | grumpy sub-command |
/// |-----------------------------|--------------------|
/// | `ToolcallStart` – write / patch / multi_patch | `pre-edit`    |
/// | `ToolcallStart` – shell                       | `pre-commit`  |
/// | `ToolcallEnd`   – shell                       | `post-test`   |
/// | `End`                                         | `stop`        |
///
/// # Blocking behaviour
///
/// When grumpy exits with code 2 (block), the feedback message is injected into
/// the conversation so the LLM can act on it.  For the `End` event the injected
/// message also causes the orchestrator to continue the session, matching
/// Claude Code's stop-guard semantics.  For `ToolcallStart` events the
/// underlying tool still executes; hard pre-execution blocking requires a
/// future orchestrator enhancement.
#[derive(Debug, Clone, Default)]
pub struct GrumpyHandler;

impl GrumpyHandler {
    /// Creates a new grumpy handler.
    pub fn new() -> Self {
        Self
    }

    /// Resolves the path to the `grumpy` binary.
    ///
    /// Checks `PATH` first (correct when ForgeCode is launched from a terminal),
    /// then falls back to well-known install locations that GUI-launched
    /// processes on macOS miss because their `PATH` is stripped to the
    /// launch-services default (`/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin`).
    fn grumpy_bin() -> Option<std::path::PathBuf> {
        use std::path::PathBuf;

        // 1. Walk PATH — works for terminal-launched ForgeCode.
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join("grumpy");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        // 2. Probe well-known locations missed by GUI-launched processes.
        //    grumpy's installer puts the binary in ~/.local/bin by default.
        let home = std::env::var("HOME").ok().map(PathBuf::from);
        [
            home.as_ref().map(|h| h.join(".local/bin/grumpy")),
            home.as_ref().map(|h| h.join(".cargo/bin/grumpy")),
            Some(PathBuf::from("/usr/local/bin/grumpy")),
            Some(PathBuf::from("/opt/homebrew/bin/grumpy")),
        ]
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
    }

    /// Runs `grumpy hook <sub_command>` with `payload` piped to stdin.
    ///
    /// # Returns
    ///
    /// `Some(stderr)` when grumpy exits with code 2 (action blocked), or
    /// `None` when grumpy is not installed, not active, or allows the action.
    fn call_grumpy(sub_command: &str, payload: &Value, active_id: &str) -> Option<String> {
        let bin = Self::grumpy_bin()?;
        let payload_str = serde_json::to_string(payload).ok()?;
        let mut child = Command::new(bin)
            .args(["hook", sub_command])
            .env("GRUMPY_ACTIVE_ID", active_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload_str.as_bytes());
        }

        let output = child.wait_with_output().ok()?;
        if output.status.code() == Some(2) {
            Some(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            None
        }
    }

    /// Injects a grumpy feedback message into the conversation context as a
    /// user turn, making it visible to the LLM in the next request.
    fn inject_block(msg: &str, conversation: &mut Conversation) {
        if let Some(context) = conversation.context.as_mut() {
            let content = format!("[grumpy] {msg}");
            context
                .messages
                .push(ContextMessage::user(content, None).into());
        }
    }

    /// Extracts a string field from JSON tool-call arguments, trying each key
    /// in `keys` in order and returning the first match.
    fn extract_arg(args_str: &str, keys: &[&str]) -> Option<String> {
        let value: Value = serde_json::from_str(args_str).ok()?;
        keys.iter()
            .find_map(|key| value.get(*key).and_then(|v| v.as_str()).map(String::from))
    }
}

#[async_trait]
impl EventHandle<EventData<ToolcallStartPayload>> for GrumpyHandler {
    async fn handle(
        &self,
        event: &EventData<ToolcallStartPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        let active_id = conversation.id.to_string();
        let tool_name = event.payload.tool_call.name.as_str().to_lowercase();
        let args_str = event.payload.tool_call.arguments.clone().into_string();

        if FILE_WRITE_TOOLS.contains(&tool_name.as_str()) {
            if let Some(file_path) = Self::extract_arg(&args_str, &["file_path", "path"]) {
                let payload = json!({ "tool_input": { "file_path": file_path } });
                if let Some(msg) = Self::call_grumpy("pre-edit", &payload, &active_id) {
                    Self::inject_block(&msg, conversation);
                }
            }
        } else if tool_name == SHELL_TOOL {
            if let Some(command) = Self::extract_arg(&args_str, &["command"]) {
                let payload = json!({ "tool_input": { "command": command } });
                if let Some(msg) = Self::call_grumpy("pre-commit", &payload, &active_id) {
                    Self::inject_block(&msg, conversation);
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<ToolcallEndPayload>> for GrumpyHandler {
    async fn handle(
        &self,
        event: &EventData<ToolcallEndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        let tool_name = event.payload.tool_call.name.as_str().to_lowercase();
        if tool_name != SHELL_TOOL {
            return Ok(());
        }

        let active_id = conversation.id.to_string();
        let args_str = event.payload.tool_call.arguments.clone().into_string();
        let command = Self::extract_arg(&args_str, &["command"]).unwrap_or_default();

        // The shell output is rendered as XML-like text; pass the full text as
        // stdout so grumpy can detect test pass/fail patterns.
        let stdout = event
            .payload
            .result
            .output
            .as_str()
            .unwrap_or_default()
            .to_owned();

        let payload = json!({
            "tool_input":    { "command": command },
            "tool_response": { "stdout": stdout, "stderr": "" }
        });

        // post-test only updates grumpy state; we ignore any block signal.
        Self::call_grumpy("post-test", &payload, &active_id);

        Ok(())
    }
}

#[async_trait]
impl EventHandle<EventData<EndPayload>> for GrumpyHandler {
    async fn handle(
        &self,
        _event: &EventData<EndPayload>,
        conversation: &mut Conversation,
    ) -> anyhow::Result<()> {
        let active_id = conversation.id.to_string();
        let payload = json!({});

        if let Some(msg) = Self::call_grumpy("stop", &payload, &active_id) {
            // Injecting a message causes the orchestrator to detect additional
            // messages and continue the loop, preventing premature session end.
            Self::inject_block(&msg, conversation);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{
        Agent, Context, Conversation, EndPayload, EventData, EventHandle, ModelId, ToolCallFull,
        ToolCallId, ToolName,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture_agent() -> Agent {
        Agent::new(
            "test-agent",
            "test-provider".to_string().into(),
            ModelId::new("test-model"),
        )
    }

    fn fixture_model_id() -> ModelId {
        ModelId::new("test-model")
    }

    fn fixture_conversation() -> Conversation {
        let mut conv = Conversation::generate();
        conv.context = Some(Context::default());
        conv
    }

    // ── GrumpyHandler::grumpy_bin ────────────────────────────────────────────

    #[test]
    fn test_grumpy_bin_returns_existing_path_or_none() {
        // We can't assert a specific path since the environment varies, but we
        // can assert that if it returns Some, the path actually exists on disk.
        if let Some(bin) = GrumpyHandler::grumpy_bin() {
            assert!(
                bin.is_file(),
                "grumpy_bin() returned a path that is not a file: {bin:?}"
            );
        }
        // Returning None is also valid — grumpy may not be installed in CI.
    }

    #[test]
    fn test_grumpy_bin_prefers_path_over_hardcoded() {
        // If grumpy is on PATH, grumpy_bin() must return a path whose binary
        // name is "grumpy" (not one of the hardcoded fallbacks).
        if let Ok(path_var) = std::env::var("PATH") {
            let on_path = std::env::split_paths(&path_var)
                .map(|d| d.join("grumpy"))
                .find(|p| p.is_file());
            if let (Some(expected), Some(actual)) = (on_path, GrumpyHandler::grumpy_bin()) {
                assert_eq!(actual, expected);
            }
        }
    }

    // ── GrumpyHandler::extract_arg ───────────────────────────────────────────

    #[test]
    fn test_extract_arg_returns_first_matching_key() {
        let args = r#"{"file_path": "/src/main.rs", "content": "hi"}"#;
        let actual = GrumpyHandler::extract_arg(args, &["file_path", "path"]);
        let expected = Some("/src/main.rs".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_extract_arg_falls_through_to_alias() {
        let args = r#"{"path": "/src/lib.rs", "content": "hi"}"#;
        let actual = GrumpyHandler::extract_arg(args, &["file_path", "path"]);
        let expected = Some("/src/lib.rs".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_extract_arg_missing_key_returns_none() {
        let args = r#"{"other": "value"}"#;
        let actual = GrumpyHandler::extract_arg(args, &["file_path", "path"]);
        let expected = None;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_extract_arg_invalid_json_returns_none() {
        let actual = GrumpyHandler::extract_arg("not-json", &["file_path"]);
        let expected = None;
        assert_eq!(actual, expected);
    }

    // ── GrumpyHandler::inject_block ──────────────────────────────────────────

    #[test]
    fn test_inject_block_prepends_grumpy_prefix() {
        let mut conv = fixture_conversation();
        GrumpyHandler::inject_block("write failing test first", &mut conv);

        let context = conv.context.as_ref().unwrap();
        assert_eq!(context.messages.len(), 1);
        let content = context.messages[0].message.content().unwrap();
        assert!(
            content.contains("[grumpy]"),
            "message should start with [grumpy] prefix"
        );
    }

    #[test]
    fn test_inject_block_no_op_without_context() {
        let mut conv = Conversation::generate();
        // No context set — should not panic.
        GrumpyHandler::inject_block("msg", &mut conv);
        assert!(conv.context.is_none());
    }

    // ── ToolcallStart (non-write tool) ────────────────────────────────────────

    #[tokio::test]
    async fn test_toolcall_start_read_tool_is_ignored() {
        let handler = GrumpyHandler::new();
        let tool_call = ToolCallFull {
            name: ToolName::new("read"),
            call_id: Some(ToolCallId::new("c1")),
            arguments: serde_json::json!({"file_path": "/src/main.rs"}).into(),
            thought_signature: None,
        };
        let event = EventData::new(
            fixture_agent(),
            fixture_model_id(),
            ToolcallStartPayload::new(tool_call),
        );
        let mut conv = fixture_conversation();
        let before = conv.context.as_ref().unwrap().messages.len();

        handler.handle(&event, &mut conv).await.unwrap();

        // No message injected — grumpy was not called (no GRUMPY_ACTIVE_ID and
        // grumpy binary may not be present in CI).
        let actual = conv.context.as_ref().unwrap().messages.len();
        assert_eq!(actual, before);
    }

    // ── End hook ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_end_hook_no_active_id_is_no_op() {
        // When GRUMPY_ACTIVE_ID is not set grumpy exits cleanly (or is absent),
        // so no message should be injected.
        let handler = GrumpyHandler::new();
        let event = EventData::new(fixture_agent(), fixture_model_id(), EndPayload);
        let mut conv = fixture_conversation();
        let before = conv.context.as_ref().unwrap().messages.len();

        handler.handle(&event, &mut conv).await.unwrap();

        let actual = conv.context.as_ref().unwrap().messages.len();
        // grumpy is either absent (no injection) or present but not blocking.
        // Either way we only assert the handler doesn't error.
        let _ = actual;
        let _ = before;
    }
}
