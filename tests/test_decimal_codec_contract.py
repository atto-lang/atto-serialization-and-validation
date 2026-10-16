#!/usr/bin/env python3
"""Mutation checks for native int64 serialization source invariants."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from decimal_conversion_contract import validate_decimal_source

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "src/core/value.ores").read_text(encoding="utf-8")


class DecimalCodecContractTest(unittest.TestCase):
    def test_actual_source_meets_i64_safe_contract(self):
        self.assertEqual([], validate_decimal_source(SOURCE))

    def test_undefined_digit_regression_rejected(self):
        mutated = SOURCE.replace("let int digit =", "let int missing_digit =", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_zeroed_digit_expression_is_rejected(self):
        source_line = "let int digit = remainder < 0 ? 0 - remainder : remainder;"
        self.assertIn(source_line, SOURCE)
        mutated = SOURCE.replace(source_line, "let int digit = 0;", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_raw_non_ascii_digit_emission_is_rejected(self):
        source_line = "reversed.add(48 + digit);"
        self.assertIn(source_line, SOURCE)
        mutated = SOURCE.replace(source_line, "reversed.add(digit);", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_reversed_owner_shadowing_rejected(self):
        mutated = SOURCE.replace(
            "let List<int> reversed = new List<int>();",
            "let List<int> reversed = new List<int>();\n  let List<int> reversed = new List<int>();",
            1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_unbounded_loop_regression_rejected(self):
        mutated = SOURCE.replace("current = quotient;", "current = current;", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_missing_signed_remainder_rejected(self):
        mutated = SOURCE.replace("let int remainder = current - (quotient * 10);",
                                 "let int remainder = current;", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_unsafe_negation_rejected(self):
        mutated = SOURCE.replace("current = quotient;",
                                 "current = -current;\n    current = quotient;", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_explicit_digit_counter_must_remain_declared(self):
        self.assertIn("let int digit_count = 0;", SOURCE)
        mutated = SOURCE.replace("let int digit_count = 0;", "", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_explicit_digit_counter_must_advance(self):
        self.assertIn("digit_count = digit_count + 1;", SOURCE)
        mutated = SOURCE.replace("digit_count = digit_count + 1;", "", 1)
        self.assertTrue(validate_decimal_source(mutated))

    def test_list_length_arithmetic_regression_rejected(self):
        self.assertIn("let int i = digit_count;", SOURCE)
        mutated = SOURCE.replace(
            "let int i = digit_count;", "let int i = reversed.length - 1;", 1
        )
        self.assertTrue(validate_decimal_source(mutated))

    def test_excess_nested_loop_rejected(self):
        mutated = SOURCE.replace("loop do\n    if current == 0 then",
                                 "loop do\n    loop do\n    if current == 0 then", 1)
        self.assertTrue(validate_decimal_source(mutated))


if __name__ == "__main__":
    unittest.main()
