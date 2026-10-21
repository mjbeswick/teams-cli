# Teams CLI for AI agents

`teams` is a small Rust command-line wrapper around the Microsoft Teams web client. It also discovers local agent skills and can print their `SKILL.md` instructions so an agent can consume them directly.

This project is an independent web launcher, not an official Microsoft client and not a fork of [teams-for-linux](https://github.com/ISMAELMARTINEZ/teams-for-linux). It does not embed a browser, automate sign-in, inspect Teams pages, or call Microsoft Graph. Authentication remains in the browser session.

## Build

```sh
cargo build --release
```

## Use

```sh
# Discover skills in conventional project and user locations
teams skills list

# Add extra roots; roots can also be listed in TEAMS_CLI_SKILL_PATHS
teams --skills-dir ./agent-skills skills list

# Print the exact instructions an agent should follow
teams skills show code-review

# Open the Teams web client in the system browser
teams web

# Select another browser executable or URL
teams web --browser /path/to/browser --url https://teams.microsoft.com/
```

Skill discovery searches recursively for files named `SKILL.md` under:

- Explicit `--skills-dir` roots and paths in `TEAMS_CLI_SKILL_PATHS` (using the platform path separator).
- The current directory's `.agents/skills`, `.claude/skills`, `.opencode/skills`, and `skills` directories.
- The user's `~/.agents/skills`, `~/.claude/skills`, and `~/.config/opencode/skills` directories.

The optional YAML-style frontmatter fields `name` and `description` are used for listing; the complete file is retained for `skills show`. A missing name falls back to the containing directory name. If duplicate names exist, listing shows every match and `skills show` reports ambiguity instead of choosing arbitrarily.

## Agent integration

An agent can run `teams skills list`, select an applicable skill, then pass its name to `teams skills show <name>` and follow the returned instructions. `teams web` opens the interactive web client; browser UI automation and Teams APIs are intentionally outside this initial CLI scope.
