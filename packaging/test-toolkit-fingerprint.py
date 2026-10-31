#!/usr/bin/env python3
"""Fingerprint workspace inputs without generated output or ancestor repositories."""
import hashlib
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
excluded = {'.git', 'target', 'tmp', '.dart_tool', 'build', 'node_modules', '__pycache__'}
inputs = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml',
          '.cargo', 'vendor'}


def allowed(path):
    parts = Path(path).parts
    if not parts or excluded.intersection(parts):
        return False
    if parts[0] in inputs:
        return True
    if parts[0] == 'flutter_app':
        return len(parts) == 1 or path == 'flutter_app/fonts/D2Coding.ttf' or path == 'flutter_app/fonts'
    if parts[0].startswith('upeg-'):
        return len(parts) == 1 or parts[1] in {'Cargo.toml', 'build.rs', 'src'}
    return False


def archive_paths():
    for top in root.iterdir():
        if not allowed(top.name):
            continue
        if top.is_dir() and not top.is_symlink():
            for directory, dirs, files in os.walk(top, followlinks=False):
                dirs[:] = sorted(name for name in dirs if allowed(str((Path(directory) / name).relative_to(root))))
                for name in sorted(files + [name for name in dirs if (Path(directory) / name).is_symlink()]):
                    relative = str((Path(directory) / name).relative_to(root))
                    if allowed(relative):
                        yield os.fsencode(relative)
        else:
            yield os.fsencode(top.name)


if (root / '.git').exists():
    paths = subprocess.check_output(['git', 'ls-files', '-co', '--exclude-standard', '-z'], cwd=root).split(b'\0')
    paths = [name for name in paths if name and allowed(os.fsdecode(name))]
else:
    paths = list(archive_paths())
digest = hashlib.sha256()
for name in sorted(set(paths)):
    path = root / os.fsdecode(name)
    digest.update(name + b'\0')
    if path.is_symlink():
        digest.update(b'link\0' + os.fsencode(path.readlink()))
    elif path.is_file():
        with path.open('rb') as file:
            for chunk in iter(lambda: file.read(1024 * 1024), b''):
                digest.update(chunk)
    else:
        digest.update(b'missing')
    digest.update(b'\0')
print(digest.hexdigest())
