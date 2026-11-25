//! Regression guard: hosted contract CI must test the exact PR source tree.
//! This does NOT certify any runtime guest execution or independent approval.
const WORKFLOW: &str = include_str!("../.github/workflows/contract-conformance.yml");

#[test]
fn checkout_and_expected_head_are_same_exact_pr_commit() {
    const SHA_EXPR: &str = "github.event.pull_request.head.sha || github.sha";
    assert_eq!(WORKFLOW.matches(SHA_EXPR).count(), 2);
    assert!(WORKFLOW.contains("ref: "));
    assert!(WORKFLOW.contains("EXPECTED_HEAD: "));
    assert!(WORKFLOW.contains("test \"$(git rev-parse HEAD)\" = \"$EXPECTED_HEAD\""));
    assert!(WORKFLOW.contains("git rev-parse 'HEAD^{tree}'"));
}

#[test]
fn workflow_rejects_untracked_or_modified_fixture_trees_before_tests() {
    assert!(WORKFLOW.contains("test -z \"$(git status --porcelain)\""));
    assert!(WORKFLOW.contains("git ls-files -z"));
    assert!(WORKFLOW.contains("test ! -L \"$file\""));
    assert!(WORKFLOW.contains("git hash-object \"$file\""));
    assert!(WORKFLOW.contains("git rev-parse \"HEAD:$file\""));
    assert!(WORKFLOW.contains("count=$((count + 1))"));
    assert!(WORKFLOW.contains("test \"$count\" -gt 0"));
    assert!(WORKFLOW.contains("cargo +1.88.0 test --locked --all-targets"));
    assert!(WORKFLOW.contains("tsp compile contracts/v1/"));
    assert!(WORKFLOW.contains("wasm-tools component wit contracts/v1"));
    let exact_head_gate = WORKFLOW.find("Verify exact PR head, tree").unwrap();
    let rust_test = WORKFLOW
        .find("cargo +1.88.0 test --locked --all-targets")
        .unwrap();
    assert!(exact_head_gate < rust_test);
}
