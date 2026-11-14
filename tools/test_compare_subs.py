import io
import os
import tempfile
import unittest

from tools.compare_subs import (
    configure_stdout,
    output_path_for_video,
    parse_vtt,
    score_alignment,
)


class CompareSubtitlesTests(unittest.TestCase):
    def test_selects_current_raw_suffix_before_legacy_suffix(self):
        with tempfile.TemporaryDirectory() as output_dir:
            current = os.path.join(output_dir, "episode001.subtitles.zh.vtt")
            legacy = os.path.join(output_dir, "episode001.zh.vtt")
            with open(current, "w", encoding="utf-8"):
                pass
            with open(legacy, "w", encoding="utf-8"):
                pass

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

    def test_parser_skips_empty_cues_and_keeps_following_cues(self):
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", suffix=".vtt", delete=False
        ) as vtt_file:
            vtt_file.write(
                "WEBVTT\n\n"
                "00:00:00.000 --> 00:00:01.000\n\n"
                "00:00:02.000 --> 00:00:03.000\nA\n"
            )
            path = vtt_file.name

        try:
            self.assertEqual(parse_vtt(path), [(2.0, 3.0, "A")])
        finally:
            os.remove(path)

    def test_empty_alignment_and_completely_missing_or_extra_cues(self):
        empty_cue = [(0.0, 1.0, "")]
        empty_score = score_alignment(empty_cue, empty_cue, lambda text: text)
        self.assertEqual(empty_score["conditional_accuracy"], None)
        self.assertEqual(empty_score["overall_cer"], None)
        self.assertEqual(empty_score["exact"], 1)

        missing_score = score_alignment(
            [(0.0, 1.0, "AB")], [], lambda text: text
        )
        self.assertEqual(len(missing_score["missing"]), 1)
        self.assertEqual(missing_score["missing_chars"], 2)
        self.assertEqual(missing_score["overall_cer"], 1.0)

        extra_score = score_alignment(
            [], [(0.0, 1.0, "XYZ")], lambda text: text
        )
        self.assertEqual(len(extra_score["extra"]), 1)
        self.assertEqual(extra_score["extra_chars"], 3)
        self.assertEqual(extra_score["overall_errors"], 3)
        self.assertIsNone(extra_score["overall_cer"])

    def test_stdout_is_reconfigured_to_utf8_for_unicode_episode_table(self):
        output = io.BytesIO()
        stream = io.TextIOWrapper(output, encoding="cp1252")

        configure_stdout(stream)
        stream.write("| Tập | 第001集 |\n")
        stream.flush()

        rendered = output.getvalue().decode("utf-8").replace("\r\n", "\n")
        self.assertEqual(rendered, "| Tập | 第001集 |\n")


if __name__ == "__main__":
    unittest.main()
