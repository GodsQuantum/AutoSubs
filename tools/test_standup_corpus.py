"""Deterministic sidecar output and search regression tests (no network/FFmpeg)."""
import tempfile
import unittest
from pathlib import Path
import standup_corpus as corpus

WORDS = [
    {"word":"Bonjour,", "start":0.15,"end":0.52},
    {"word":"je", "start":0.53,"end":0.65},
    {"word":"parle", "start":0.67,"end":0.96},
    {"word":"de", "start":1.0,"end":1.12},
    {"word":"politique.", "start":1.15,"end":1.80},
    {"word":"Merci", "start":3.20,"end":3.6},
    {"word":"à", "start":3.62,"end":3.75},
    {"word":"vous.", "start":3.77,"end":4.1},
]

class CorpusTests(unittest.TestCase):
    def test_500_stt_fallback_only_accepts_silero_confirmed_silence(self):
        from unittest.mock import patch
        with patch.object(corpus, 'multipart', side_effect=[
            RuntimeError(f'{corpus.STT}: HTTP 500: Internal Server Error'), []]) as call:
            self.assertIsNone(corpus.transcribe_text_or_silence(b'valid-WAV', 'fr'))
            self.assertEqual(call.call_count, 2)
            self.assertTrue(call.call_args.args[3].endswith('/v1/audio/speech/timestamps'))

    def test_500_stt_with_detected_speech_must_not_be_silently_discarded(self):
        from unittest.mock import patch
        with patch.object(corpus, 'multipart', side_effect=[
            RuntimeError(f'{corpus.STT}: HTTP 500: Internal Server Error'),
            [{'start':1000,'end':2000}] ]):
            with self.assertRaisesRegex(RuntimeError, 'HTTP 500'):
                corpus.transcribe_text_or_silence(b'valid-WAV', 'fr')

    def test_transient_500_retries_without_discarding_audio(self):
        from unittest.mock import patch
        with patch.object(corpus, 'get_words', side_effect=[
            RuntimeError('HTTP 500'), [{'word':'bonjour','start':0,'end':.1}]]) as call:
            with patch.object(corpus.time,'sleep') as sleep:
                assert len(corpus.get_words_retry('video',0,90,'fr'))==1
                self.assertEqual(call.call_count,2)
                self.assertEqual(sleep.call_count,1)

    def test_one_transient_http_503_retries_same_audio_chunk(self):
        from unittest.mock import patch
        word={"word":"Bonjour", "start":0.0, "end":0.5}
        with patch.object(corpus, "get_words", side_effect=[RuntimeError("HTTP 503: temporary"),[word]]) as getter:
            with patch.object(corpus.time, "sleep") as snooze:
                result=corpus.get_words_retry("movie", 0, 90, "fr")
                self.assertEqual(result,[word])
                self.assertEqual(getter.call_count,2)
                self.assertEqual(snooze.call_count,1)

    def test_chunk_checkpoint_identity_includes_duration_and_chunk_size(self):
        # A chunk-size change must not silently reuse cached 240 s timing on 90 s slices.
        video=corpus.ROOT / "Divers" / "capture.mp4"
        first=corpus.chunk_identity(video, 100, 123456, 240)
        second=corpus.chunk_identity(video, 100, 123456, 90)
        assert first != second
        assert first == corpus.chunk_identity(video, 100, 123456, 240)

    def test_stt_is_transcribed_without_duplicate_word_alignment(self):
        options=corpus.transcription_options("fr")
        self.assertEqual(options["response_format"], "verbose_json")
        self.assertEqual(options["language"],"fr")
        self.assertNotIn("timestamp_granularities[]", options)

    def test_word_timestamp_and_srt_format(self):
        srt=corpus.srt_output(WORDS)
        self.assertIn("00:00:00,150 --> 00:00:01,800",srt)
        self.assertIn("politique.",srt)
        self.assertIn("00:00:03,200 --> 00:00:04,100",srt)
        self.assertTrue(srt.startswith("1\n"))

    def test_markdown_preserves_verbatim_text(self):
        video=corpus.ROOT/"Paname Comedy Club"/"test.mp4"
        markdown=corpus.markdown_output(video,WORDS,60,"fr")
        self.assertIn("# test",markdown)
        self.assertIn("**Horodatages**",markdown)
        self.assertIn("Bonjour, je parle de politique. Merci à vous.",markdown)
        self.assertIn("### [00:00:00.150]",markdown)

    def test_database_full_text_search_and_overwrite_protection(self):
        with tempfile.TemporaryDirectory() as td:
            prev_db=corpus.DB
            corpus.DB=Path(td)/"index.sqlite3"
            try:
                db=corpus.db_open()
                with db:
                    db.execute("INSERT INTO text_index (path,start,end,text) VALUES(?,?,?,?)",
                               ("Paname/test.mp4",0.15,4.1,"Bonjour, je parle de politique."))
                rows=db.execute("SELECT path,start FROM text_index WHERE text_index MATCH 'politique'").fetchall()
                self.assertEqual(rows,[("Paname/test.mp4",0.15)])
                sidecar=Path(td)/"test.srt"
                corpus.atomic_text(sidecar,"content")
                with self.assertRaises(FileExistsError): corpus.atomic_text(sidecar,"do not override")
                self.assertEqual(sidecar.read_text(),"content")
                db.close()
            finally: corpus.DB=prev_db

if __name__=="__main__":
    unittest.main()
