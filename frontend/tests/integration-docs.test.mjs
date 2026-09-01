import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const root = new URL('../../', import.meta.url);

test('v3.1.0 integration docs expose the shipped version and lifecycle API', async () => {
  const [cargo, frontend, lock, changelog, readme] = await Promise.all([
    readFile(new URL('Cargo.toml', root), 'utf8'),
    readFile(new URL('frontend/package.json', root), 'utf8'),
    readFile(new URL('frontend/package-lock.json', root), 'utf8'),
    readFile(new URL('CHANGELOG.md', root), 'utf8'),
    readFile(new URL('README.md', root), 'utf8')
  ]);

  assert.match(cargo, /version = "3\.1\.0"/);
  assert.match(frontend, /"version": "3\.1\.0"/);
  assert.match(lock, /"version": "3\.1\.0"/g);
  for (const endpoint of ['/api/v1/fonts', '/api/v1/fonts/css', '/api/v1/jobs/{id}/retranscribe']) {
    assert.ok(readme.includes(endpoint), `missing README endpoint: ${endpoint}`);
  }
  assert.match(readme, /GET\/PUT\/DELETE\s+\/api\/v1\/jobs\/\{id\}/);
  for (const topic of ['fonts', 'word timing', 'French segmentation', 'maxLines', 'Split', 'Merge', 'retranscri', 're-render', 'delete', 'animation', 'black bars']) {
    assert.ok(changelog.toLowerCase().includes(topic.toLowerCase()), `missing changelog topic: ${topic}`);
  }
});
