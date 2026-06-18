// ABOUTME: Release workflow credential and trigger contract tests.

use std::fs;

fn release_workflow() -> String {
    fs::read_to_string(".github/workflows/release.yml").unwrap()
}

fn release_doc() -> String {
    fs::read_to_string("docs/release.md").unwrap()
}

fn homebrew_tap_job(workflow: &str) -> &str {
    let start = workflow.find("  update-homebrew-tap:\n").unwrap();
    &workflow[start..]
}

fn publish_job(workflow: &str) -> &str {
    let start = workflow.find("  publish:\n").unwrap();
    let end = workflow.find("  update-homebrew-tap:\n").unwrap();
    &workflow[start..end]
}

#[test]
fn test_homebrew_tap_update_is_stable_tag_or_explicit_repair_only() {
    let workflow = release_workflow();
    let job = homebrew_tap_job(&workflow);

    assert!(job.contains("needs.gate.outputs.is_tag_release == 'true'"));
    assert!(job.contains("inputs.update_homebrew_tap == true"));
    assert!(job.contains("needs.gate.outputs.is_prerelease != 'true'"));
    assert!(workflow.contains("update_homebrew_tap:"));
    assert!(workflow.contains("Update Homebrew tap for an existing stable release"));
}

#[test]
fn test_homebrew_tap_checkout_uses_scoped_secret() {
    let workflow = release_workflow();
    let job = homebrew_tap_job(&workflow);
    let check_token = job.find("- name: Check Homebrew tap token").unwrap();
    let checkout_tap = job.find("- name: Checkout Homebrew tap").unwrap();

    assert!(check_token < checkout_tap);
    assert!(job.contains("repository: zzerding/homebrew-tap"));
    assert!(job.contains("token: ${{ secrets.HOMEBREW_TAP_TOKEN }}"));
    assert!(job.contains("HOMEBREW_TAP_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}"));
    assert!(job.contains("HOMEBREW_TAP_TOKEN is not configured."));
    assert!(job.contains("zzerding/homebrew-tap with Contents read/write"));
    assert!(!job.contains("echo \"$HOMEBREW_TAP_TOKEN"));
    assert!(!job.contains("printf \"%s\\n\" \"$HOMEBREW_TAP_TOKEN"));
}

#[test]
fn test_release_workflow_does_not_hardcode_github_token_values() {
    let workflow = release_workflow();
    let token_prefixes = ["github_pat_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_"];

    for prefix in token_prefixes {
        assert!(
            !workflow.contains(prefix),
            "found hardcoded token prefix: {prefix}"
        );
    }
}

#[test]
fn test_publish_job_checks_out_source_before_release_create() {
    let workflow = release_workflow();
    let job = publish_job(&workflow);
    let checkout = job.find("- name: Checkout").unwrap();
    let download_archives = job.find("- name: Download release archives").unwrap();
    let create_release = job.find("- name: Create GitHub Release").unwrap();

    assert!(checkout < download_archives);
    assert!(download_archives < create_release);
    assert!(job.contains("uses: actions/checkout@v4"));
    assert!(job.contains("--verify-tag"));
    assert!(job.contains("Require existing release for Homebrew tap repair"));
    assert!(job.contains("needs.gate.outputs.is_tag_release == 'true' && steps.existing_release.outputs.release_exists != 'true'"));
    assert!(job.contains("workflow_dispatch Homebrew tap repair requires an existing release"));
}

#[test]
fn test_homebrew_tap_downloads_checksums_with_explicit_repo() {
    let workflow = release_workflow();
    let job = homebrew_tap_job(&workflow);

    assert!(job.contains("gh release download \"${{ needs.gate.outputs.tag }}\""));
    assert!(job.contains("--repo \"${{ github.repository }}\""));
}

#[test]
fn test_release_docs_match_workflow_runner_and_asset_contract() {
    let workflow = release_workflow();
    let release_doc = release_doc();

    for expected in [
        "macos-15",
        "macos-15-intel",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "tm-watcher-v0.2.0-aarch64-apple-darwin.tar.gz",
        "tm-watcher-v0.2.0-x86_64-apple-darwin.tar.gz",
        "CHANGELOG.md",
        "SHA256SUMS",
    ] {
        assert!(release_doc.contains(expected));
    }

    assert!(workflow.contains("macos-15"));
    assert!(workflow.contains("macos-15-intel"));
    assert!(workflow.contains("aarch64-apple-darwin.tar.gz"));
    assert!(workflow.contains("x86_64-apple-darwin.tar.gz"));
    assert!(workflow.contains("CHANGELOG.md"));
    assert!(workflow.contains("SHA256SUMS"));
    assert!(!workflow.contains("macos-14"));

    assert!(release_doc.contains("RC 只发布 GitHub prerelease，不更新 Homebrew formula"));
    assert!(release_doc.contains("stable 发布 GitHub Release，并自动更新 Homebrew formula"));
    assert!(release_doc.contains("formula 不定义 `service do`"));
}
