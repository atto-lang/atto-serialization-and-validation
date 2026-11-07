import json
import pathlib
import re
import unittest

from jsonschema import Draft202012Validator

ROOT = pathlib.Path(__file__).resolve().parents[1] / 'contracts' / 'v1'
SCHEMA = json.loads((ROOT / 'actor-envelope.schema.json').read_text())
TSP = (ROOT / 'actor-envelope.tsp').read_text()
WIT = (ROOT / 'actor-envelope.wit').read_text()

BASE = {'version': 1, 'message_id': 'm-1', 'source_actor': 'source-1'}
GOOD = [
    {**BASE, 'event': 'message', 'target_actor': 'worker-1', 'payload_utf8': ''},
    {**BASE, 'event': 'failure', 'error_code': 'IO_ERROR'},
    {**BASE, 'event': 'shutdown'},
]
BAD = [
    {**GOOD[0], 'version': 2},
    {**GOOD[0], 'event': 'unknown'},
    {**GOOD[0], 'extra': True},
    {**GOOD[0], 'target_actor': ''},
    {**GOOD[0], 'message_id': ''},
    {**GOOD[0], 'message_id': 'x' * 129},
    {k: v for k, v in GOOD[0].items() if k != 'payload_utf8'},
    {**GOOD[0], 'error_code': 'IO_ERROR'},
    {**GOOD[1], 'error_code': 'bad-code'},
    {**GOOD[1], 'target_actor': 'worker-1'},
    {**GOOD[2], 'payload_utf8': ''},
    {**GOOD[2], 'event': 'message'},
    {**GOOD[2], 'source_actor': None},
]

class ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        Draft202012Validator.check_schema(SCHEMA)
        cls.validator = Draft202012Validator(SCHEMA)

    def test_accepts_all_variants(self):
        for payload in GOOD:
            with self.subTest(payload=payload):
                self.assertTrue(self.validator.is_valid(payload), list(self.validator.iter_errors(payload)))

    def test_rejects_invalid_and_cross_variant_payloads(self):
        for payload in BAD:
            with self.subTest(payload=payload):
                self.assertFalse(self.validator.is_valid(payload), payload)

    def test_variant_sets_match_across_three_idls(self):
        json_events = set(SCHEMA['properties']['event']['enum'])
        json_arms = {arm['properties']['event']['const'] for arm in SCHEMA['oneOf']}
        tsp_events = set(re.findall(r'event:\s*"([a-z-]+)"\s*;', TSP))
        wit_match = re.search(r'enum\s+event-kind\s*\{([^}]*)\}', WIT)
        self.assertIsNotNone(wit_match)
        wit_events = {part.strip() for part in wit_match.group(1).split(',') if part.strip()}
        self.assertEqual(json_events, {'message', 'failure', 'shutdown'})
        self.assertEqual(json_events, json_arms)
        self.assertEqual(json_events, tsp_events)
        self.assertEqual(json_events, wit_events)

    def test_property_name_projection_to_wit(self):
        wit_record = re.search(r'record\s+actor-envelope\s*\{([^}]*)\}', WIT)
        self.assertIsNotNone(wit_record)
        wit_names = set(re.findall(r'([a-z][a-z0-9-]*):', wit_record.group(1)))
        expected = {name.replace('_', '-') for name in SCHEMA['properties']}
        self.assertEqual(expected, wit_names)

    def test_tsp_declares_every_json_property(self):
        for name in SCHEMA['properties']:
            with self.subTest(name=name):
                self.assertRegex(TSP, r'\b' + name + r'\s*:')

    def test_closed_schema_and_explicit_version(self):
        self.assertEqual(SCHEMA['$schema'], 'https://json-schema.org/draft/2020-12/schema')
        self.assertIs(SCHEMA['additionalProperties'], False)
        self.assertEqual(SCHEMA['properties']['version']['const'], 1)
        self.assertIn('package atto:contracts@1.0.0;', WIT)

if __name__ == '__main__':
    unittest.main()
