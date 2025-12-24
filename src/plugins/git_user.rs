//! Git user plugin - displays the current git user from git config.

use std::process::Command;
use std::time::Duration;

use ratatui::style::{Color, Style};

use crate::plugins::context::PluginContext;
use crate::plugins::{Plugin, PluginConfig, PluginResult};
use crate::status_bar::StatusBarSegment;

/// Cached git user information.
#[derive(Clone, Debug, Default)]
struct GitUserInfo {
    name: Option<String>,
    email: Option<String>,
}

/// Plugin that displays the current git user (name and email).
pub struct GitUserPlugin {
    info: GitUserInfo,
}

impl GitUserPlugin {
    /// Create a new `GitUserPlugin`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: GitUserInfo::default(),
        }
    }

    /// Execute git config command and get output.
    fn git_config(cwd: &std::path::Path, key: &str) -> Option<String> {
        Command::new("git")
            .args(["config", key])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            })
    }
}

impl Default for GitUserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for GitUserPlugin {
    fn name(&self) -> &'static str {
        "git-user"
    }

    fn config(&self) -> PluginConfig {
        PluginConfig {
            // Git user doesn't change often, refresh every 30 seconds
            refresh_interval: Duration::from_secs(30),
            // Show on the left side of the status bar
            priority: 10,
        }
    }

    fn refresh(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        // Execute git config commands synchronously
        // Note: These are fast local operations, so blocking is acceptable
        self.info.name = Self::git_config(&ctx.cwd, "user.name");
        self.info.email = Self::git_config(&ctx.cwd, "user.email");
        Ok(())
    }

    fn render(&self) -> StatusBarSegment {
        match (&self.info.name, &self.info.email) {
            (Some(name), Some(email)) => StatusBarSegment::new(format!("{name} <{email}>"))
                .icon("@")
                .style(Style::default().fg(Color::Cyan)),
            (Some(name), None) => StatusBarSegment::new(name.clone())
                .icon("@")
                .style(Style::default().fg(Color::Cyan)),
            (None, Some(email)) => StatusBarSegment::new(format!("<{email}>"))
                .icon("@")
                .style(Style::default().fg(Color::Cyan)),
            (None, None) => {
                // Not in a git repo or no user configured
                StatusBarSegment::new("No git user")
                    .icon("?")
                    .style(Style::default().fg(Color::DarkGray))
            }
        }
    }
}
