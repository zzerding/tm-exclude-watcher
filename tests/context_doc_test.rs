// ABOUTME: Long-term context documentation contract tests.

use std::fs;

fn context_doc() -> String {
    fs::read_to_string("docs/CONTEXT.md").unwrap()
}

#[test]
fn test_context_documents_current_release_model() {
    let context = context_doc();

    assert!(context.contains("## v0.2.0 发布范围"));
    assert!(context.contains("GitHub Release"));
    assert!(context.contains("macOS 双架构 tarball"));
    assert!(context.contains("SHA256SUMS"));
    assert!(context.contains("Homebrew 安装"));
    assert!(context.contains("安装后不自动启动 daemon"));
}

#[test]
fn test_context_documents_config_command() {
    let context = context_doc();

    assert!(context.contains("tm-watcher config show"));
    assert!(context.contains("config add-path"));
    assert!(context.contains("config add-rule"));
}

#[test]
fn test_context_documents_daemon_subcommands() {
    let context = context_doc();

    assert!(context.contains("tm-watcher daemon start"));
    assert!(context.contains("tm-watcher daemon stop"));
    assert!(context.contains("tm-watcher daemon status"));
    assert!(context.contains("tm-watcher daemon restart"));
}

#[test]
fn test_context_does_not_promise_unshipped_commands_or_old_homebrew_plan() {
    let context = context_doc();

    assert!(!context.contains("### 推迟到 v1.0.0"));
    assert!(!context.contains("- Homebrew 发布"));
}
