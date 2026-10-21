# Teams CLI for AI agents

`teams` is a small Rust command-line wrapper around the Microsoft Teams web client. It also discovers local agent skills and can print their `SKILL.md` instructions so an agent can consume them directly.

This project is an independent client, not an official Microsoft client and not a fork of [teams-for-linux](https://github.com/ISMAELMARTINEZ/teams-for-linux). `teams web` does not embed a browser or automate sign-in; `teams chat` reads chats through Microsoft Graph with a per-run device-code sign-in (see below).

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

# Install this CLI's own skill into Claude/OpenCode/.agents skill directories
teams skills install
teams skills install --target claude --force

# Open the Teams web client in the system browser
teams web

# Select another browser executable or URL
teams web --browser /path/to/browser --url https://teams.microsoft.com/

# Sign in (device code) and list your chats
teams chat list

# Read the most recent messages in one chat
teams chat messages <chat-id> --top 20
```

### Reading chats (`teams chat`)

`teams chat list` and `teams chat messages` sign in with Microsoft's OAuth2
**device-code flow** and call Microsoft Graph (`GET /me/chats`,
`GET /me/chats/{id}/messages`) — they do not embed a browser, scrape the
Teams web UI, or automate the browser session `teams web` opens. The only
*functional* permission requested is the read-only `Chat.Read` scope;
sending, editing, or deleting messages is out of scope. `User.Read` is also
requested because Entra's device-code endpoint rejects `Chat.Read` on its
own with `AADSTS70011` — `User.Read` is the minimal addition that is
accepted and grants no more than the caller's own basic profile. **No token
is cached**: every invocation prints a fresh `https://microsoft.com/device`
code and requires you to complete sign-in again.

**Client ID and governance risk.** By default this CLI authenticates using
Microsoft's own first-party public client ID
(`d3590ed6-52b3-4102-aeff-aad2292ab01c`, Microsoft's "Microsoft Office"
application — the same fallback several open-source Graph CLI tools use,
e.g. [`teams-tui-go`](https://github.com/nospor/teams-tui-go)). It is *not*
the real Teams application's own client ID, but it is still a trusted
first-party Microsoft app. Signing in with a first-party client ID may be
treated by your tenant's Conditional Access or sign-in-risk policies as
coming from a trusted Microsoft app, which can bypass whatever review your
organization's own app-registration process is meant to enforce. If your
organization requires an explicitly registered, reviewed app for
third-party tools, confirm this approach with your security team before
relying on it, or override it with your own registration:

```sh
export TEAMS_CLI_CLIENT_ID=<your-app-registration-client-id>
export TEAMS_CLI_TENANT=<your-tenant-id>   # defaults to "common"
```

Your app registration needs "Allow public client flows" enabled and the
`User.Read` and `Chat.Read` delegated permissions.

Skill discovery searches recursively for files named `SKILL.md` under:

- Explicit `--skills-dir` roots and paths in `TEAMS_CLI_SKILL_PATHS` (using the platform path separator).
- The current directory's `.agents/skills`, `.claude/skills`, `.opencode/skills`, and `skills` directories.
- The user's `~/.agents/skills`, `~/.claude/skills`, and `~/.config/opencode/skills` directories.

The optional YAML-style frontmatter fields `name` and `description` are used for listing; the complete file is retained for `skills show`. A missing name falls back to the containing directory name. If duplicate names exist, listing shows every match and `skills show` reports ambiguity instead of choosing arbitrarily.

### Auto-discovery in your coding agent

`teams skills install` writes this CLI's own bundled skill (`skills/teams-cli/SKILL.md`) into `~/.claude/skills/teams-cli/`, `~/.config/opencode/skills/teams-cli/`, and `~/.agents/skills/teams-cli/` — the same directories `teams skills list` already scans. Once installed, any coding agent that scans those conventional directories (including `teams` itself) auto-discovers the `teams-cli` skill without further configuration. Pass `--target claude|opencode|agents` to limit which directory is written, and `--force` to overwrite an existing copy; otherwise an existing file is left as-is.

## Agent integration

An agent can run `teams skills list`, select an applicable skill, then pass its name to `teams skills show <name>` and follow the returned instructions. `teams web` opens the interactive web client; browser UI automation and Teams APIs are intentionally outside this initial CLI scope.
