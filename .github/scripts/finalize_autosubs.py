from pathlib import Path

def replace(path, old, new, expected=None):
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if expected is not None and count != expected:
        raise SystemExit(f"{path}: expected {expected} occurrence(s), found {count}: {old!r}")
    if count == 0:
        raise SystemExit(f"{path}: pattern not found: {old!r}")
    p.write_text(text.replace(old, new))

# Frontend typing fix proven by the previous GitHub run.
Path("frontend/src/lib/pairing.js").write_text("""\
/** @typedef {{ name: string }} NamedFile */

const subtitleExtensions = new Set(['srt', 'ass', 'ssa', 'json']);

/** @param {string} name @returns {string} */
const ext = (name) => name.includes('.') ? name.split('.').pop()?.toLowerCase() ?? '' : '';

/** @param {string} name @returns {string} */
const stem = (name) => name.replace(/\\.[^.]+$/, '');

/** @param {string} name @returns {string} */
const lower = (name) => name.toLowerCase();

/** @param {string} name @returns {boolean} */
const isInternalStaging = (name) => {
  const value = lower(name);
  return ext(value) === 'partial' || value.includes('.partial-') || value.endsWith('.uploading') || value.endsWith('_words.json');
};

/**
 * Pair media candidates with basename-matching subtitle sidecars.
 * @param {NamedFile[]} files
 * @returns {Array<{ video: NamedFile, sidecar: NamedFile | undefined }>}
 */
export function pairFiles(files) {
  const candidates = files.filter((file) => !isInternalStaging(file.name));
  /** @type {Map<string, NamedFile>} */
  const sidecars = new Map();
  for (const file of candidates) {
    if (subtitleExtensions.has(ext(file.name))) sidecars.set(stem(file.name).toLowerCase(), file);
  }
  return candidates
    .filter((file) => !subtitleExtensions.has(ext(file.name)))
    .map((video) => ({ video, sidecar: sidecars.get(stem(video.name).toLowerCase()) }));
}
""")

# Five exact Rust failures from Actions job 98615769064.
replace(
    "src/api/media.rs",
    "HeaderMap, HeaderValue, Response, StatusCode, header",
    "HeaderMap, Response, StatusCode, header",
    1,
)

replace(
    "src/jobs.rs",
    "Asset, Brand, EncoderKind, FormatKey, Job, JobStatus, Preset, RawWord, SubtitleLine,",
    "Asset, Brand, EncoderKind, Job, JobStatus, Preset, RawWord, SubtitleLine,",
    1,
)

replace(
    "src/workflows.rs",
    "use uuid::Uuid;\n",
    "",
    1,
)

replace(
    "src/api/settings.rs",
    "let mut models = value",
    "let mut models: Vec<String> = value",
    1,
)

replace(
    "src/jobs.rs",
    "let mut published = Vec::new();",
    "let mut published: Vec<PathBuf> = Vec::new();",
    1,
)

# HTML/Svelte correctness already proven by warnings.
replace(
    "frontend/src/lib/views/SettingsView.svelte",
    "<option value={m}/>",
    "<option value={m}></option>",
)

replace(
    "frontend/src/lib/components/PathPicker.svelte",
    '<section class="modal" role="dialog" aria-modal="true" aria-label={title || $dictionary.filePicker}>',
    '<div class="modal" role="dialog" aria-modal="true" aria-label={title || $dictionary.filePicker}>',
    1,
)
replace(
    "frontend/src/lib/components/PathPicker.svelte",
    "</section>",
    "</div>",
    1,
)

# Generic self-hosted example instead of a deployment-specific hostname.
replace(
    "frontend/src/lib/views/SettingsView.svelte",
    "http://speaches:8005/v1/audio/transcriptions",
    "http://transcriber:8000/v1/audio/transcriptions",
)

