use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use teams_cli::graph::{list_chats, list_messages, print_chats, print_messages, sign_in};
use teams_cli::{
    SkillTarget, discover_skills, install_bundled_skill, open_teams, print_install_results,
    print_skill, print_skills,
};

const DEFAULT_TEAMS_URL: &str = "https://teams.microsoft.com/";

#[derive(Debug, Parser)]
#[command(
    name = "teams",
    version,
    about = "Agent-friendly Microsoft Teams web CLI"
)]
struct Cli {
    /// Add a directory to search for agent skills (may be repeated).
    #[arg(long = "skills-dir", global = true, value_name = "DIR")]
    skills_dirs: Vec<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover and inspect SKILL.md files for the current agent.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    /// Open Microsoft Teams in the system browser or a selected browser executable.
    Web {
        /// Teams URL to open.
        #[arg(long, default_value = DEFAULT_TEAMS_URL)]
        url: String,

        /// Browser executable to launch; defaults to TEAMS_CLI_BROWSER or the system opener.
        #[arg(long, env = "TEAMS_CLI_BROWSER")]
        browser: Option<PathBuf>,
    },
    /// Read-only Teams chat access via Microsoft Graph. Requires a fresh device-code
    /// sign-in each run (Chat.Read scope); no token is ever written to disk.
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ChatCommand {
    /// List your chats (id, display name, chat type).
    List,
    /// List the most recent messages in one chat.
    Messages {
        /// Chat id, as shown by `teams chat list`.
        chat_id: String,

        /// Maximum number of messages to fetch (Graph's default order is newest first).
        #[arg(long, default_value_t = 20)]
        top: u32,
    },
}

#[derive(Debug, Subcommand)]
enum SkillsCommand {
    /// List discovered skills and their descriptions.
    List,
    /// Print a skill's complete SKILL.md instructions.
    Show {
        /// Skill name or directory name.
        name: String,
    },
    /// Install this CLI's own skill into agent skill directories so it is auto-discovered.
    Install {
        /// Install into this target only (claude, opencode, agents); repeatable. Defaults to all.
        #[arg(long = "target", value_name = "TARGET")]
        targets: Vec<SkillTarget>,

        /// Overwrite an existing installed copy.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Skills { command } => {
            let skills = discover_skills(&cli.skills_dirs)?;
            match command {
                SkillsCommand::List => print_skills(&skills),
                SkillsCommand::Show { name } => print_skill(&skills, &name)?,
                SkillsCommand::Install { targets, force } => {
                    let results = install_bundled_skill(&targets, force)?;
                    print_install_results(&results);
                }
            }
        }
        Command::Web { url, browser } => open_teams(&url, browser.as_deref())?,
        Command::Chat { command } => {
            let token = sign_in()?;
            match command {
                ChatCommand::List => print_chats(&list_chats(&token)?),
                ChatCommand::Messages { chat_id, top } => {
                    print_messages(&list_messages(&token, &chat_id, top)?)
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_teams_url_is_https() {
        assert!(DEFAULT_TEAMS_URL.starts_with("https://"));
    }
}
