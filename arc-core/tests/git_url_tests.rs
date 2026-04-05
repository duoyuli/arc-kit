use arc_core::market::git_url::parse_git_remote_parts;

#[test]
fn parses_https_git_remote() {
    let result = parse_git_remote_parts("https://github.com/openai/codex.git");
    assert_eq!(result, Some(("openai".to_string(), "codex".to_string())));
}

#[test]
fn parses_ssh_git_remote() {
    let result = parse_git_remote_parts("git@github.com:openai/codex.git");
    assert_eq!(result, Some(("openai".to_string(), "codex".to_string())));
}

#[test]
fn parses_ssh_protocol_url() {
    let result = parse_git_remote_parts("ssh://git@git.example.com/acme/toolkit.git");
    assert_eq!(result, Some(("acme".to_string(), "toolkit".to_string())));
}

#[test]
fn rejects_invalid_git_remote() {
    assert_eq!(parse_git_remote_parts("hello-world"), None);
}