# Asset MIME hardening:
# never trust multipart Content-Type supplied by a browser/client.
replace(
    "src/api/assets.rs",
    'let name = field.file_name().unwrap_or("asset.bin").to_owned(); let mime = field.content_type().unwrap_or("application/octet-stream").to_owned();',
    'let name = field.file_name().unwrap_or("asset.bin").to_owned(); let mime = mime_guess::from_path(&name).first_or_octet_stream().to_string();',
    1,
)

replace(
    "src/api/assets.rs",
    '.header(header::CACHE_CONTROL, "private, max-age=3600")',
    '.header(header::CACHE_CONTROL, "private, max-age=3600")\n        .header("X-Content-Type-Options", "nosniff")',
    1,
)

# Generalise known infrastructure examples wherever present.
for path in [
    ".env.example",
    "compose.example.yaml",
    "README.md",
    "README.fr.md",
]:
    p = Path(path)
    if not p.exists():
        continue
    text = p.read_text()
    text = text.replace("Europe/Paris", "UTC")
    text = text.replace("/mnt/NAS_SAL", "/srv/media")
    text = text.replace("/mnt/NAS", "/srv/media")
    text = text.replace("speaches:8005", "transcriber:8000")
    p.write_text(text)

# --------------------------------------------------
# Rust fixes proven by strict Clippy v2.
# --------------------------------------------------

# regex::Regex verbose mode (?x) treats '#' as a
# comment introducer. This compact expression needs
# only case-insensitive mode.
replace(
    "src/subtitle/segment.rs",
    'r"(?xi)^(?:https?://\\S+',
    'r"(?i)^(?:https?://\\S+',
    1,
)

# Give the dynamic-programming memo a semantic name.
replace(
    "src/subtitle/segment.rs",
    "static ABBREVIATION_RE: OnceLock<Regex> = OnceLock::new();\n",
    "static ABBREVIATION_RE: OnceLock<Regex> = OnceLock::new();\n"
    "\n"
    "type LayoutMemo = "
    "HashMap<(usize, usize), Option<(f64, Vec<usize>)>>;\n",
    1,
)

replace(
    "src/subtitle/segment.rs",
    "memo: &mut HashMap<(usize, usize), "
    "Option<(f64, Vec<usize>)>>",
    "memo: &mut LayoutMemo",
    1,
)

# build_render_plan is an FFmpeg boundary whose
# independent immutable inputs are intentionally
# explicit. Keep the API explicit and document the
# single local Clippy exception rather than disabling
# the lint globally.
replace(
    "src/media/render.rs",
    "pub fn build_render_plan(\n",
    "#[expect(\n"
    "    clippy::too_many_arguments,\n"
    "    reason = "
    "\"render-plan construction keeps independent "
    "FFmpeg resources explicit\"\n"
    ")]\n"
    "pub fn build_render_plan(\n",
    1,
)

# --------------------------------------------------
# Rust fixes proven by strict Clippy v3.
# --------------------------------------------------

# Stable descending chronological ordering without
# unnecessary_sort_by.
replace(
    "src/api/jobs.rs",
    "jobs.sort_by(|a,b| b.created_at_ms.cmp(&a.created_at_ms));",
    "jobs.sort_by_key(|job| std::cmp::Reverse(job.created_at_ms));",
    1,
)

# Initialize changed Default fields directly.
replace(
    "src/state.rs",
    'let mut base = Preset::default(); base.name = "Default".into(); base.migrate();',
    'let mut base = Preset { name: "Default".into(), ..Preset::default() }; base.migrate();',
    1,
)

# Render tests: construct the requested format directly.
replace(
    "src/media/render.rs",
    "let mut preset = Preset::default();\n"
    "        preset.format = FormatProfile::default();",
    "let preset = Preset::default();",
    1,
)

replace(
    "src/media/render.rs",
    "let mut preset = Preset::default();\n"
    "        preset.format = FormatProfile { key: FormatKey::Portrait916, fit: FitMode::Cover, width: None, height: None };",
    "let preset = Preset {\n"
    "            format: FormatProfile { key: FormatKey::Portrait916, fit: FitMode::Cover, width: None, height: None },\n"
    "            ..Preset::default()\n"
    "        };",
    1,
)

