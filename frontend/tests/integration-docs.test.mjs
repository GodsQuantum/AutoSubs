import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const root = new URL('../../', import.meta.url);

test('v3.5.0 integration docs expose the shipped version, AppImage and lifecycle API', async () => {
  const [cargo, frontend, lock, changelog, readme] = await Promise.all([
    readFile(new URL('Cargo.toml', root), 'utf8'),
    readFile(new URL('frontend/package.json', root), 'utf8'),
    readFile(new URL('frontend/package-lock.json', root), 'utf8'),
    readFile(new URL('CHANGELOG.md', root), 'utf8'),
    readFile(new URL('README.md', root), 'utf8')
  ]);

  assert.match(cargo, /version = "3\.5\.0"/);
  assert.match(frontend, /"version": "3\.5\.0"/);
  assert.match(lock, /"version": "3\.5\.0"/g);
  for (const topic of [
    'AppImage',
    '--background',
    'XDG_CONFIG_HOME',
    'AUTOSUBS_USE_SYSTEM_MEDIA_TOOLS',
    'folder workflows'
  ]) {
    assert.ok(readme.includes(topic), `missing AppImage README topic: ${topic}`);
  }
  for (const endpoint of [
    '/api/v1/fonts',
    '/api/v1/fonts/css',
    '/api/v1/preview/frame',
    '/api/v1/jobs/{id}/render-options',
    '/api/v1/jobs/{id}/retranscribe'
  ]) {
    assert.ok(readme.includes(endpoint), `missing README endpoint: ${endpoint}`);
  }
  assert.match(readme, /GET\/PUT\/DELETE\s+\/api\/v1\/jobs\/\{id\}/);
  for (const topic of ['fonts', 'word timing', 'French segmentation', 'maxLines', 'Split', 'Merge', 'retranscri', 're-render', 'delete', 'animation', 'black bars']) {
    assert.ok(changelog.toLowerCase().includes(topic.toLowerCase()), `missing changelog topic: ${topic}`);
  }
});

test('native AppImage workflow builds and attaches both Linux architectures', async () => {
  const [workflow, release, builder] = await Promise.all([
    readFile(new URL('.github/workflows/appimage.yml', root), 'utf8'),
    readFile(new URL('.github/workflows/release.yml', root), 'utf8'),
    readFile(new URL('packaging/appimage/build.sh', root), 'utf8')
  ]);

  for (const arch of ['x86_64', 'aarch64']) {
    assert.ok(workflow.includes(`arch: ${arch}`), `missing AppImage architecture: ${arch}`);
  }
  assert.match(workflow, /ubuntu-24\.04-arm/);
  assert.match(workflow, /packaging\/appimage\/build\.sh/);
  assert.match(workflow, /ffmpegReady/);
  assert.match(workflow, /libass/);
  assert.match(workflow, /\bfile\b/);
  assert.match(workflow, /actions\/upload-artifact/);
  assert.match(workflow, /AUTOSUBS_APPIMAGE_OPEN_BROWSER/);
  assert.match(workflow, /APPIMAGE_EXTRACT_AND_RUN/);

  assert.match(release, /uses:\s+\.\/\.github\/workflows\/appimage\.yml/);
  assert.match(release, /actions\/download-artifact/);
  assert.match(release, /gh release upload/);
  assert.match(release, /\.AppImage/);
  assert.match(release, /\.sha256/);
  assert.doesNotMatch(release, /TAG_SHA=.*gh api.*\|\| true/);
  assert.match(release, /if TAG_SHA="\$\(gh api/);
  assert.match(release, /else\s*\n\s*TAG_SHA=""/);

  assert.match(builder, /linuxdeploy/);
  assert.match(builder, /appimagetool/);
});
