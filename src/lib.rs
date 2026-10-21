use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub mod graph;

/// A locally discovered agent skill, backed by its original SKILL.md file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Find skills in common agent directories, plus any caller-provided roots.
///
/// Each root is searched recursively for `SKILL.md`; symlinks are not followed.
pub fn discover_skills(extra_roots: &[PathBuf]) -> Result<Vec<Skill>> {
    discover_in_roots(&skill_roots(extra_roots))
}

fn discover_in_roots(roots: &[PathBuf]) -> Result<Vec<Skill>> {
    let mut files = Vec::new();
    let mut seen_files = HashSet::new();

    for root in roots {
        if !root.is_dir() {
            continue;
        }

        for entry in WalkDir::new(root).follow_links(false).into_iter() {
            let entry = entry.with_context(|| format!("searching skills in {}", root.display()))?;
            if entry.file_type().is_file()
                && entry.file_name() == "SKILL.md"
                && seen_files.insert(entry.path().to_path_buf())
            {
                files.push(entry.path().to_path_buf());
            }
        }
    }

    files.sort();
    files.into_iter().map(|path| read_skill(&path)).collect()
}

fn skill_roots(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = extra_roots.to_vec();
    if let Some(paths) = env::var_os("TEAMS_CLI_SKILL_PATHS") {
        roots.extend(env::split_paths(&paths));
    }

    if let Ok(current_dir) = env::current_dir() {
        roots.extend([
            current_dir.join(".agents/skills"),
            current_dir.join(".claude/skills"),
            current_dir.join(".opencode/skills"),
            current_dir.join("skills"),
        ]);
    }
    if let Some(home) = home_dir() {
        roots.extend([
            home.join(".agents/skills"),
            home.join(".claude/skills"),
            home.join(".config/opencode/skills"),
        ]);
    }

    let mut seen = HashSet::new();
    roots
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

fn read_skill(path: &Path) -> Result<Skill> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading skill file {}", path.display()))?;
    let (name, description) = parse_frontmatter(&content);
    let fallback_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed-skill");

    Ok(Skill {
        name: name.unwrap_or_else(|| fallback_name.to_string()),
        description: description.unwrap_or_default(),
        path: path.to_path_buf(),
    })
}

fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }

    let mut name = None;
    let mut description = None;
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = unquote(value.trim());
        match key.trim() {
            "name" if !value.is_empty() => name = Some(value.to_string()),
            "description" if !value.is_empty() => description = Some(value.to_string()),
            _ => {}
        }
    }
    (name, description)
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

pub fn print_skills(skills: &[Skill]) {
    if skills.is_empty() {
        println!("No skills found. Add a SKILL.md under a skills directory or pass --skills-dir.");
        return;
    }

    for skill in skills {
        let description = if skill.description.is_empty() {
            "(no description)"
        } else {
            &skill.description
        };
        println!("{}\t{}\t{}", skill.name, description, skill.path.display());
    }
}

pub fn print_skill(skills: &[Skill], name: &str) -> Result<()> {
    let matches: Vec<_> = skills
        .iter()
        .filter(|skill| skill.name == name || skill_directory_name(skill) == name)
        .collect();
    match matches.as_slice() {
        [skill] => print!("{}", fs::read_to_string(&skill.path)?),
        [] => bail!("skill '{name}' was not found; run `teams skills list`"),
        _ => {
            let paths = matches
                .iter()
                .map(|skill| skill.path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n  ");
            bail!("skill name '{name}' is ambiguous; matching files:\n  {paths}");
        }
    }
    Ok(())
}

fn skill_directory_name(skill: &Skill) -> &str {
    skill
        .path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("")
}

/// This CLI's own bundled skill, so agents can discover how to drive `teams`.
const BUNDLED_SKILL_NAME: &str = "teams-cli";
const BUNDLED_SKILL_CONTENT: &str = include_str!("../skills/teams-cli/SKILL.md");

/// An agent-skill directory convention this CLI can install its bundled skill into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillTarget {
    Claude,
    Opencode,
    Agents,
}

impl SkillTarget {
    pub const ALL: [SkillTarget; 3] = [
        SkillTarget::Claude,
        SkillTarget::Opencode,
        SkillTarget::Agents,
    ];

    fn relative_dir(self) -> &'static str {
        match self {
            SkillTarget::Claude => ".claude/skills",
            SkillTarget::Opencode => ".config/opencode/skills",
            SkillTarget::Agents => ".agents/skills",
        }
    }
}

impl std::str::FromStr for SkillTarget {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "claude" => Ok(SkillTarget::Claude),
            "opencode" => Ok(SkillTarget::Opencode),
            "agents" => Ok(SkillTarget::Agents),
            other => Err(format!(
                "unknown skill target '{other}' (expected claude, opencode, or agents)"
            )),
        }
    }
}

/// The outcome of attempting to install the bundled skill into one target directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed(PathBuf),
    AlreadyPresent(PathBuf),
    NoHomeDirectory,
}

