"""Inspect native executable dependencies and minimum supported OS versions."""
import re

from .identity import architecture, output


def linux_report(dynamic, versions, interpreter, loaded):
    needed = re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', dynamic)
    allowed = {'libgcc_s.so.1', 'libstdc++.so.6', 'libm.so.6', 'libc.so.6', 'ld-linux-x86-64.so.2',
               'libpthread.so.0', 'libdl.so.2', 'librt.so.1'}
    if not needed or set(needed) - allowed:
        raise ValueError('undeclared Linux dynamic dependencies: ' + str(needed))
    required = sorted({tuple(map(int, value.split('.'))) for value in re.findall(r'\bGLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)\b', versions)})
    if not required or max(required) > (2, 39):
        raise ValueError('executable exceeds the Ubuntu24.04 glibc2.39 baseline')
    if '[Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]' not in interpreter:
        raise ValueError('undeclared Linux ELF interpreter')
    if 'not found' in loaded:
        raise ValueError('missing Linux runtime dependency')
    for line in loaded.splitlines():
        paths = re.findall(r'(?:=>\s*)?(/[^\s(]+)', line)
        if any(not path.startswith(('/lib/', '/lib64/', '/usr/lib/', '/usr/lib64/')) for path in paths):
            raise ValueError('Linux runtime dependency escapes system library paths')
    return {'needed': needed, 'maximum_glibc': '.'.join(map(str, max(required))),
            'elf_interpreter': '/lib64/ld-linux-x86-64.so.2',
            'dynamic': dynamic, 'versions': versions, 'program_headers': interpreter, 'loaded': loaded}


def macos_report(libraries, commands):
    dependencies = [line.strip().split(' (', 1)[0] for line in libraries.splitlines()[1:] if line.strip()]
    if not dependencies or any(not name.startswith(('/usr/lib/', '/System/Library/Frameworks/'))
                               or any(word in name.lower() for word in ('webkit', 'javascriptcore', 'webview'))
                               for name in dependencies):
        raise ValueError('undeclared or browser macOS runtime dependency')
    minimum = re.findall(r'^\s*minos\s+([0-9.]+)\s*$', commands, re.M)
    if minimum != ['15.0']:
        raise ValueError('Mach-O must declare exactly the macOS15.0 deployment target')
    return {'dependencies': dependencies, 'minimum_os': '15.0',
            'libraries': libraries, 'load_commands': commands}


def inspect(root, binary, target):
    architecture(binary, target)
    if target.endswith('linux-gnu'):
        return linux_report(*(output(root, command) for command in [
            ['readelf', '-d', str(binary)], ['readelf', '--version-info', str(binary)],
            ['readelf', '-l', str(binary)], ['ldd', str(binary)]]))
    return macos_report(output(root, ['/usr/bin/otool', '-L', str(binary)]),
                        output(root, ['/usr/bin/otool', '-l', str(binary)]))
