"""Signing admission, notarization and credential lifetime without Apple calls."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_release import signing, package


class Signing(unittest.TestCase):
    def test_signing_requires_external_candidate_digest_and_clean_matching_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary)/'candidate.dmg'
            artifact.write_bytes(b'unsigned bytes')
            release = {'signing':{'developer_id':False}}
            with patch.object(signing.install,'artifact_record',return_value=(artifact,release)), \
                 patch.object(signing,'clean_source',return_value='a'*40), \
                 patch.object(signing,'runtime_host') as host:
                args = [Path(temporary),Path(temporary),'a'*40,'aarch64-apple-darwin',signing.digest(artifact)]
                self.assertEqual(signing.selection(*args), release)
                for position,value in [(2,'b'*40),(3,'x86_64-unknown-linux-gnu'),(4,'c'*64),(4,'')]:
                    changed = list(args)
                    changed[position] = value
                    with self.assertRaises(ValueError):
                        signing.selection(*changed)
                release['signing']['developer_id'] = True
                with self.assertRaises(ValueError):
                    signing.selection(*args)
                self.assertEqual(host.call_count, 1)

    def test_notarization_staples_actual_app_only_after_accepted_submission_and_retained_log(self):
        identity = '00112233-4455-4677-8899-aabbccddeeff'
        for status in ('Accepted','Invalid','In Progress'):
            tools = Mock(return_value=(json.dumps({'id':identity,'status':status}),''))
            if status == 'Accepted':
                signing.notarize(Path('app.zip'),Path('isolated.keychain'),'profile',tools,'app',staple=Path('Librairii.app'))
                self.assertEqual([call.args[0] for call in tools.call_args_list],
                                 ['app-submit','app-log','app-staple','app-ticket'])
                self.assertEqual(tools.call_args_list[2].args[1][-1],Path('Librairii.app'))
            else:
                with self.assertRaises(ValueError):
                    signing.notarize(Path('app.zip'),Path('isolated.keychain'),'profile',tools,'app',staple=Path('Librairii.app'))
                self.assertEqual([call.args[0] for call in tools.call_args_list],['app-submit','app-log'])

    def test_signature_requires_external_team_certificate_timestamp_and_hardened_runtime(self):
        leaf = b'synthetic public certificate'
        fingerprint = hashlib.sha1(leaf).hexdigest().upper()
        valid = ('Identifier='+package.BUNDLE_ID+'\nAuthority=Developer ID Application: Fixture (ABCDEFGHIJ)\n'
                 'TeamIdentifier=ABCDEFGHIJ\nTimestamp=Oct 30, 2026\nCodeDirectory flags=0x10000(runtime)\n')
        for change in (None,'team','certificate','timestamp','runtime','identifier','authority'):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary)
                display = valid
                if change in ('team','identifier','authority'):
                    key = {'team':'TeamIdentifier','identifier':'Identifier','authority':'Authority'}[change]
                    display = '\n'.join('invalid='+line if line.startswith(key+'=') else line for line in valid.splitlines())+'\n'
                if change == 'timestamp':
                    display = display.replace('Timestamp=','Unsigned=')
                if change == 'runtime':
                    display = display.replace('runtime','adhoc')
                def tools(name, arguments, **_):
                    if name == 'codesign-display':
                        return '',display
                    if name == 'codesign-certificate':
                        Path(str(arguments[-2])+'0').write_bytes(leaf)
                    return '',''
                args = [Path('Librairii.app'),'ABCDEFGHIJ',fingerprint,output,tools]
                if change == 'certificate':
                    args[2] = 'F'*40
                if change is None:
                    signing.signature(*args,application=True)
                else:
                    with self.assertRaises(ValueError):
                        signing.signature(*args,application=True)

    def test_dedicated_keychain_secrets_are_consumed_and_retired_on_success_or_failure(self):
        for failure in (False,True,'tool-timeout'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                environment = {'NATIVE_SIGNING_KEYCHAIN_BASE64':base64.b64encode(b'encrypted synthetic fixture').decode(),
                               'NATIVE_SIGNING_KEYCHAIN_PASSWORD':'never-retained-password'}
                calls = []
                def execute(arguments, **_):
                    calls.append(arguments)
                    if arguments[1] == 'delete-keychain':
                        Path(arguments[-1]).unlink()
                    elif failure == 'tool-timeout':
                        raise subprocess.TimeoutExpired(arguments,30)
                    return subprocess.CompletedProcess(arguments,0,b'',b'')
                def use():
                    with patch.object(signing.subprocess,'run',side_effect=execute):
                        with signing.credentials(temporary,environment) as keychain:
                            self.assertEqual(keychain.stat().st_mode & 0o777,0o600)
                            self.assertEqual(environment,{})
                            if failure:
                                raise ValueError('synthetic sign failure')
                if failure:
                    with self.assertRaises(ValueError) as error:
                        use()
                    self.assertNotIn('never-retained-password',str(error.exception))
                else:
                    use()
                self.assertEqual(list(Path(temporary).iterdir()),[])
                self.assertEqual(environment,{})
                self.assertEqual(calls[-1][1],'delete-keychain')


if __name__ == '__main__':
    unittest.main()
