"""Safe source/admission regressions: no decoder execution or Docker."""
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))
import count_budget_stage1 as stage
spec = importlib.util.spec_from_file_location('source_recipe', HERE / 'decoder-budget/source_recipe.py')
recipe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recipe)


class SourceAdmission(unittest.TestCase):
    def test_exact_derivative_tree_rejects_extra_source_and_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data = b'original source\n'
            (root / 'source.rs').write_bytes(data)
            rows = [{'path': 'source.rs', 'bytes': len(data), 'sha256': recipe.digest(data)}]
            recipe.verify_tree(root, rows)
            (root / 'extra.rs').write_text('never compile this')
            with self.assertRaisesRegex(ValueError, 'extra/missing'):
                recipe.verify_tree(root, rows)
            (root / 'extra.rs').unlink()
            (root / 'source.rs').unlink()
            (root / 'source.rs').symlink_to(root / 'absent')
            with self.assertRaisesRegex(ValueError, 'symlink'):
                recipe.verify_tree(root, rows)

    def test_archive_refuses_extra_link_unsafe_and_changed_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'source.crate'
            expected = b'// preserved header\n'
            row = {'path': 'src/lib.rs', 'bytes': len(expected), 'sha256': recipe.digest(expected),
                   'selected': True, 'first12Lines': expected.decode().splitlines()}
            for kind in ('valid', 'extra', 'link', 'unsafe', 'changed'):
                with self.subTest(kind=kind):
                    with tarfile.open(archive, 'w:gz') as target:
                        info = tarfile.TarInfo('p-1/src/lib.rs')
                        data = b'// modified source\n' if kind == 'changed' else expected
                        info.size = len(data)
                        target.addfile(info, io.BytesIO(data))
                        if kind in ('extra', 'link', 'unsafe'):
                            extra = tarfile.TarInfo('p-1/../escape' if kind == 'unsafe' else 'p-1/src/extra.rs')
                            if kind == 'link':
                                extra.type = tarfile.SYMTYPE
                                extra.linkname = 'lib.rs'
                            target.addfile(extra)
                    manifest = {'crate': {'name': 'p', 'version': '1', 'bytes': archive.stat().st_size,
                                          'sha256': recipe.digest(archive.read_bytes())}, 'members': [row]}
                    if kind == 'valid':
                        self.assertEqual(recipe.authenticate(archive, manifest), {'src/lib.rs': expected})
                    else:
                        with self.assertRaises(ValueError):
                            recipe.authenticate(archive, manifest)


class StageAdmission(unittest.TestCase):
    def test_retained_recipes_materialize_only_three_exact_identities(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixtures = stage.fixtures(root)
            self.assertEqual([f['name'] for f in fixtures], list(stage.IDENTITIES))
            self.assertEqual(sorted(p.name for p in root.iterdir()), sorted(f['file'] for f in fixtures))
            for fixture in fixtures:
                data = (root / fixture['file']).read_bytes()
                self.assertEqual((len(data), hashlib.sha256(data).hexdigest()), stage.IDENTITIES[fixture['name']])

    def test_actual_controls_fail_closed_under_both_optimization_routes(self):
        script = '''import json
from pathlib import Path
from unittest.mock import patch
import count_budget_stage1 as stage
failed = {"exitCode":0,"records":[{"outcome":"smoke_failed"}]}
report = {"controls":[],"cases":[]}
with patch.object(stage.boundary,"contained",return_value=failed) as child, patch.object(stage,"fixtures") as producer:
    error=None
    try: stage.execute(Path("unused"),Path("unused"),report)
    except RuntimeError as failure: error=str(failure)
    print(json.dumps({"optimized":not __debug__,"calls":child.call_count,"mode":child.call_args.args[2],"producer":producer.called,"cases":report["cases"],"error":error}))
'''
        for flags, optimize in ((['-O'], '0'), ([], '1')):
            result = subprocess.run([sys.executable, *flags, '-B', '-c', script], cwd=HERE,
                                    env=dict(os.environ, PYTHONOPTIMIZE=optimize), text=True,
                                    capture_output=True, timeout=10, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            row = json.loads(result.stdout)
            self.assertTrue(row['optimized'])
            self.assertEqual((row['calls'], row['mode']), (1, 'smoke'))
            self.assertFalse(row['producer'])
            self.assertEqual(row['cases'], [])
            self.assertIn('NO allocation probes', row['error'])

    def test_kill_timeout_decoder_error_and_missing_evidence_never_pass(self):
        fixture = {'expectedRefusal': {'field':'num files','count':2000000,'maxCount':10000}}
        limits = f'Max address space {stage.boundary.ADDRESS} {stage.boundary.ADDRESS}\nMax cpu time 2 2'
        observation = {'exitCode':0,'wallWatchdog':False,'oomKilled':False,'dockerStateError':'',
            'stderr':'','memoryEventsAfter':{'oom':0,'oom_kill':0},'cgroupMemoryPeakAfter':4000000,
            'callbackCount':0,'records':[
                {'memoryMax':str(stage.boundary.MEMORY),'swapMax':'0','limits':limits},
                {'stage':'before_constructor','experimentalCountGuards':stage.GUARDS},
                {'outcome':'qualification_count_budget_refused','details':dict(fixture['expectedRefusal'],
                    stage='header_before_target_vector_allocation',maxVectorBytes=8<<20,reason='count_ceiling',
                    callbackCount=0,elementBytes=100,requestedBytes=200000000)}]}
        self.assertEqual(stage.evaluate(fixture, observation)['failures'], [])
        for changes in ({'exitCode':137,'oomKilled':True}, {'wallWatchdog':True},
                        {'memoryEventsAfter':None}, {'callbackCount':1}, {'cgroupMemoryPeakAfter':None}):
            changed = copy.deepcopy(observation)
            changed.update(changes)
            self.assertTrue(stage.evaluate(fixture, changed)['failures'])
        changed = copy.deepcopy(observation)
        changed['records'][-1]['outcome'] = 'library_error'
        self.assertTrue(stage.evaluate(fixture, changed)['failures'])


if __name__ == '__main__':
    unittest.main()
