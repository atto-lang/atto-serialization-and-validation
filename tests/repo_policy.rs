use std::fs;
use std::path::Path;
fn inspect(path: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let item = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if item.is_dir() {
            if ![".git","target","node_modules"].contains(&name.as_ref()) {
                inspect(&item);
            }
        } else {
            assert_ne!(item.extension().and_then(|e| e.to_str()), Some("py"),
                "unexpected forbidden script: {}",item.display());
        }
    }
}
#[test]
fn rust_is_the_only_test_and_validation_script_language() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    inspect(root);
    let workflow=fs::read_to_string(root.join(".github/workflows/contract-conformance.yml")).unwrap();
    assert!(workflow.contains("cargo +1.88.0 test --all-targets"));
    assert!(!workflow.contains("setup-python"));
    assert!(!workflow.contains("python -m"));
}
