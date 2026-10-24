"""Static scale claims and summary flags cannot replace actual transitions."""
import copy
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('display_linux', pathlib.Path(__file__).parents[1] / 'check-display-linux.py')
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class DisplayLinux(unittest.TestCase):
    def fixture(self):
        return {'checks': dict.fromkeys(observer.REQUIRED | {'actual_mixed_scale_transitions'}, True),
            'errors': [], 'returncode': 0,
            'sizes': [{'logical_width': w, 'logical_height': h, 'compact': w < 850, 'wide': w >= 1180,
                'contrast_ratios': [7, 5, 4.6]} for w,h in ((560,400), (900,760), (1280,800))],
            'scale_transitions': [{'scale': scale, 'width': 1280 * scale, 'height': 800 * scale,
                'logical_width': 1280, 'logical_height': 800, 'rows': 1000} for scale in (2,1,2,1)]}

    def test_real_sizes_contrast_and_each_scale_transition_are_required(self):
        report = self.fixture()
        self.assertTrue(observer.qualified(report, 'wayland'))
        for defect in ('missing', 'static', 'wrong_pixels', 'wrong_rows', 'low_contrast', 'nonfinite', 'wide', 'failed'):
            changed = copy.deepcopy(report)
            if defect == 'missing': changed['scale_transitions'].pop()
            elif defect == 'static': changed['scale_transitions'][0]['scale'] = 1
            elif defect == 'wrong_pixels': changed['scale_transitions'][0]['width'] = 900
            elif defect == 'wrong_rows': changed['scale_transitions'][0]['rows'] = 0
            elif defect == 'low_contrast': changed['sizes'][0]['contrast_ratios'][0] = 4.4
            elif defect == 'nonfinite': changed['sizes'][0]['contrast_ratios'][0] = float('nan')
            elif defect == 'wide': changed['sizes'][2]['wide'] = False
            else: changed['errors'] = ['window failed']
            with self.subTest(defect=defect):
                self.assertFalse(observer.qualified(changed, 'wayland'))
        for key in report['checks']:
            changed = copy.deepcopy(report)
            del changed['checks'][key]
            self.assertFalse(observer.qualified(changed, 'wayland'))
