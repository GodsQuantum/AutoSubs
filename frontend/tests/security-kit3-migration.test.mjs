import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';

test('security migration uses supported SvelteKit 3 in Vite config', () => {
  const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  const major = Number(pkg.devDependencies['@sveltejs/kit'].replace(/^[^0-9]*/, '').split('.')[0]);
  assert.ok(major >= 3, 'SvelteKit 2 has security advisories fixed in 3');
  assert.ok(Number(pkg.devDependencies['@sveltejs/adapter-static'].split('.')[0]) >= 4);
  assert.equal(existsSync(new URL('../svelte.config.js', import.meta.url)), false);
  const vite = readFileSync(new URL('../vite.config.ts', import.meta.url), 'utf8');
  assert.match(vite, /sveltekit\s*\(\s*\{/);
  assert.match(vite, /adapter:\s*adapter\(/);
});
