import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeSpeachesUrl, probeSpeaches, probeSpeachesViaApp } from '../src/lib/speaches-discovery.js';

test('keeps localhost/private LAN HTTP and refuses public HTTP and credentials', () => {
  assert.equal(normalizeSpeachesUrl('http://127.0.0.1:8005/v1'), 'http://127.0.0.1:8005');
  assert.equal(normalizeSpeachesUrl('http://192.0.2.42:8005'), 'http://192.0.2.42:8005');
  assert.equal(normalizeSpeachesUrl('https://speaches.example.test'), 'https://speaches.example.test');
  assert.throws(() => normalizeSpeachesUrl('http://example.com'), /HTTPS/);
  assert.throws(() => normalizeSpeachesUrl('http://user:secret@127.0.0.1:8005'), /credentials/);
  assert.throws(() => normalizeSpeachesUrl('file:///tmp/test'), /HTTP/);
});

test('discovers native word-timestamp Whisper Turbo without forced aligner', async () => {
  const calls = [];
  const fake = async (url) => {
    calls.push(url);
    let data;
    if (url === 'http://127.0.0.1:8005/health') data = {message:'OK'};
    else if (url.endsWith('/v1/models')) data = {data:[{id:'deepdml/faster-whisper-large-v3-turbo-ct2'}]};
    else data = {status:'ok',backend:'vulkan',backend_selection:'configured_not_hardware_probed',
      models:[{id:'fastconformer-fr-q4',available:true}]};
    return {ok:true,json: async()=>data};
  };
  const result = await probeSpeaches('http://127.0.0.1:8005', fake);
  assert.equal(result.model,'deepdml/faster-whisper-large-v3-turbo-ct2');
  assert.equal(result.alignmentUrl,'');
  assert.equal(result.backend,'whisper-native-words');
  assert.equal(result.backendVerified,false);
  assert.equal(calls.length,2);
});

test('rejects missing Whisper model before changing settings', async () => {
  const fake = async (url) => ({ok:true,json:async()=>url.endsWith('/v1/models')?{data:[]}:{message:'OK'}});
  await assert.rejects(() => probeSpeaches('http://127.0.0.1:8005',fake),/Whisper Turbo/);
});

test('one-click connection performs same-origin authenticated-app-server probe only', async () => {
  const calls = [];
  const fake = async (url, options) => {
    calls.push({ url, options });
    return { ok: true, json: async () => ({
      origin: 'http://192.0.2.42:8005',
      transcriptionUrl: 'http://192.0.2.42:8005/v1/audio/transcriptions',
      alignmentUrl: '',
      model: 'deepdml/faster-whisper-large-v3-turbo-ct2',
      alignmentModel: '', backend: 'whisper-native-words', backendVerified: false,
    }) };
  };
  const result = await probeSpeachesViaApp('http://192.0.2.42:8005', fake);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, '/api/v1/speaches/probe');
  assert.equal(calls[0].options.method, 'POST');
  assert.deepEqual(JSON.parse(calls[0].options.body), {url:'http://192.0.2.42:8005'});
  assert.equal(result.model, 'deepdml/faster-whisper-large-v3-turbo-ct2');
});

test('one-click connection surfaces same-origin upstream errors without modifying settings', async () => {
  const fake = async () => ({ok: false, status: 400, json: async () => ({
    error: {message: 'Native word timestamps unavailable'}
  })});
  await assert.rejects(
    () => probeSpeachesViaApp('http://192.0.2.42:8005', fake),
    /Native word timestamps/
  );
});
