// Host-only static descriptor admission; not guest Zorde execution or provenance proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {descriptorFromSchema} from '../../scripts/zorde-static-contract-ir.mjs';

test('int32 signed endpoints preserve exact type and integer bounds', () => {
  const d = descriptorFromSchema({type: 'integer', format: 'int32',
    minimum: -2147483648, maximum: 2147483647,
    $schema: 'https://json-schema.org/draft/2020-12/schema', title: 'bounded'});
  assert.deepEqual(d, {version: 1, kind: 2, property: 1,
    minimum: -2147483648, maximum: 2147483647, excluded: []});
});

test('booleans, fractional bounds and unsupported schema extras are denied', () => {
  for (const pair of [[true, 1], [0, false], [0.5, 2], [2, 1], [-2147483649, 0]]) {
    assert.throws(() => descriptorFromSchema({type: 'integer',
      minimum: pair[0], maximum: pair[1]}), /ZORDE_STATIC_REFUSED/);
  }
  for (const extra of [{default: 0}, {multipleOf: 2}, {format: 'int64'},
                       {$schema: 'http://json-schema.org/draft-07/schema#'}]) {
    assert.throws(() => descriptorFromSchema({type: 'integer', minimum: 0,
      maximum: 1, ...extra}), /ZORDE_STATIC_REFUSED/);
  }
});

test('string code-point length refuses implicit or noninteger bounds', () => {
  assert.deepEqual(descriptorFromSchema({type: 'string', minLength: 0, maxLength: 65536}),
    {version: 1, kind: 3, property: 2, minimum: 0, maximum: 65536, excluded: []});
  for (const pair of [[false, 3], [0, 1.25], [-1, 1], [0, 65537], [3, 2]]) {
    assert.throws(() => descriptorFromSchema({type: 'string',
      minLength: pair[0], maxLength: pair[1]}), /ZORDE_STATIC_REFUSED/);
  }
  assert.throws(() => descriptorFromSchema(Object.create({type: 'integer',
    minimum: 0, maximum: 1})), /ZORDE_STATIC_REFUSED/);
});
