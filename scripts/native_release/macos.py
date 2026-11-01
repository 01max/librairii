"""Thin native macOS .app/DMG assembly; public signing is a separate gate."""
import os
import plistlib
import subprocess

from .files import copy_file, copy_tree, fresh_output, write_json
from .identity import verify_embedded
from . import linkage, package


def app_info(release):
    return {'CFBundleExecutable': 'librairii-desktop', 'CFBundleIdentifier': package.BUNDLE_ID,
            'CFBundleName': 'Librairii', 'CFBundleDisplayName': 'Librairii',
            'CFBundlePackageType': 'APPL', 'CFBundleVersion': release['version'],
            'CFBundleShortVersionString': release['version'], 'CFBundleIconFile': 'Librairii.icns',
            'CFBundleDevelopmentRegion': 'en', 'CFBundleLocalizations': ['en', 'fr'],
            'LSMinimumSystemVersion': '15.0', 'LSRequiresNativeExecution': True,
            'NSHighResolutionCapable': True}


def assemble(root, stage, requested_output):
    stage, release, host = package.inputs(root, stage)
    if not release['target'].endswith('apple-darwin'):
        raise ValueError('macOS package requires a macOS native stage')
    destination = fresh_output(root, requested_output)
    image = destination / 'image'
    image.mkdir()
    app = image / 'Librairii.app'
    contents = app / 'Contents'
    (contents / 'MacOS').mkdir(parents=True)
    resources = contents / 'Resources'
    resources.mkdir()
    binary = contents / 'MacOS/librairii-desktop'
    copy_file(stage / 'payload/bin/librairii-desktop', binary, executable=True)
    copy_tree(stage / 'payload/share/librairii', resources / 'librairii')
    with (contents / 'Info.plist').open('xb') as stream:
        plistlib.dump(app_info(release), stream, sort_keys=True)
    (contents / 'PkgInfo').write_bytes(b'APPL????')
    write_json(resources / 'librairii/runtime-requirements.json', package.requirements(release['target']))
    write_json(resources / 'librairii/linkage.json', dict(linkage.inspect(root, binary, release['target']), host=host))
    iconset = destination / 'Librairii.iconset'
    iconset.mkdir()
    with (destination / 'package.log').open('x') as log:
        def run(arguments):
            subprocess.run(arguments, cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)
        for points in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = '@2x' if scale == 2 else ''
                run(['/usr/bin/sips', '-z', str(points*scale), str(points*scale),
                     str(resources / 'librairii/appicon.png'), '--out', str(iconset / f'icon_{points}x{points}{suffix}.png')])
        run(['/usr/bin/iconutil', '--convert', 'icns', '--output', str(resources / 'Librairii.icns'), str(iconset)])
        with (image / 'INSTALL.txt').open('x') as stream:
            stream.write('Librairii ' + release['version'] + ' — ' + release['target'] + '\n\n'
                         'Requires macOS15.0+ on the matching native architecture. Copy Librairii.app\n'
                         'to Applications or another permanent application directory and open it.\n'
                         'The UI, artwork fallbacks and English/French catalogs are embedded.\n'
                         'To update, quit Librairii and replace the application. To uninstall, remove\n'
                         'only Librairii.app. Keep your independent native profile, library, backups\n'
                         'and recovery journals. Reinstall and select the same library to reopen it.\n'
                         'This internal candidate uses an ad-hoc signature. It is NOT Developer ID\n'
                         'signed, notarized, stapled or qualified for public distribution.\n')
        for path in [*image.rglob('*'), image]:
            os.utime(path, (release['source_date_epoch'], release['source_date_epoch']))
        run(['/usr/bin/codesign', '--force', '--sign', '-', '--timestamp=none', str(app)])
        run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(app)])
        verify_embedded(root, binary, release)
        name = 'Librairii-' + release['version'] + '-' + release['target']
        artifact = destination / (name + '-unsigned.dmg')
        run(['/usr/bin/hdiutil', 'create', '-format', 'UDZO', '-imagekey', 'zlib-level=9',
             '-fs', 'HFS+', '-volname', 'Librairii ' + release['version'], '-srcfolder', str(image), str(artifact)])
        run(['/usr/bin/hdiutil', 'verify', str(artifact)])
    return package.finish(root, destination, artifact, image, release, 'dmg',
                          {'state': 'ad-hoc internal candidate', 'developer_id': False,
                           'notarized': False, 'stapled': False})
