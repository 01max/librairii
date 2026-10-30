"""Protected native Mac signing of an explicitly reviewed unsigned candidate."""
from contextlib import contextmanager
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from .files import copy_tree, digest, fresh_output, regular, write_json
from .identity import clean_source, runtime_host
from . import install, package


def selection(root, directory, source, target, expected_sha256):
    if not re.fullmatch('[0-9a-f]{64}', expected_sha256):
        raise ValueError('an independently selected unsigned SHA-256 is required')
    artifact, release = install.artifact_record(directory, source, target)
    if (not target.endswith('apple-darwin') or clean_source(root) != source
            or digest(artifact) != expected_sha256 or release['signing'].get('developer_id') is not False):
        raise ValueError('signing requires the exact reviewed native unsigned candidate and clean source')
    runtime_host(root, target)
    return release


@contextmanager
def credentials(temporary, environment):
    # Consume secrets before launching any external tool. The only credential
    # file is an encrypted, dedicated keychain supplied through the environment.
    encoded = environment.pop('NATIVE_SIGNING_KEYCHAIN_BASE64', '')
    password = environment.pop('NATIVE_SIGNING_KEYCHAIN_PASSWORD', '')
    if not encoded or not password or len(encoded) > 33554432:
        raise ValueError('missing or oversized protected native Mac signing credentials')
    owner = Path(tempfile.mkdtemp(prefix='librairii-signing-', dir=temporary)).resolve()
    keychain = owner/'signing.keychain-db'
    cleanup = False
    try:
        data = base64.b64decode(encoded, validate=True)
        if not data:
            raise ValueError('empty signing keychain')
        with keychain.open('xb') as stream:
            stream.write(data)
        keychain.chmod(0o600)
        for arguments in (
            ['/usr/bin/security','unlock-keychain','-p',password,str(keychain)],
            ['/usr/bin/security','set-key-partition-list','-S','apple-tool:,apple:','-s','-k',password,str(keychain)],
        ):
            try:
                result = subprocess.run(arguments, capture_output=True, timeout=30)
            except (OSError, subprocess.SubprocessError):
                raise ValueError('dedicated signing keychain tool failed') from None
            if result.returncode:
                # Never retain a tool exception/command that includes a password.
                raise ValueError('dedicated signing keychain could not be unlocked/configured')
        yield keychain
    finally:
        if keychain.exists():
            result = subprocess.run(['/usr/bin/security','delete-keychain',str(keychain)], capture_output=True, timeout=30)
            cleanup = result.returncode == 0 and not keychain.exists()
        else:
            cleanup = True
        if cleanup:
            shutil.rmtree(owner)
        else:
            raise ValueError('dedicated signing keychain retirement uncertain; stop this disposable signing host')


class Tools:
    def __init__(self, output):
        self.output, self.index = output, 0

    def __call__(self, name, arguments, *, timeout=120):
        self.index += 1
        prefix = self.output/(str(self.index).zfill(2)+'-'+name)
        receipt = {'tool':name, 'complete':False}
        try:
            result = subprocess.run([str(value) for value in arguments], capture_output=True, timeout=timeout)
            stdout, stderr = result.stdout, result.stderr
            receipt['returncode'] = result.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr = error.stdout or b'', error.stderr or b''
            receipt['timed_out'] = True
        (prefix.with_suffix('.stdout')).write_bytes(stdout[:4194304])
        (prefix.with_suffix('.stderr')).write_bytes(stderr[:4194304])
        receipt['complete'] = receipt.get('returncode') == 0 and max(len(stdout),len(stderr)) <= 4194304
        write_json(prefix.with_suffix('.json'), receipt)
        if not receipt['complete']:
            raise ValueError('native signing tool failed or timed out: '+name)
        return stdout.decode('utf-8'), stderr.decode('utf-8')


def signature(path, team, certificate, output, tools, *, application):
    tools('codesign-verify', ['/usr/bin/codesign','--verify','--deep','--strict',path])
    stdout, stderr = tools('codesign-display', ['/usr/bin/codesign','--display','--verbose=4',path])
    display = stdout+stderr
    identifier = package.BUNDLE_ID if application else package.BUNDLE_ID+'.dmg'
    if (not re.search(r'^Authority=Developer ID Application: .+$', display, re.M)
            or not re.search(r'^TeamIdentifier='+re.escape(team)+r'$', display, re.M)
            or not re.search(r'^Identifier='+re.escape(identifier)+r'$', display, re.M)
            or not re.search(r'^Timestamp=.+$', display, re.M)
            or (application and not re.search(r'flags=0x[0-9a-f]+\([^\n]*\bruntime\b', display))):
        raise ValueError('signature lacks the selected Developer ID, timestamp, identity or hardened runtime')
    prefix = output/('app-cert-' if application else 'dmg-cert-')
    tools('codesign-certificate', ['/usr/bin/codesign','--display','--extract-certificates',prefix,path])
    leaf = Path(str(prefix)+'0')
    regular(leaf)
    if hashlib.sha1(leaf.read_bytes()).hexdigest().upper() != certificate:
        raise ValueError('signature leaf certificate differs from the independently selected identity')


