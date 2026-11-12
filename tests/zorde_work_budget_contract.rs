//! Source-contract checks and a small independent arithmetic oracle.
//! These do NOT execute or certify the AttoLang guest implementation.
//! Native Java/GraalVM and LLVM checks remain separate required gates.

use std::{fs, path::PathBuf};

fn validator() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/validation/validation.ores");
    fs::read_to_string(path).expect("Zorde validator source is present")
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, rest) = source.split_once(start).expect("expected source function");
    let (part, _) = rest.split_once(end).expect("expected next source function");
    part
}

fn before(source: &str, earlier: &str, later: &str) {
    let a = source.find(earlier).expect("earlier guard missing");
    let b = source.find(later).expect("later work missing");
    assert!(a < b, "{earlier:?} must precede {later:?}");
}

#[test]
fn all_dynamic_entrypoints_share_preflight() {
    let src = validator();
    for (start, end) in [
        ("pub fnc validate_value(", "fnc remember_text_index("),
        ("pub fnc validate_object(", "pub fnc report_is_valid("),
    ] {
        assert!(section(&src, start, end)
            .contains("checked_value_tree_valid(rt borrow value)"));
    }
    assert!(section(&src, "pub fnc validate_value_checked(", "pub fnc validate_object_checked(")
        .contains("validate_value(value, rule, options)"));
    assert!(src.contains("validate_object(value, schema, options)"));
    assert!(src.contains("\"invalid_value_tree\""));
}

#[test]
fn graph_preflight_guards_before_nested_work() {
    let src = validator();
    let graph = section(&src, "fnc checked_value_tree_valid(", "pub fnc validate_value_checked(");
    for case in [
        "max_checked_value_nodes()",
        "max_checked_value_depth()",
        "max_checked_value_bytes()",
        "number_lexeme_valid(rt borrow value)",
        "utf8_is_valid(value.string_value.bytes)",
        "byte < 0 || byte > 255",
        "object_keys_unique_bounded(rt borrow value, key_work)",
        "pending.add(entry.key)",
        "pending.add(entry.value)",
    ] {
        assert!(graph.contains(case), "missing graph guard: {case}");
    }
    before(graph, "value.number_lexeme.length > max_checked_value_bytes()",
        "number_lexeme_valid(rt borrow value)");
    before(graph, "value.string_value.bytes.length > max_checked_value_bytes()",
        "utf8_is_valid(value.string_value.bytes)");
}

#[test]
fn unique_item_work_is_charged_before_hash_and_comparison() {
    let src = validator();
    let unique = section(&src, "fnc validate_unique(", "fnc validate_rule_at(");
    // These anchors must track the metered runtime API, not the removed
    // compatibility calls. Otherwise this Rust gate fails before running
    // even a single AttoLang guest case.
    before(unique, "value.array_value.length > max_unique_validation_items()",
        "val ValueHashAttempt hash_attempt = value_hash_with_fuel(");
    before(unique, "unique_value_work_cost(rt borrow item)",
        "val ValueHashAttempt hash_attempt = value_hash_with_fuel(");
    before(unique, "candidate_checks >= max_unique_candidate_checks()",
        "val ValueEqualAttempt equal_attempt = value_deep_equal_with_fuel(");
    before(unique, "candidate_costs[i]",
        "val ValueEqualAttempt equal_attempt = value_deep_equal_with_fuel(");
    assert!(!unique.contains("let int hash = value_hash(item)"));
    assert!(!unique.contains("if value_deep_equal(candidate, item)"));
    for code in [
        "\"unique_items_limit\"", "\"unique_items_complexity_limit\"",
        "\"unique_items_work_limit\"",
    ] {
        assert!(unique.contains(code), "missing fail-closed code: {code}");
    }
}

#[test]
fn schema_keys_and_field_lookups_have_work_refusal_paths() {
    let src = validator();
    let schema = section(&src, "fnc object_schema_valid(", "fnc empty_key(");
    before(schema, "max_checked_value_bytes() - schema_bytes",
        "let int hash = text_hash(field.key)");
    assert!(schema.contains("charge_graph_key_compare("));
    let obj = section(&src, "pub fnc validate_object(", "pub fnc report_is_valid(");
    assert!(obj.contains("lookup_work"));
    assert!(obj.contains("if index == -2"));
    assert!(obj.contains("if schema_index_for_member == -2"));
    assert!(obj.contains("\"schema_lookup_work_limit\""));
}