replace(
    "src/media/render.rs",
    "let mut preset = Preset::default();\n"
    "        preset.format = FormatProfile { key: FormatKey::Square11, fit: FitMode::Preserve, width: None, height: None };",
    "let preset = Preset {\n"
    "            format: FormatProfile { key: FormatKey::Square11, fit: FitMode::Preserve, width: None, height: None },\n"
    "            ..Preset::default()\n"
    "        };",
    1,
)

# ASS tests: same direct-initialization rule.
replace(
    "src/subtitle/ass.rs",
    "let mut preset = Preset::default();\n"
    "        preset.format = FormatProfile { key: FormatKey::Source, fit: FitMode::Preserve, width: None, height: None };",
    "let preset = Preset {\n"
    "            format: FormatProfile { key: FormatKey::Source, fit: FitMode::Preserve, width: None, height: None },\n"
    "            ..Preset::default()\n"
    "        };",
    1,
)

replace(
    "src/subtitle/ass.rs",
    "let mut preset = Preset::default();\n"
    "        preset.format = FormatProfile { key: FormatKey::Square11, fit: FitMode::Cover, width: None, height: None };",
    "let preset = Preset {\n"
    "            format: FormatProfile { key: FormatKey::Square11, fit: FitMode::Cover, width: None, height: None },\n"
    "            ..Preset::default()\n"
    "        };",
    1,
)

# --------------------------------------------------
# Frontend accessibility/security fixes proven by
# final quality gate run 33111620084.
# --------------------------------------------------

import json
import re

# Latest stable SvelteKit still resolves cookie 0.6.x.
# Force the patched, API-compatible pre-1.0 line.
package_path = Path("frontend/package.json")
package = json.loads(package_path.read_text())
overrides = package.get("overrides", {})
if not isinstance(overrides, dict):
    raise SystemExit("frontend/package.json: overrides must be an object")
overrides["cookie"] = "0.7.2"
package["overrides"] = overrides
package_path.write_text(
    json.dumps(package, indent=2, ensure_ascii=False) + "\n"
)

# Brand defaults are rendered inside an #each loop, so
# use a dynamic id rather than duplicating a static id.
replace(
    "frontend/src/lib/views/BrandsView.svelte",
    '<div class="field"><label>{formatLabel(key)}</label><select class="select"',
    '<div class="field"><label for={`brand-default-${key}`}>{formatLabel(key)}</label><select id={`brand-default-${key}`} class="select"',
    1,
)

# Associate every remaining plain field label with its
# first form control. Labels which wrap checkboxes use
# `class="check"` and are already semantically valid.
def associate_plain_labels(file_path, prefix):
    p = Path(file_path)
    value = p.read_text()
    counter = 0

    pattern = re.compile(
        r'<label>(?P<body>.*?)</label>'
        r'(?P<between>(?:(?!</div>|<label).)*?)'
        r'(?P<tag><(?:input|select|textarea)\b)',
        re.DOTALL,
    )

    def repl(match):
        nonlocal counter
        counter += 1
        control_id = f"{prefix}-field-{counter}"
        return (
            f'<label for="{control_id}">'
            f'{match.group("body")}</label>'
            f'{match.group("between")}'
            f'{match.group("tag")} id="{control_id}"'
        )

    value, count = pattern.subn(repl, value)

    if count == 0:
        raise SystemExit(
            f"{file_path}: no unassociated plain labels found"
        )

    p.write_text(value)
    print(f"{file_path}: associated {count} labels")

for file_path, prefix in [
    ("frontend/src/lib/views/BrandsView.svelte", "brands"),
    ("frontend/src/lib/views/EditorView.svelte", "editor"),
    ("frontend/src/lib/views/PresetsView.svelte", "presets"),
    ("frontend/src/lib/views/QueueView.svelte", "queue"),
    ("frontend/src/lib/views/SettingsView.svelte", "settings"),
    ("frontend/src/lib/views/WorkflowsView.svelte", "workflows"),
]:
    associate_plain_labels(file_path, prefix)

