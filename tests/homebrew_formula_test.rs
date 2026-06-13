// ABOUTME: Homebrew formula generation tests for the release workflow.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn render_formula(version: &str, aarch64_sha: &str, x86_64_sha: &str) -> String {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("Formula/tm-watcher.rb");
    let status = Command::new("bash")
        .arg("scripts/render-homebrew-formula.sh")
        .arg(version)
        .arg(aarch64_sha)
        .arg(x86_64_sha)
        .arg(&output_path)
        .status()
        .unwrap();

    assert!(status.success());
    fs::read_to_string(output_path).unwrap()
}

#[test]
fn test_render_homebrew_formula_uses_prebuilt_release_tarballs() {
    let formula = render_formula(
        "0.2.0",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    assert!(formula.contains("class TmWatcher < Formula"));
    assert!(formula.contains("  version \"0.2.0\""));
    assert!(formula.contains(
        "url \"https://github.com/zzerding/tm-exclude-watcher/releases/download/v0.2.0/tm-watcher-v0.2.0-aarch64-apple-darwin.tar.gz\""
    ));
    assert!(formula.contains(
        "url \"https://github.com/zzerding/tm-exclude-watcher/releases/download/v0.2.0/tm-watcher-v0.2.0-x86_64-apple-darwin.tar.gz\""
    ));
    assert!(
        formula.contains(
            "sha256 \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\""
        )
    );
    assert!(
        formula.contains(
            "sha256 \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\""
        )
    );
    let arm_url = formula
        .find("tm-watcher-v0.2.0-aarch64-apple-darwin.tar.gz")
        .unwrap();
    let arm_sha = formula
        .find("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .unwrap();
    let intel_url = formula
        .find("tm-watcher-v0.2.0-x86_64-apple-darwin.tar.gz")
        .unwrap();
    let intel_sha = formula
        .find("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .unwrap();
    assert!(arm_url < arm_sha);
    assert!(arm_sha < intel_url);
    assert!(intel_url < intel_sha);
    assert!(formula.contains("bin.install \"tm-watcher\""));
}

#[test]
fn test_render_homebrew_formula_has_expected_tests_and_caveats() {
    let formula = render_formula(
        "0.2.0",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    assert!(formula.contains("assert_match version.to_s, shell_output"));
    assert!(formula.contains("tm-watcher --version"));
    assert!(formula.contains("assert_match \"tm-watcher\", shell_output"));
    assert!(formula.contains("tm-watcher --help"));
    assert!(formula.contains("tm-watcher is installed but not started automatically."));
    assert!(formula.contains("tm-watcher daemon start"));
    assert!(formula.contains("tm-watcher daemon status"));
    assert!(formula.contains("tm-watcher daemon stop"));
}

#[test]
fn test_render_homebrew_formula_does_not_define_service_or_source_build() {
    let formula = render_formula(
        "0.2.0",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );

    assert!(!formula.contains("service do"));
    assert!(!formula.contains("depends_on \"rust\""));
    assert!(!formula.contains("cargo install"));
    assert!(!formula.contains("system \"cargo\""));
}
