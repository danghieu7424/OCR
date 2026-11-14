import os
import tempfile
import unittest

from tools.compare_subs import output_path_for_video, score_alignment


class CompareSubtitlesTests(unittest.TestCase):
    def test_selects_current_raw_suffix_before_legacy_suffix(self):
        with tempfile.TemporaryDirectory() as output_dir:
            current = os.path.join(output_dir, "episode001.subtitles.zh.vtt")
            legacy = os.path.join(output_dir, "episode001.zh.vtt")
            open(current, "w", encoding="utf-8").close()
            open(legacy, "w", encoding="utf-8").close()

            self.assertEqual(
                output_path_for_video("input/episode001.mp4", output_dir),
                current,
            )
            os.remove(current)
            self.assertEqual(
                output_path_for_video("input/episode001.mp4", output_dir),
                legacy,
            )

    def test_overall_cer_counts_missing_and_extra_cue_characters(self):
        reference = [(0.0, 1.0, "AB"), (2.0, 3.0, "C")]
        recognized = [(0.0, 1.0, "AX"), (5.0, 6.0, "Z")]

        score = score_alignment(reference, recognized, lambda text: text)

        self.assertEqual(len(score["missing"]), 1)
        self.assertEqual(len(score["extra"]), 1)
        self.assertEqual(score["conditional_accuracy"], 0.5)
        self.assertEqual(score["overall_errors"], 3)
        self.assertEqual(score["total_ref_chars"], 3)
        self.assertEqual(score["overall_cer"], 1.0)


if __name__ == "__main__":
    unittest.main()
