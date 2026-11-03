from __future__ import annotations

import unittest
from pathlib import Path

from fetch_ffmpeg import LOCK, URLS

ROOT = Path(__file__).resolve().parents[2]
RELEASE = "autobuild-2026-10-31-12-59"
ASSET = "ffmpeg-n8.1.3-27-g8dca1586f4-win64-gpl-8.1.zip"
DIGEST = "401dffcf14c301082e1fc03b0bc876b92ea676bfc521fc56e80a1537d137034f"
NSIS_ACTION = "negrutiu/nsis-install@f3339c88dba6fd08910d5275a943f8f746d94876"


class InstallerDependenciesTest(unittest.TestCase):
    def test_windows_ffmpeg_is_immutable_and_locked(self) -> None:
        self.assertEqual(URLS["windows-amd64"], f"https://github.com/BtbN/FFmpeg-Builds/releases/download/{RELEASE}/{ASSET}")
        self.assertEqual(LOCK["windows-amd64"], (ASSET, DIGEST))

    def test_nsis_uses_pinned_action_not_chocolatey(self) -> None:
        pipeline = (ROOT / "packaging/desktop/native_pipeline.py").read_text()
        workflow = (ROOT / ".github/workflows/desktop-installer.yml").read_text()
        self.assertNotIn('run("choco"', pipeline)
        self.assertIn(NSIS_ACTION, workflow)


if __name__ == "__main__":
    unittest.main()
