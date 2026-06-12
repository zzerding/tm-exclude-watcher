// ABOUTME: Release workflow credential and trigger contract tests.

use std::fs;

fn release_workflow() -> String {
    fs::read_to_string(".github/workflows/release.yml").unwrap()
}

fn homebrew_tap_job(workflow: &str) -> &str {
    let start = workflow.find("  update-homebrew-tap:\n").unwrap();
    &workflow[start..]
}

#[test]
fn test_homebrew_tap_update_is_stable_tag_only() {
    let workflow = release_workflow();
    let job = homebrew_tap_job(&workflow);

    assert!(job.contains("needs.gate.outputs.is_tag_release == 'true'"));
    assert!(job.contains("needs.gate.outputs.is_prerelease != 'true'"));
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