# API-key fields have two controls: the label above is
# associated with the action selector; explicitly name
# the password input as well.
replace(
    "frontend/src/lib/views/SettingsView.svelte",
    '<input class="input" type="password" disabled={transAction!==\'replace\'} bind:value={transKey}/>',
    '<input class="input" type="password" aria-label={$dictionary.apiKey} disabled={transAction!==\'replace\'} bind:value={transKey}/>',
    1,
)

replace(
    "frontend/src/lib/views/SettingsView.svelte",
    '<input class="input" type="password" disabled={localAction!==\'replace\'} bind:value={localKey}/>',
    '<input class="input" type="password" aria-label={$dictionary.apiKey} disabled={localAction!==\'replace\'} bind:value={localKey}/>',
    1,
)

replace(
    "frontend/src/lib/views/SettingsView.svelte",
    '<input class="input" type="password" disabled={llmAction!==\'replace\'} bind:value={llmKey}/>',
    '<input class="input" type="password" aria-label={$dictionary.apiKey} disabled={llmAction!==\'replace\'} bind:value={llmKey}/>',
    1,
)

# Real WebVTT generation for the editor preview rather
# than silencing Svelte's media-caption warning.
Path("frontend/src/lib/captions.js").write_text(r"""
/**
 * @typedef {{ start: number, end: number, text: string }} CaptionLine
 */

/** @param {number} seconds */
function vttTime(seconds) {
  const total = Math.max(0, Math.round(Number(seconds || 0) * 1000));
  const hours = Math.floor(total / 3600000);
  const minutes = Math.floor((total % 3600000) / 60000);
  const secs = Math.floor((total % 60000) / 1000);
  const millis = total % 1000;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${String(millis).padStart(3, '0')}`;
}

/**
 * Build a browser-native WebVTT track for editor preview.
 * @param {CaptionLine[]} lines
 */
export function subtitlesToVtt(lines) {
  const cues = lines.map((line, index) => {
    const end = Math.max(Number(line.end || 0), Number(line.start || 0) + 0.001);
    const text = String(line.text ?? '').replace(/\r/g, '');
    return `${index + 1}\n${vttTime(line.start)} --> ${vttTime(end)}\n${text}\n`;
  }).join('\n');

  return `WEBVTT\n\n${cues}`;
}
""")

Path("frontend/tests/captions.test.mjs").write_text(r"""
import test from 'node:test';
import assert from 'node:assert/strict';
import { subtitlesToVtt } from '../src/lib/captions.js';

test('subtitlesToVtt emits valid WebVTT timing and text', () => {
  const vtt = subtitlesToVtt([
    { start: 1.25, end: 2.5, text: 'Bonjour monde' }
  ]);

  assert.match(vtt, /^WEBVTT/);
  assert.match(
    vtt,
    /00:00:01\.250 --> 00:00:02\.500/
  );
  assert.match(vtt, /Bonjour monde/);
});
""")

replace(
    "frontend/src/lib/views/EditorView.svelte",
    "  import { dictionary } from '$lib/i18n';\n",
    "  import { dictionary } from '$lib/i18n';\n"
    "  import { subtitlesToVtt } from '$lib/captions.js';\n",
    1,
)

replace(
    "frontend/src/lib/views/EditorView.svelte",
    "  let fileInput:HTMLInputElement;\n",
    "  let fileInput:HTMLInputElement;\n"
    "  let captionTrackUrl='';\n",
    1,
)

replace(
    "frontend/src/lib/views/EditorView.svelte",
    "  $: currentPreset = presets.find(p=>p.id===selectedPreset);\n",
    "  $: currentPreset = presets.find(p=>p.id===selectedPreset);\n"
    "  $: captionTrackUrl = "
    "`data:text/vtt;charset=utf-8,${encodeURIComponent(subtitlesToVtt(lines))}`;\n",
    1,
)

