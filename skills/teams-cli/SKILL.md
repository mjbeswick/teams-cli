---
name: teams-cli
description: Use when the user wants to open Microsoft Teams from the command line, or when an agent needs to discover and read other local SKILL.md instructions on this machine. Activates for "open Teams", "launch Teams", or "what skills are installed".
---

# teams-cli

`teams` is a small Rust CLI that (1) opens the Microsoft Teams web client in the
system browser and (2) discovers and prints local agent `SKILL.md` files.

## Opening Teams

```sh
teams web
```

This opens `https://teams.microsoft.com/` in the system browser (or an
explicit `--browser <path>` / `--url <url>`). Sign-in happens in the browser;
`teams` does not read, store, or automate browser credentials, cookies, or
tokens, and it does not call the Teams or Microsoft Graph APIs. Do not treat a
successful launch as proof of a signed-in session — only the user can
confirm that from the browser window.

## Discovering skills

```sh
teams skills list
teams skills show <name>
```

`skills list` recursively searches for `SKILL.md` files under conventional
project (`.agents/skills`, `.claude/skills`, `.opencode/skills`, `skills`) and
user (`~/.agents/skills`, `~/.claude/skills`,
`~/.config/opencode/skills`) directories, plus any `--skills-dir` or
`TEAMS_CLI_SKILL_PATHS` roots. `skills show <name>` prints one skill's full
instructions by its frontmatter `name` (or containing directory name as a
fallback); an ambiguous or unknown name fails explicitly rather than guessing.

## Installing this skill elsewhere

```sh
teams skills install            # install into every known agent skill directory
teams skills install --target claude     # claude | opencode | agents
teams skills install --force    # overwrite an existing copy of this file
```

This writes this exact `SKILL.md` into `~/.claude/skills/teams-cli/`,
`~/.config/opencode/skills/teams-cli/`, and `~/.agents/skills/teams-cli/`, so
any coding agent that already scans those directories (including `teams`
itself) auto-discovers it. It never overwrites unrelated skills and skips an
existing file unless `--force` is given.
