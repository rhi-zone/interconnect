/// Generate a `.claude/settings.json` file that wires Claude Code hooks to
/// the `interconnect` CLI.
///
/// Hook behaviour:
/// - `PostToolUse`: after each tool call Claude makes, pipe the event JSON
///   from stdin to `interconnect send $INTERCONNECT_REPLY_TO`.
/// - `Stop`: when Claude stops (final turn), send a stop notification to
///   the reply-to room.
///
/// The `INTERCONNECT_REPLY_TO` environment variable must be set by the outer
/// process (e.g. `interconnect recv` or a wrapper script) to name the room
/// that should receive Claude's output.
use crate::config::Config;

pub struct ClaudePreset {
    pub rooms: Vec<String>,
}

impl ClaudePreset {
    pub fn from_config(config: &Config) -> Self {
        Self {
            rooms: config.room.iter().map(|r| r.name.clone()).collect(),
        }
    }

    /// Render the `.claude/settings.json` content.
    pub fn render(&self) -> serde_json::Value {
        // PostToolUse: read the event JSON from stdin and forward it to the
        // configured reply-to room. Uses a POSIX-compatible shell pipeline;
        // errors are suppressed so a missing daemon doesn't break Claude's
        // normal operation.
        let post_tool_use_cmd =
            r#"interconnect send "$INTERCONNECT_REPLY_TO" "$(cat)" 2>/dev/null || true"#;

        let stop_cmd =
            r#"interconnect send "$INTERCONNECT_REPLY_TO" '{"type":"stop"}' 2>/dev/null || true"#;

        // PreToolUse / UserPromptSubmit: inject any pending messages from the
        // reply-to room into Claude's context. Hook stdout is shown to Claude
        // before the tool call or prompt is processed. If there are no pending
        // messages the command prints nothing and Claude proceeds normally.
        let inject_cmd =
            r#"interconnect recv --nowait "$INTERCONNECT_REPLY_TO" 2>/dev/null || true"#;

        serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": inject_cmd
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": post_tool_use_cmd
                            }
                        ]
                    }
                ],
                "UserPromptSubmit": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": inject_cmd
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [
                            {
                                "type": "command",
                                "command": stop_cmd
                            }
                        ]
                    }
                ]
            },
            "_interconnect_rooms": self.rooms,
            "_interconnect_note": "Set INTERCONNECT_REPLY_TO=<room-name> before invoking claude to route events."
        })
    }
}
