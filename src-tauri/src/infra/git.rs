use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// Git operations naeasy needs: remote lookup for "open in browser" and the
/// branch plumbing behind fast-start. Behind a trait so the service is
/// unit-testable with a fake.
pub trait GitClient: Send + Sync {
    /// The `origin` remote URL of the repo, if any.
    fn remote_url(&self, project_path: &str) -> Option<String>;
    /// Host alias -> real hostname pairs from ~/.ssh/config.
    fn ssh_aliases(&self) -> HashMap<String, String>;
    fn branch_exists(&self, project_path: &str, branch: &str) -> bool;
    fn checkout(&self, project_path: &str, branch: &str) -> Result<(), String>;
    /// Create `branch` from the freshest `base` available: tries
    /// `git fetch origin base` first, then branches from `origin/base`,
    /// falling back to the local `base` (offline case).
    fn create_branch_from(&self, project_path: &str, branch: &str, base: &str)
        -> Result<(), String>;
}

pub struct SystemGitClient;

fn git(project_path: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .output()
        .map_err(|e| format!("git not available: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            err
        })
    }
}

impl GitClient for SystemGitClient {
    fn remote_url(&self, project_path: &str) -> Option<String> {
        git(project_path, &["remote", "get-url", "origin"]).ok()
    }

    fn ssh_aliases(&self) -> HashMap<String, String> {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return HashMap::new();
        };
        let Ok(text) = std::fs::read_to_string(home.join(".ssh/config")) else {
            return HashMap::new();
        };
        parse_ssh_aliases(&text)
    }

    fn branch_exists(&self, project_path: &str, branch: &str) -> bool {
        git(
            project_path,
            &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")],
        )
        .is_ok()
    }

    fn checkout(&self, project_path: &str, branch: &str) -> Result<(), String> {
        git(project_path, &["checkout", branch]).map(|_| ())
    }

    fn create_branch_from(
        &self,
        project_path: &str,
        branch: &str,
        base: &str,
    ) -> Result<(), String> {
        // Best effort freshness; offline fetch failure is fine.
        let fetched = git(project_path, &["fetch", "origin", base]).is_ok();
        if fetched {
            if git(project_path, &["checkout", "-b", branch, &format!("origin/{base}")]).is_ok() {
                return Ok(());
            }
        }
        git(project_path, &["checkout", "-b", branch, base]).map(|_| ())
    }
}

/// Parse `Host alias` / `HostName real` pairs out of an ssh config.
/// Wildcard hosts are skipped; a Host line may declare several aliases.
fn parse_ssh_aliases(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or_default().to_ascii_lowercase();
        if key == "host" {
            current = parts
                .filter(|a| !a.contains('*') && !a.contains('?'))
                .map(str::to_string)
                .collect();
        } else if key == "hostname" {
            if let Some(real) = parts.next() {
                for alias in &current {
                    map.insert(alias.clone(), real.to_string());
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_and_skips_wildcards() {
        let cfg = r#"
# comment
Host gitlab-crypto github-work gitlab.com
    HostName gitlab.com
    User git

Host *
    AddKeysToAgent yes
"#;
        let map = parse_ssh_aliases(cfg);
        assert_eq!(map.get("gitlab-crypto").map(String::as_str), Some("gitlab.com"));
        assert_eq!(map.get("github-work").map(String::as_str), Some("gitlab.com"));
        assert!(!map.keys().any(|k| k.contains('*')));
    }
}
