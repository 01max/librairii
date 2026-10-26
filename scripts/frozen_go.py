"""Materialize a private Go module solely from admitted, verified source bytes."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import tempfile


@contextmanager
def frozen_module(root, prefix):
    tmp = root / "tmp"
    tmp.mkdir(exist_ok=True)
    manifest_bytes = (root / "preservation/oracle-source.json").read_bytes()
    with tempfile.TemporaryDirectory(prefix=prefix, dir=tmp) as work:
        work = Path(work)
        for entry in json.loads(manifest_bytes)["files"]:
            path = entry["path"]
            if path.startswith("internal/") or path in ("go.mod", "go.sum"):
                body = (root / path).read_bytes()
                if hashlib.sha256(body).hexdigest() != entry["sha256"]:
                    raise ValueError(f"frozen source changed: {path}")
                target = work / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(body)
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith("GO")}
        environment.update(GOWORK="off", GOENV="off", GOFLAGS="", GOTOOLCHAIN="local")
        yield work, environment, manifest_bytes