def notarize(path, keychain, profile, tools, name, *, staple=None):
    auth = ['--keychain-profile',profile,'--keychain',keychain]
    stdout, _ = tools(name+'-submit', ['/usr/bin/xcrun','notarytool','submit',path,*auth,
                      '--wait','--timeout','20m','--output-format','json','--no-progress'], timeout=1260)
    result = json.loads(stdout)
    if not re.fullmatch('[0-9a-fA-F-]{36}', result.get('id','')):
        raise ValueError('notary response lacks a retained submission identity')
    tools(name+'-log', ['/usr/bin/xcrun','notarytool','log',result['id'],*auth], timeout=120)
    if result.get('status') != 'Accepted':
        raise ValueError('Apple did not accept this exact notarization submission')
    ticket = path if staple is None else staple
    tools(name+'-staple', ['/usr/bin/xcrun','stapler','staple',ticket])
    tools(name+'-ticket', ['/usr/bin/xcrun','stapler','validate',ticket])
    return result


def verify_final(artifact, app, team, certificate, output):
    if not re.fullmatch('[A-Z0-9]{10}', team) or not re.fullmatch('[0-9A-F]{40}', certificate):
        raise ValueError('independently selected team and certificate fingerprints are required')
    tools = Tools(output)
    signature(app, team, certificate, output, tools, application=True)
    signature(artifact, team, certificate, output, tools, application=False)
    for name, path in [('app',app),('dmg',artifact)]:
        tools(name+'-ticket', ['/usr/bin/xcrun','stapler','validate',path])
    tools('gatekeeper-app', ['/usr/sbin/spctl','--assess','--type','execute','--verbose=4',app])
    tools('gatekeeper-dmg', ['/usr/sbin/spctl','--assess','--type','open','--context','context:primary-signature','--verbose=4',artifact])
    return {'developer_id':True,'team_id':team,'certificate_sha1':certificate,
            'app_and_dmg_tickets_validated':True,'gatekeeper_assessments_passed':True}


def sign(root, directory, source, target, expected_sha256, requested_output, keychain, profile, team, certificate):
    release = selection(root, directory, source, target, expected_sha256)
    destination = fresh_output(root, requested_output)
    inspected = install.inspect(root, directory, source, target, destination/'unsigned-inspection')
    image = destination/'image'
    copy_tree(Path(inspected['installed']), image)
    app = image/'Librairii.app'
    install_text = image/'INSTALL.txt'
    text = install_text.read_text()
    warning = ('This internal candidate uses an ad-hoc signature. It is NOT Developer ID\n'
               'signed, notarized, stapled or qualified for public distribution.\n')
    if warning not in text:
        raise ValueError('unsigned install instructions differ from the reviewed candidate format')
    install_text.write_text(text.replace(warning, 'This application and disk image carry Developer ID signatures and stapled\n'
                                        'notarization tickets. Installed GUI qualification is recorded separately.\n'))
    logs = destination/'signing'
    logs.mkdir()
    tools = Tools(logs)
    tools('sign-app', ['/usr/bin/codesign','--force','--sign',certificate,'--keychain',keychain,
                      '--options','runtime','--timestamp',app])
    signature(app, team, certificate, logs, tools, application=True)
    archive = destination/'notary-app.zip'
    tools('archive-app', ['/usr/bin/ditto','-c','-k','--keepParent',app,archive])
    # ZIPs cannot carry a stapled ticket; staple and validate the actual app.
    application_notary = notarize(archive, keychain, profile, tools, 'app', staple=app)
    artifact = destination/('Librairii-'+release['version']+'-'+target+'-signed.dmg')
    tools('create-dmg', ['/usr/bin/hdiutil','create','-format','UDZO','-imagekey','zlib-level=9','-fs','HFS+',
                        '-volname','Librairii '+release['version'],'-srcfolder',image,artifact])
    tools('sign-dmg', ['/usr/bin/codesign','--sign',certificate,'--keychain',keychain,'--timestamp',
                      '--identifier',package.BUNDLE_ID+'.dmg',artifact])
    disk_notary = notarize(artifact, keychain, profile, tools, 'dmg')
    tools('verify-dmg', ['/usr/bin/hdiutil','verify',artifact])
    verification = destination/'signature-verification'
    verification.mkdir()
    evidence = verify_final(artifact, app, team, certificate, verification)
    return package.finish(root, destination, artifact, image, release, 'dmg',
        dict(evidence, state='Developer ID signed, notarized and stapled; installed qualification pending',
             notarized=True, stapled=True, unsigned_sha256=expected_sha256,
             app_notarization=application_notary, dmg_notarization=disk_notary))
