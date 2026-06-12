use std::collections::HashMap;

/// Convert a git remote URL into a browsable https URL.
/// `aliases` maps ssh-config host aliases to real hostnames (Host → HostName),
/// so remotes like `git@gitlab-crypto:group/repo.git` resolve to gitlab.com.
/// Returns None when the remote can't be turned into a web URL.
pub fn web_url_from_remote(remote: &str, aliases: &HashMap<String, String>) -> Option<String> {
    let remote = remote.trim();
    if remote.is_empty() {
        return None;
    }

    // https://host/path(.git) | http://host/path(.git)
    if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        let (host, path) = rest.split_once('/')?;
        // Strip embedded credentials (user@host).
        let host = host.rsplit('@').next()?;
        return Some(assemble(host, path, aliases));
    }

    // ssh://git@host[:port]/path(.git)
    if let Some(rest) = remote.strip_prefix("ssh://") {
        let rest = rest.rsplit('@').next()?; // drop user@
        let (host_port, path) = rest.split_once('/')?;
        let host = host_port.split(':').next()?;
        return Some(assemble(host, path, aliases));
    }

    // scp-like: git@host:path(.git)
    if let Some((user_host, path)) = remote.split_once(':') {
        if user_host.contains('/') {
            return None; // a local path like ./repo:x — not a remote
        }
        let host = user_host.rsplit('@').next()?;
        return Some(assemble(host, path, aliases));
    }

    None
}

fn assemble(host: &str, path: &str, aliases: &HashMap<String, String>) -> String {
    let host = aliases.get(host).map(String::as_str).unwrap_or(host);
    let path = path.trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    format!("https://{host}/{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aliases() -> HashMap<String, String> {
        HashMap::from([("gitlab-crypto".to_string(), "gitlab.com".to_string())])
    }

    #[test]
    fn converts_scp_like_remote() {
        assert_eq!(
            web_url_from_remote("git@github.com:user/repo.git", &HashMap::new()),
            Some("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn resolves_ssh_config_alias() {
        assert_eq!(
            web_url_from_remote("git@gitlab-crypto:vahagn.crypto.99-group/naeasy.git", &aliases()),
            Some("https://gitlab.com/vahagn.crypto.99-group/naeasy".to_string())
        );
    }

    #[test]
    fn converts_ssh_scheme_with_port() {
        assert_eq!(
            web_url_from_remote("ssh://git@git.example.org:2222/team/proj.git", &HashMap::new()),
            Some("https://git.example.org/team/proj".to_string())
        );
    }

    #[test]
    fn passes_through_https_stripping_dot_git() {
        assert_eq!(
            web_url_from_remote("https://github.com/user/repo.git", &HashMap::new()),
            Some("https://github.com/user/repo".to_string())
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(web_url_from_remote("", &HashMap::new()), None);
        assert_eq!(web_url_from_remote("/local/path/repo", &HashMap::new()), None);
    }
}
