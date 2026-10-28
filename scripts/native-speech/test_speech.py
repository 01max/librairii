"""Reject stale or unrelated Orca focus acknowledgements in retained debug logs."""
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import speech


class ReaderLocusTests(unittest.TestCase):
    target = {'role': 'push button', 'name': 'Move to Shelf'}
    identity = "[push button: 'Move to Shelf']"
    other = "[push button: 'Close']"
    prefix = '23:15:53.709953 - FOCUS MANAGER: '

    def change(self, previous, current, notify=True):
        return self.prefix + f'Changing locus of focus from {previous} to {current}. Notify: {notify}'

    def request(self, identity):
        return self.prefix + f'Request to set locus of focus to {identity} event_manager > focus_manager'

    def test_accepts_only_the_complete_current_identity(self):
        accepted = self.change(self.other, self.identity)
        self.assertEqual(speech.reader_locus_acknowledgement(accepted, self.target), accepted)
        for rejected in (self.change(self.identity, self.other),
                         self.change(self.other, self.identity, False),
                         self.change(self.other, "[check box: 'Move to Shelf']"),
                         self.change(self.other, "[push button: 'Move to Shelf later']")):
            with self.subTest(log=rejected):
                self.assertIsNone(speech.reader_locus_acknowledgement(rejected, self.target))

    def test_existing_locus_requires_the_matching_request(self):
        existing = self.prefix + 'Setting locus of focus to existing locus of focus'
        self.assertIsNone(speech.reader_locus_acknowledgement(existing, self.target))
        self.assertIsNone(speech.reader_locus_acknowledgement(self.request(self.other) + '\n' + existing, self.target))
        accepted = self.request(self.identity) + '\n' + existing
        self.assertEqual(speech.reader_locus_acknowledgement(accepted, self.target), existing)
        for intervening in (self.request(self.other), self.change(self.identity, self.other)):
            with self.subTest(log=intervening):
                self.assertIsNone(speech.reader_locus_acknowledgement(
                    self.request(self.identity) + '\n' + intervening + '\n' + existing, self.target))

    def test_names_with_direction_words_apostrophes_and_wrapped_lines(self):
        target = {'role': 'push button', 'name': "L'objet to conserver\nÉté"}
        identity = "[push button: 'L'objet to conserver\\nÉté']"
        accepted = self.change(self.identity, identity)
        self.assertEqual(speech.reader_locus_acknowledgement(accepted, target), accepted)
        rejected = self.change(identity, self.other)
        self.assertIsNone(speech.reader_locus_acknowledgement(rejected, target))

    def test_later_locus_invalidates_an_earlier_acknowledgement(self):
        accepted = self.change(self.other, self.identity)
        for later in (self.change(self.identity, self.other), self.request(self.other),
                      self.request(self.identity)):
            with self.subTest(log=later):
                self.assertIsNone(speech.reader_locus_acknowledgement(accepted + '\n' + later, self.target))
        self.assertEqual(speech.reader_locus_acknowledgement(
            accepted + '\n' + self.prefix + 'unrelated speech debug', self.target), accepted)

    def test_actual_multiline_name_is_one_complete_log_record(self):
        target = {'role': 'push button', 'name': 'Export · Completed · Oct 27, 2026\n1 / 1 · f0000000'}
        identity = "[push button: '" + target['name'] + "']"
        accepted = self.change(self.other, identity)
        self.assertEqual(speech.reader_locus_acknowledgement(accepted, target), accepted)
        self.assertIsNone(speech.reader_locus_acknowledgement(accepted.split('\n')[0], target))
        self.assertIsNone(speech.reader_locus_acknowledgement(
            accepted + '\n' + self.change(identity, self.other), target))
        existing = self.prefix + 'Setting locus of focus to existing locus of focus'
        self.assertEqual(speech.reader_locus_acknowledgement(
            self.request(identity) + '\n' + existing, target), existing)


if __name__ == '__main__':
    unittest.main()
