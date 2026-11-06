"""Compile actual observer declarations with inert, module-local AX fixtures."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

source = Path(__file__).with_name('observer.swift')
fixture = source.parent/'attribute-tests/main.swift'
output = Path(sys.argv[1])
output.mkdir()  # Refuse existing/unowned output; never remove a supplied path.
marker = '// NATIVE_OBSERVER_ENTRY_POINT: headless fixtures compile declarations above only.'
source_bytes, fixture_bytes = source.read_bytes(), fixture.read_bytes()
source_text = source_bytes.decode('utf-8')
prefix, found, _ = source_text.partition(marker)
if not found or source_text.count(marker) != 1:
    raise ValueError('unique native observer entry point required')
main = output/'main.swift'
main.write_bytes(prefix.encode('utf-8') + b'\n' + fixture_bytes)
binary = output/'attribute-fixtures'
subprocess.run(['xcrun', 'swiftc', '-module-cache-path', str(output/'modules'),
                str(main), '-o', str(binary)], check=True, timeout=120)
subprocess.run([str(binary.resolve())], check=True, timeout=10)
if source.read_bytes() != source_bytes or fixture.read_bytes() != fixture_bytes:
    raise ValueError('headless AX fixture inputs changed during execution')
(output/'receipt.json').write_text(json.dumps({
    'format': 'librairii.headless-ax-attribute-fixtures', 'schema': 1,
    'observer_sha256': hashlib.sha256(source_bytes).hexdigest(),
    'fixture_sha256': hashlib.sha256(fixture_bytes).hexdigest(),
    'actual_accessibility_reads': 0, 'native_input': False, 'passed': True,
}, indent=2) + '\n')
