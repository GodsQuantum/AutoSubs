"""Test safe recommendations without relying on host-specific values."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location("auto_tune", Path(__file__).with_name("auto-tune.py"))
assert spec is not None and spec.loader is not None
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


class TestRecommendations(unittest.TestCase):
    def test_tiny_host_is_serial(self) -> None:
        for app in ("autosubs", "scribewatch"):
            profile = mod.recommend(app, 2, 3)
            self.assertEqual(profile["recommended_environment"][f"{app.upper()}_MAX_TRANSCRIPTION_JOBS"], "1")

    def test_cloud9_host_stays_bounded(self) -> None:
        profile = mod.recommend("autosubs", 16, 32)
        self.assertEqual(profile["recommended_environment"]["AUTOSUBS_MAX_RENDER_JOBS"], "2")
        self.assertEqual(profile["recommended_environment"]["AUTOSUBS_MAX_TRANSCRIPTION_JOBS"], "2")

    def test_scribewatch_ffmpeg_is_sane(self) -> None:
        profile = mod.recommend("scribewatch", 16, 32)
        self.assertEqual(profile["recommended_environment"]["SCRIBEWATCH_FFMPEG_THREADS"], "1")

    def test_unknown_app_rejected(self) -> None:
        with self.assertRaises(ValueError):
            mod.recommend("unknown", 16, 32)


if __name__ == "__main__":
    unittest.main()
