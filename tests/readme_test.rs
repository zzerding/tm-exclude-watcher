// ABOUTME: README release and installation contract tests.

use std::fs;

fn readme() -> String {
    fs::read_to_string("README.md").unwrap()
}

#[test]
fn test_readme_documents_supported_install_paths() {
    let readme = readme();

    assert!(readme.contains("brew tap zzerding/tap"));
    assert!(readme.contains("brew install tm-watcher"));
    assert!(readme.contains("stable 发布后"));
    assert!(readme.contains("cargo install --path ."));
    assert!(readme.contains("GitHub Release"));
    assert!(readme.contains("tm-watcher-v<version>-aarch64-apple-darwin.tar.gz"));
    assert!(readme.contains("tm-watcher-v<version>-x86_64-apple-darwin.tar.gz"));
    assert!(readme.contains("VERSION=<version>"));
    assert!(readme.contains("shasum -a 256 -c SHA256SUMS"));
}

#[test]
fn test_readme_documents_homebrew_daemon_lifecycle() {
    let readme = readme();

    assert!(readme.contains("Homebrew 安装后不会自动启动 daemon"));
    assert!(readme.contains("tm-watcher daemon start"));
    assert!(readme.contains("tm-watcher daemon status"));
    assert!(readme.contains("tm-watcher daemon stop"));
    assert!(readme.contains("tm-watcher daemon restart"));
}

#[test]
fn test_readme_documents_config_command() {
    let readme = readme();

    assert!(readme.contains("tm-watcher config show"));
    assert!(readme.contains("tm-watcher config add-path"));
    assert!(readme.contains("tm-watcher config add-rule"));
}

#[test]
fn test_readme_does_not_promise_unshipped_release_features() {
    let readme = readme();
    let unsupported_phrases = [
        "brew services",
        "service do",
        "tm-watcher doctor",
        "Homebrew service",
        "notarization",
        "GUI",
    ];

    for phrase in unsupported_phrases {
        assert!(
            !readme.contains(phrase),
            "README promises unsupported feature: {phrase}"
        );
    }
}
