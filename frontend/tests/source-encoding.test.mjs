import test from 'node:test';
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import { extname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const sourceRoot = fileURLToPath(new URL('../src/', import.meta.url));
const textExtensions = new Set(['.svelte', '.ts', '.js', '.css']);

async function* sourceFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* sourceFiles(path);
    else if (textExtensions.has(extname(entry.name))) yield path;
  }
}

test('frontend sources contain no Unicode replacement characters', async () => {
  const root = sourceRoot;
  const failures = [];
  for await (const path of sourceFiles(root)) {
    const text = await readFile(path, 'utf8');
    if (text.includes(String.fromCodePoint(0xfffd))) failures.push(relative(root, path));
  }
  assert.deepEqual(failures, []);
});
