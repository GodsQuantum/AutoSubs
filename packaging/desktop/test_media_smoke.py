from __future__ import annotations

import unittest

from media_smoke import validate_pcm_probe, validate_subtitle_frame


class NativeMediaSmokeTests(unittest.TestCase):
    def test_audio_probe_verifies_pcm_metadata(self) -> None:
        payload = {"streams": [{"codec_type": "audio", "sample_rate": "16000"}], "format": {"duration": "0.800"}}
        validate_pcm_probe(payload)
        payload["streams"][0]["sample_rate"] = "44100"
        with self.assertRaisesRegex(ValueError, "sample"):
            validate_pcm_probe(payload)

    def test_subtitle_frame_requires_rendered_pixels(self) -> None:
        frame = bytearray(320 * 180 * 3)
        with self.assertRaisesRegex(ValueError, "subtitle"):
            validate_subtitle_frame(frame)
        frame[:650] = b"\xff" * 650
        validate_subtitle_frame(frame)


if __name__ == "__main__":
    unittest.main()
