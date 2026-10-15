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
