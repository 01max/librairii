#!/usr/bin/env python3
"""Qualify actual installed native packages on disposable matching CI hosts."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'scripts'))
from native_release.files import copy_file, copy_tree, digest, fresh_output, inventory, regular, write_json
from native_release.identity import clean_source, runtime_host
from packaged_acceptance.run import qualified, run


def retire(cid_file, image_id, name, invocation, force):
    regular(cid_file)
    cid = cid_file.read_text().strip()
    if not re.fullmatch('[0-9a-f]{64}', cid):
        raise ValueError('owned observer creation receipt missing')
    objects = json.loads(subprocess.check_output(['docker', 'inspect', cid], text=True, timeout=10))
    if len(objects) != 1:
        raise ValueError('owned observer not uniquely identified')
    observed = objects[0]
    if (observed['Id'] != cid or observed['Name'] != '/'+name or observed['Image'] != image_id
            or observed['Config']['Labels'].get('librairii.packaged-observer') != invocation
            or (observed['State']['Running'] and not force)):
        raise ValueError('observer ownership/lifetime changed before retirement')
    subprocess.run(['docker', 'rm', *(['--force'] if force else []), cid], check=True, timeout=15)
    return cid


def method_record():
    paths = [*sorted((ROOT/'scripts/native_release').glob('*.py')),
             *sorted((ROOT/'scripts/packaged_acceptance').glob('*.py')),
             ROOT/'scripts/verify-native-packages.py', ROOT/'scripts/native-speech/desktop.py',
             ROOT/'qualification/section-2/native-macos/observer.swift',
             ROOT/'qualification/section-2/check-native-macos.py',
             ROOT/'qualification/section-18/inside.py', ROOT/'qualification/section-18/Dockerfile']
    return {'format':'librairii.packaged-gui-method','schema':1,'source_commit':clean_source(ROOT),
            'files':{path.relative_to(ROOT).as_posix():{'sha256':digest(path),'bytes':regular(path).st_size} for path in paths}}


def context(destination, root, method):
    destination.mkdir()
    checks = destination/'checks'
    checks.mkdir()
    for package in ('native_release', 'packaged_acceptance'):
        selected = checks/package
        selected.mkdir()
        for file in sorted((ROOT/'scripts'/package).glob('*.py')):
            copy_file(file, selected/file.name)
    copy_file(ROOT/'scripts/native-speech/desktop.py', checks/'desktop.py')
    copy_file(ROOT/'qualification/section-18/inside.py', checks/'inside.py')
    copy_file(ROOT/'qualification/section-18/Dockerfile', destination/'Dockerfile')
    write_json(checks/'method.json',method)
    fixtures = destination/'fixtures'
    fixtures.mkdir()
    copy_tree(root/'preservation/corpus/libraries/schema018', fixtures/'legacy-copy')
    # Keep the original expected fixture paths, without exposing a checkout.
    target = fixtures/'preservation/corpus/libraries'
    target.mkdir(parents=True)
    (fixtures/'legacy-copy').rename(target/'schema018')
    for path in ('preservation/corpus/archives/clockwork-forest.zip',
                 'preservation/corpus/archives/clockwork-forest.plain.pk',
                 'crates/librairii-app/tests/fixtures/lunii-catalog.json', 'build/appicon.png'):
        copy_file(root/path, fixtures/path)


def verify_report(path, source, target, expected_artifact, backend, case, session, expected_method):
    report = json.loads(path.read_text())
    if (not qualified(report) or report['source_commit'] != source or report['target'] != target
            or report['artifact'] != expected_artifact or report['backend'] != backend
            or report['case'] != case or report['session'] != session or report['method'] != expected_method):
        raise ValueError('package GUI receipt does not match the exact candidate/lane')
    actual = inventory(path.parent)
    actual.pop(path.name)
    if actual != report['files']:
        raise ValueError('retained GUI files differ from their final report inventory')
    return {'report': str(path), 'sha256': digest(path), 'checks': list(report['checks']),
            'processes': report['processes'], 'passed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inspection', type=Path, required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--expected-source')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('native GUI qualification requires disposable CI; do not run on a personal desktop')
    root = args.source_root.resolve(strict=True)
    source = clean_source(root)
    method = method_record()
    if source != (args.expected_source or os.environ['GITHUB_SHA']):
        raise ValueError('source differs from the actual CI commit')
    target = os.environ['RUST_TARGET']
    host = runtime_host(root, target)
    inspection = json.loads(args.inspection.read_text())
    if inspection['source_commit'] != source or inspection['target'] != target:
        raise ValueError('installed package source/target differs from current qualification')
    output = fresh_output(root, args.output)
    result = {'source_commit': source, 'target': target, 'host': host, 'artifact': inspection['artifact'],
              'method':method,
              'complete': False, 'public_release_qualified': False, 'lanes': [], 'errors': []}
    try:
        if target.endswith('apple-darwin'):
            helper = output/'macos-observer'
            subprocess.run(['xcrun', 'swiftc', '-module-cache-path', str(output/'swift-modules'),
                            str(ROOT/'qualification/section-2/native-macos/observer.swift'), '-o', str(helper)], check=True)
            for case, backend in [('fresh', 'winit-skia'), ('upgraded', 'winit-skia-software')]:
                lane = {'name': case, 'passed': False}
                result['lanes'].append(lane)
                try:
                    run(root, args.inspection, source, target, output/case, case, backend, helper=helper,
                        method=method, observer_source=ROOT)
                    lane.update(verify_report(output/case/'report.json', source, target, inspection['artifact'],
                                              backend, case, 'macOS',method))
                except Exception as error:
                    lane['error'] = str(error)
                    result['errors'].append(case+': '+str(error))
                    break  # No further native input after a failed custody lane.
        else:
            build_context = output/'runtime-context'
            context(build_context, root, method)
            result['runtime_context'] = inventory(build_context)
            image = 'librairii-packaged:'+source
            with (output/'image-build.log').open('x') as log:
                subprocess.run(['docker', 'build', '--build-arg', 'QUALIFICATION_UID='+str(os.getuid()),
                                '--build-arg', 'QUALIFICATION_GID='+str(os.getgid()), '-t', image,
                                str(build_context)], stdout=log, stderr=subprocess.STDOUT, check=True)
            image_id = subprocess.check_output(['docker', 'image', 'inspect', '--format', '{{.Id}}', image], text=True).strip()
            result['image_id'] = image_id
            invocation = uuid.uuid4().hex
            for session in ('x11', 'wayland'):
                for case, backend in [('fresh', 'winit-skia'), ('upgraded', 'winit-skia-software')]:
                    name = session+'-'+case
                    destination = output/name
                    destination.mkdir()
                    cid_file = output/(name+'.container-id')
                    container = 'librairii-packaged-'+invocation+'-'+name
                    lane = {'name': name, 'passed': False}
                    result['lanes'].append(lane)
                    command = ['docker', 'run', '--name', container, '--cidfile', str(cid_file),
                               '--label', 'librairii.packaged-observer='+invocation,
                               '--read-only', '--cap-drop=ALL', '--security-opt=no-new-privileges', '--network=none',
                               '--user', str(os.getuid())+':'+str(os.getgid()),
                               '--memory=4g', '--cpus=2', '--pids-limit=256',
                               '--tmpfs', '/tmp:rw,exec,nosuid,nodev,mode=1777,size=2g',
                               '--mount', 'type=bind,src='+str(args.inspection.resolve().parent)+',dst='+str(args.inspection.resolve().parent)+',readonly',
                               '--mount', 'type=bind,src='+str(destination)+',dst=/work/target/native-release',
                               '--env', 'GITHUB_ACTIONS=true', image_id,
                               '--inspection', str(args.inspection.resolve()), '--source', source,
                               '--case', case, '--backend', backend, '--session', session]
                    timed_out = False
                    try:
                        with (output/(name+'.log')).open('x') as log:
                            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=900)
                        lane.update(verify_report(destination/'case/report.json', source, target, inspection['artifact'],
                                                  backend, case, session,method))
                        clean = json.loads((destination/'clean-host.json').read_text())
                        if not clean['desktop_processes_retired'] or clean['observer_overflow']:
                            raise ValueError('private desktop did not retire cleanly')
                    except Exception as error:
                        timed_out = isinstance(error, subprocess.TimeoutExpired)
                        lane['passed'] = False
                        lane['error'] = str(error)
                        result['errors'].append(name+': '+str(error))
                    finally:
                        if cid_file.is_file():
                            lane['retired_container'] = retire(cid_file, image_id, container, invocation, timed_out)
                        else:
                            raise ValueError('no observer creation receipt; do not retire by name')
        if clean_source(root) != source or method_record() != method:
            raise ValueError('source changed during installed GUI qualification')
        result['complete'] = not result['errors'] and all(lane['passed'] for lane in result['lanes'])
        if not result['complete']:
            raise ValueError('installed package GUI lanes failed: '+str(result['errors']))
    finally:
        write_json(output/'qualification.json', result)
    print(json.dumps({'source': source, 'target': target, 'installed_gui_qualified': result['complete']}))


if __name__ == '__main__':
    main()
