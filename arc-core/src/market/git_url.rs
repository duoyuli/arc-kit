use regex::Regex;

pub fn parse_git_remote_parts(url: &str) -> Option<(String, String)> {
    static SSH_RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"^[^@]+@[^:]+:(?P<owner>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?$").unwrap()
    });
    static URL_RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"^(?:https|git|ssh|file)://.+/(?P<owner>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?/?$")
            .unwrap()
    });

    let captures = SSH_RE.captures(url).or_else(|| URL_RE.captures(url))?;
    let owner = captures.name("owner")?.as_str().to_string();
    let repo = captures.name("repo")?.as_str().to_string();
    Some((owner, repo))
}

/// Derive a slug identifier from a git URL (e.g. "owner-repo").
pub fn slug_from_git_url(git_url: &str) -> String {
    if let Some((owner, repo)) = parse_git_remote_parts(git_url) {
        format!("{owner}-{repo}")
    } else {
        git_url
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_lowercase()
    }
}
