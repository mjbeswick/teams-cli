use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use teams_cli::{discover_skills, open_teams, print_skill, print_skills};

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Skills { command } => {
            let skills = discover_skills(&cli.skills_dirs)?;
            match command {
                SkillsCommand::List => print_skills(&skills),
                SkillsCommand::Show { name } => print_skill(&skills, &name)?,
            }
        }
        Command::Web { url, browser } => open_teams(&url, browser.as_deref())?,
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