/// Install this CLI's bundled `teams-cli` skill into the given targets (or all known
/// targets when empty), so agents that scan those directories auto-discover it.
///
/// Returns one outcome per target in the same order; nothing is overwritten unless
/// `force` is set.
pub fn install_bundled_skill(
    targets: &[SkillTarget],
    force: bool,
) -> Result<Vec<(SkillTarget, InstallOutcome)>> {
    let targets: &[SkillTarget] = if targets.is_empty() {
        &SkillTarget::ALL
    } else {
        targets
    };
    let Some(home) = home_dir() else {
        return Ok(targets
            .iter()
            .map(|target| (*target, InstallOutcome::NoHomeDirectory))
            .collect());
    };

    let mut results = Vec::with_capacity(targets.len());
    for &target in targets {
        let dir = home.join(target.relative_dir()).join(BUNDLED_SKILL_NAME);
        let file = dir.join("SKILL.md");
        if file.is_file() && !force {
            results.push((target, InstallOutcome::AlreadyPresent(file)));
            continue;
        }

        fs::create_dir_all(&dir)
            .with_context(|| format!("creating skill directory {}", dir.display()))?;
        fs::write(&file, BUNDLED_SKILL_CONTENT)
            .with_context(|| format!("writing skill file {}", file.display()))?;
        results.push((target, InstallOutcome::Installed(file)));
    }
    Ok(results)
}

pub fn print_install_results(results: &[(SkillTarget, InstallOutcome)]) {
    for (target, outcome) in results {
        let label = match target {
            SkillTarget::Claude => "claude",
            SkillTarget::Opencode => "opencode",
            SkillTarget::Agents => "agents",
        };
        match outcome {
            InstallOutcome::Installed(path) => println!("installed\t{label}\t{}", path.display()),
            InstallOutcome::AlreadyPresent(path) => {
                println!(
                    "already present (use --force to overwrite)\t{label}\t{}",
                    path.display()
                )
            }
            InstallOutcome::NoHomeDirectory => {
                println!("skipped\t{label}\t(no home directory found)")
            }
        }
    }
}

/// Open the Teams web client without invoking a shell.
pub fn open_teams(url: &str, browser: Option<&Path>) -> Result<()> {
    let parsed = url::Url::parse(url).context("invalid Teams URL")?;
    if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
        bail!("Teams URL must be an absolute http(s) URL");
    }

    if let Some(browser) = browser {
        Command::new(browser)
            .arg(url)
            .spawn()
            .with_context(|| format!("launching browser {}", browser.display()))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    bail!("no system browser opener is configured for this platform; pass --browser");

    command
        .arg(url)
        .spawn()
        .context("launching the system browser")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("teams-cli-test-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn parses_skill_frontmatter_and_quoted_values() {
        let content =
            "---\nname: \"code-review\"\ndescription: 'Review changes safely'\n---\nInstructions\n";
        assert_eq!(
            parse_frontmatter(content),
            (
                Some("code-review".to_string()),
                Some("Review changes safely".to_string())
            )
        );
    }

    #[test]
    fn discovers_skill_files_recursively_and_uses_directory_fallback() {
        let root = scratch_dir();
        let nested = root.join("group").join("fallback-name");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("SKILL.md"),
            "---\ndescription: A test skill\n---\nDo the thing.\n",
        )
        .unwrap();

        let skills = discover_in_roots(std::slice::from_ref(&root)).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "fallback-name");
        assert_eq!(skills[0].description, "A test skill");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_http_teams_urls() {
        assert!(open_teams("javascript:alert(1)", None).is_err());
        assert!(
            open_teams(
                "https://teams.microsoft.com/",
                Some(Path::new("/missing/browser"))
            )
            .is_err()
        );
    }

    /// Serializes tests that temporarily override `$HOME`, since environment
    /// variables are process-global.
    static HOME_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn installs_bundled_skill_and_skips_existing_unless_forced() {
        let _guard = HOME_ENV_LOCK.lock().unwrap();
        let fake_home = scratch_dir();
        fs::create_dir_all(&fake_home).unwrap();
        let previous_home = env::var_os("HOME");
        unsafe {
            env::set_var("HOME", &fake_home);
        }

        let claude_file = fake_home.join(".claude/skills/teams-cli/SKILL.md");

        let first = install_bundled_skill(&[SkillTarget::Claude], false).unwrap();
        assert_eq!(
            first,
            vec![(
                SkillTarget::Claude,
                InstallOutcome::Installed(claude_file.clone())
            )]
        );
        assert_eq!(
            fs::read_to_string(&claude_file).unwrap(),
            BUNDLED_SKILL_CONTENT
        );

        let second = install_bundled_skill(&[SkillTarget::Claude], false).unwrap();
        assert_eq!(
            second,
            vec![(
                SkillTarget::Claude,
                InstallOutcome::AlreadyPresent(claude_file.clone())
            )]
        );

        fs::write(&claude_file, "stale").unwrap();
        let forced = install_bundled_skill(&[SkillTarget::Claude], true).unwrap();
        assert_eq!(
            forced,
            vec![(
                SkillTarget::Claude,
                InstallOutcome::Installed(claude_file.clone())
            )]
        );
        assert_eq!(
            fs::read_to_string(&claude_file).unwrap(),
            BUNDLED_SKILL_CONTENT
        );

        match previous_home {
            Some(home) => unsafe { env::set_var("HOME", home) },
            None => unsafe { env::remove_var("HOME") },
        }
        fs::remove_dir_all(fake_home).unwrap();
    }

    #[test]
    fn parses_skill_target_names() {
        assert_eq!(
            "claude".parse::<SkillTarget>().unwrap(),
            SkillTarget::Claude
        );
        assert_eq!(
            "opencode".parse::<SkillTarget>().unwrap(),
            SkillTarget::Opencode
        );
        assert_eq!(
            "agents".parse::<SkillTarget>().unwrap(),
            SkillTarget::Agents
        );
        assert!("bogus".parse::<SkillTarget>().is_err());
    }
}
