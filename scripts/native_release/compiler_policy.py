"""Admit only the committed Cargo alias and unmodified compiler inputs."""
import os
from pathlib import Path
import tomllib


def check(root):
    direct = {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC'}
    families = ('RUSTC_', 'CARGO_BUILD_', 'CARGO_TARGET_', 'CARGO_PROFILE_', 'CARGO_UNSTABLE_')
    modifiers = sorted(name for name in os.environ
                       if name in direct or name.startswith(families))
    if modifiers:
        raise ValueError('release refuses ambient compiler configuration: ' + ', '.join(modifiers))

    # Cargo merges configuration from the current directory's ancestors and
    # Cargo home. Checking plain rustc does not attest that merged configuration.
    # Accept the repository's one xtask alias; refuse all other config layers,
    # including apparently harmless ones, before creating any release output.
    root = root.resolve()
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    if not cargo_home.is_absolute():
        cargo_home = root / cargo_home
    folders = [directory / '.cargo' for directory in [root, *root.parents]]
    folders.append(cargo_home)
    committed = root / '.cargo/config.toml'
    for directory in folders:
        for name in ('config', 'config.toml'):
            path = directory / name
            if not path.exists() and not path.is_symlink():
                continue
            if path != committed or path.is_symlink() or directory.is_symlink():
                raise ValueError('release refuses inherited Cargo configuration: ' + str(path))
            if tomllib.loads(path.read_text()) != {'alias': {'xtask': 'run --locked --package xtask --'}}:
                raise ValueError('release requires the committed alias-only Cargo configuration')