#[test]
fn worst_case_numeric_budget_reference_model() {
    const FUEL: u64 = 1_000_000;
    const MAX_OUTER_COMPARISONS: u64 = 16_384;
    // One child object member gives conservative factor 1*1+1+1 == 3.
    let single_object = 1_u64 + 3 * (3 + 1 + 1_000);
    assert!(single_object * 400 > FUEL);
    assert!(64 * (1 + 2) < FUEL);

    // No attempt to perform a 16,385th equality comparison.
    let comparisons = 200 * 199 / 2;
    assert!(comparisons > MAX_OUTER_COMPARISONS);
    assert_eq!(MAX_OUTER_COMPARISONS + 1, 16_385);
}

#[test]
fn budget_math_rejects_overflow_and_never_recovers_denial() {
    fn product(a: i64, b: i64) -> Option<i64> {
        if !(0..=1_000_000).contains(&a) || !(0..=1_000_000).contains(&b) {
            return None;
        }
        let result = a * b; // <= 10^12, safely within signed i64
        (result <= 1_000_000).then_some(result)
    }
    fn add(used: i64, extra: i64) -> Option<i64> {
        if !(0..=1_000_000).contains(&used) || extra < 0
            || extra > 1_000_000 - used
        {
            return None;
        }
        Some(used + extra)
    }
    assert_eq!(product(1, 1_000_000), Some(1_000_000));
    assert_eq!(product(2, 500_001), None);
    assert_eq!(product(1_000_001, 1), None);
    assert_eq!(add(999_999, 1), Some(1_000_000));
    assert_eq!(add(1_000_000, 1), None);
    assert_eq!(add(-1, 1), None);
}

#[test]
fn metered_value_hash_and_equality_have_tristate_exhaustion() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/core/value.ores");
    let src = fs::read_to_string(path).expect("metered Value core is present");
    let hash = section(
        &src,
        "fnc value_hash_fuel_depth(",
        "pub fnc value_hash_with_fuel(",
    );
    let equal = section(
        &src,
        "fnc value_equal_fuel_depth(",
        "pub fnc value_deep_equal_with_fuel(",
    );
    assert!(src.contains("pub define class ValueWorkFuel as"));
    assert!(src.contains("pub define class ValueHashAttempt as"));
    assert!(src.contains("pub define class ValueEqualAttempt as"));
    assert!(src.contains("pub fnc value_hash_with_fuel("));
    assert!(src.contains("pub fnc value_deep_equal_with_fuel("));
    assert!(hash.contains("!value_fuel_charge(fuel, 1)"));
    assert!(equal.contains("!value_fuel_charge(fuel, 1)"));
    assert!(!equal.contains("if left == right then"),
        "identity short-circuit must not evade budget");
}

#[test]
fn every_byte_and_container_candidate_is_charged() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/core/value.ores");
    let src = fs::read_to_string(path).expect("metered Value core is present");
    let bh = section(&src, "fnc fuel_hash_bytes(", "fnc fuel_equal_bytes(");
    let be = section(&src, "fnc fuel_equal_bytes(", "fnc value_hash_fuel_depth(");
    let eq = section(&src, "fnc value_equal_fuel_depth(", "pub fnc value_deep_equal_with_fuel(");
    before(bh, "value_fuel_charge(fuel, 1)", "hash = hash_mix(hash, byte)");
    before(be, "value_fuel_charge(fuel, 1)", "if left[i] != right[i]");
    assert!(eq.contains("if !used[index] then"));
    assert!(eq.contains("if !value_fuel_charge(fuel, 1) then"));
    assert!(eq.matches("used.add(false);").count() == 2);
}

#[test]
fn validator_refuses_unfinished_hash_and_equality() {
    let src = validator();
    let body = section(&src, "fnc validate_unique(", "fnc validate_rule_at(");
    before(body, "val ValueHashAttempt hash_attempt = value_hash_with_fuel(",
        "val int hash = hash_attempt.hash;");
    before(body, "if !hash_attempt.completed then",
        "val int hash = hash_attempt.hash;");
    before(body, "if !equal_attempt.completed then",
        "if equal_attempt.equal then");
    assert!(body.contains(
        "let ValueWorkFuel fuel = new ValueWorkFuel(max_unique_validation_work())"
    ));
    assert!(!body.contains("value_hash(item)"));
    assert!(!body.contains("value_deep_equal(candidate, item)"));
    assert!(body.contains("\"unique_items_work_limit\""));
}

#[test]
fn native_guest_source_covers_metered_fuel_and_equivalence() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/validation_test.ores");
    let src = fs::read_to_string(path).expect("Zorde guest source is present");
    for check in [
        "zero fuel cannot return a successful object hash",
        "metered hash must match canonical unordered object hash",
        "metered arbitrary-map equality must be order-insensitive",
        "string hashing must stop as soon as byte fuel is exhausted",
        "identical aliased strings must not bypass metered byte comparisons",
        "metered hashing must share its exhausted budget across calls",
        "negative fuel must fail closed",
    ] {
        assert!(src.contains(check), "missing native negative case: {check}");
    }
}
