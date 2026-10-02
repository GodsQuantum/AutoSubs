import test from 'node:test';
import assert from 'node:assert/strict';
import { pathLabel, rootForPath } from '../src/lib/path-picker.js';

test('pathLabel keeps picker shortcuts compact and human readable', () => {
  assert.equal(pathLabel('/library/cloud-arezki/Projets'), 'Projets');
  assert.equal(pathLabel('/library/media'), 'media');
  assert.equal(pathLabel('/'), '/');
});

test('rootForPath selects the most specific allowed root', () => {
  const roots = ['/data', '/library/cloud-arezki', '/library/media'];
  assert.equal(rootForPath('/library/cloud-arezki/Projets/AutoSubs', roots), '/library/cloud-arezki');
  assert.equal(rootForPath('/library/media/Movies', roots), '/library/media');
  assert.equal(rootForPath('/unknown', roots), '');
});