replace(
    "frontend/src/lib/views/EditorView.svelte",
    '<video id="autosubs-editor-video" src={videoUrl(job.id)} controls preload="metadata" playsinline on:timeupdate={(e)=>currentTime=(e.currentTarget as HTMLVideoElement).currentTime}></video>',
    '<video id="autosubs-editor-video" src={videoUrl(job.id)} controls preload="metadata" playsinline on:timeupdate={(e)=>currentTime=(e.currentTarget as HTMLVideoElement).currentTime}><track kind="captions" src={captionTrackUrl} srclang="und" label={$dictionary.subtitles}></video>',
    1,
)

# Permanent CI must enforce the same quality gates as
# this one-shot finalizer.
replace(
    ".github/workflows/ci.yml",
    "      - run: npm ci\n"
    "      - run: npm test\n"
    "      - run: npm run check\n"
    "      - run: npm run build\n",
    "      - run: npm ci\n"
    "      - run: npm audit --audit-level=low\n"
    "      - run: npm test\n"
    "      - run: npm run check -- --fail-on-warnings --output machine-verbose\n"
    "      - run: npm run build\n",
    1,
)

# Regression test for API/static routing semantics.
ci_path = Path(".github/workflows/ci.yml")
ci_text = ci_path.read_text()

ci_old = """          echo "$HEALTH" | grep -q '"libass":true'
"""

ci_new = ci_old + """          INDEX_STATUS="$(docker exec autosubs-ci sh -lc 'curl -sS -o /tmp/autosubs-index -w "%{http_code}" http://127.0.0.1:3000/')"
          test "$INDEX_STATUS" = "200"
          docker exec autosubs-ci grep -qi 'autosubs' /tmp/autosubs-index

          API_404_STATUS="$(docker exec autosubs-ci sh -lc 'curl -sS -o /tmp/autosubs-api-404 -w "%{http_code}" http://127.0.0.1:3000/api/v1/definitely-not-a-route')"
          test "$API_404_STATUS" = "404"

          if docker exec autosubs-ci grep -qi '<!doctype html' /tmp/autosubs-api-404; then
            echo "Unknown API route incorrectly fell through to SPA"
            false
          fi
"""

if ci_text.count(ci_old) != 1:
    raise SystemExit(
        ".github/workflows/ci.yml: runtime smoke marker mismatch"
    )

ci_path.write_text(
    ci_text.replace(ci_old, ci_new, 1)
)

# --------------------------------------------------
# Canonical CI must validate direct pushes to main.
# --------------------------------------------------

replace(
    ".github/workflows/ci.yml",
    "on:\n"
    "  pull_request:\n"
    "    branches: [main]\n"
    "  workflow_dispatch:\n",
    "on:\n"
    "  push:\n"
    "    branches: [main]\n"
    "  pull_request:\n"
    "    branches: [main]\n"
    "  workflow_dispatch:\n",
    1,
)

# --------------------------------------------------
# Generic public documentation.
# --------------------------------------------------

import re

for path in [
    ".env.example",
    "compose.example.yaml",
    "README.md",
    "README.fr.md",
    "docs/screenshot-queue.svg",
]:
    p = Path(path)
    if not p.exists():
        continue

    value = p.read_text()

    # Deployment-specific NAS conventions -> generic.
    value = re.sub(
        r"/mnt/NAS(?:_[A-Za-z0-9_.-]+)?",
        "/srv/media",
        value,
    )

    value = value.replace(
        "Europe/Paris",
        "UTC",
    )

    value = re.sub(
        r"http://speaches:[0-9]+",
        "http://transcriber:8000",
        value,
    )

    value = value.replace(
        "/home/.../autosubs",
        "/srv/autosubs",
    )

    value = value.replace(
        "Existing homelab deployment",
        "NAS / external media deployment",
    )

    value = value.replace(
        "Déploiement homelab existant",
        "Déploiement NAS / stockage externe",
    )

    p.write_text(value)
