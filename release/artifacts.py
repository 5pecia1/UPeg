#!/usr/bin/env python3
"""Canonical release asset profiles and the v2 checksum manifest.

``build_commit`` records the commit that produced the bytes. A mirrored
release's public tag target is bound separately by github_release.py.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

MIRROR_SCHEMA = 'upeg-release-manifest/v2'
PUBLIC_REPO = '5pecia1/UPeg'
COMPONENTS = ('cli', 'desktop', 'web')
SHA_PATTERN = re.compile(r'[0-9a-f]{40}\Z')
DIGEST_PATTERN = re.compile(r'[0-9a-f]{64}\Z')


class GateError(RuntimeError):
    """A failed check; callers must stop before publishing."""


def safe_name(name: object) -> str:
    if not isinstance(name, str) or not name or name in ('.', '..'):
        raise GateError('invalid release asset name')
    if '/' in name or '\\' in name or '\x00' in name:
        raise GateError(f'unsafe release asset name: {name!r}')
    return name


def expected_files(version: str, tag: str, components: list[str]) -> list[str]:
    files: list[str] = []
    for component in components:
        if component == 'cli':
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin'):
                files += [f'upeg-{tag}-{target}.tar.gz',
                          f'upeg-{tag}-{target}.tar.gz.sha256']
            files += [f'upeg-{tag}-x86_64-pc-windows-msvc.zip',
                      f'upeg-{tag}-x86_64-pc-windows-msvc.zip.sha256']
        elif component == 'desktop':
            files += [f'upeg_{version}_amd64.deb', f'upeg_{version}_amd64.deb.sha256',
                      f'upeg-{tag}-x86_64.AppImage', f'upeg-{tag}-x86_64.AppImage.sha256']
        elif component == 'web':
            files += [f'upeg-{tag}-web.tar.gz', f'upeg-{tag}-web.tar.gz.sha256']
        else:
            raise GateError(f'unknown component: {component}')
    return sorted(files)


def profile_files(version: str, tag: str, profile: str) -> set[str]:
    if profile == 'public':
        return set(expected_files(version, tag, list(COMPONENTS)))
    if profile == 'linux':
        payloads = {
            f'upeg-{tag}-x86_64-unknown-linux-gnu.tar.gz',
            f'upeg-{tag}-aarch64-unknown-linux-gnu.tar.gz',
            f'upeg-{tag}-web.tar.gz', f'upeg_{version}_amd64.deb',
            f'upeg-{tag}-x86_64.AppImage',
        }
    elif profile == 'full':
        payloads = {
            f'upeg-{tag}-x86_64-unknown-linux-gnu.tar.gz',
            f'upeg-{tag}-aarch64-unknown-linux-gnu.tar.gz',
            f'upeg-{tag}-aarch64-apple-darwin.tar.gz',
            f'upeg-{tag}-x86_64-pc-windows-msvc.zip',
            f'upeg_{version}_amd64.deb', f'upeg-{tag}-x86_64.AppImage',
            f'upeg-{tag}-web.tar.gz', f'upeg-{tag}.dmg',
            f'upeg-{tag}-x86_64.msix',
        }
    else:
        raise GateError(f'unknown release profile: {profile!r}')
    return payloads | {f'{name}.sha256' for name in payloads}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b''):
            digest.update(chunk)
    return digest.hexdigest()


def regular_files(dist_dir: Path) -> dict[str, Path]:
    if not dist_dir.is_dir() or dist_dir.is_symlink():
        raise GateError('release asset directory is missing or is a symlink')
    files: dict[str, Path] = {}
    for path in dist_dir.iterdir():
        safe_name(path.name)
        if path.is_symlink() or not path.is_file():
            raise GateError(f'non-regular release asset: {path.name}')
        files[path.name] = path
    return files


def manifest_v2(dist_dir: Path, out_json: Path, tag: str, commit: str, profile: str) -> dict:
    version = tag.removeprefix('v')
    if not tag.startswith('v') or not version or not SHA_PATTERN.fullmatch(commit):
        raise GateError('v2 manifest needs a v-prefixed tag and full build commit SHA')
    files = regular_files(dist_dir)
    if set(files) != profile_files(version, tag, profile):
        raise GateError('release files differ from the selected profile')
    entries = [{'name': name, 'sha256': sha256(files[name]), 'bytes': files[name].stat().st_size}
               for name in sorted(files)]
    document = {'schema': MIRROR_SCHEMA, 'tag': tag, 'version': version,
                'profile': profile, 'build_commit': commit, 'files': entries}
    out_json.write_text(json.dumps(document, indent=2) + '\n')
    return document


def verify_mirror(dist_dir: Path, manifest_json: Path, tag: str) -> dict:
    document = json.loads(manifest_json.read_text())
    if not isinstance(document, dict) or document.get('schema') != MIRROR_SCHEMA:
        raise GateError('unexpected release manifest schema')
    if document.get('tag') != tag or document.get('version') != tag.removeprefix('v'):
        raise GateError('release manifest tag/version mismatch')
    if not isinstance(document.get('build_commit'), str) \
            or not SHA_PATTERN.fullmatch(document['build_commit']):
        raise GateError('release manifest build_commit is invalid')
    entries = document.get('files')
    if not isinstance(entries, list) or not entries:
        raise GateError('release manifest lists no files')
    names: list[str] = []
    for entry in entries:
        if not isinstance(entry, dict):
            raise GateError('invalid release manifest file entry')
        names.append(safe_name(entry.get('name')))
    if len(names) != len(set(names)):
        raise GateError('duplicate release manifest file name')
    if set(names) != profile_files(document['version'], tag, document.get('profile')):
        raise GateError('release manifest files differ from selected profile')
    actual = regular_files(dist_dir)
    if set(actual) != set(names):
        raise GateError('release asset set mismatch')
    for entry in entries:
        name = entry['name']
        size, checksum = entry.get('bytes'), entry.get('sha256')
        if type(size) is not int or size < 0 or not isinstance(checksum, str) \
                or not DIGEST_PATTERN.fullmatch(checksum) \
                or actual[name].stat().st_size != size or sha256(actual[name]) != checksum:
            raise GateError(f'release artifact content mismatch: {name}')
    return document


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    exp = sub.add_parser('expected-files')
    exp.add_argument('version')
    exp.add_argument('tag')
    exp.add_argument('components', nargs='*', default=list(COMPONENTS))
    man = sub.add_parser('manifest-v2')
    man.add_argument('dist_dir', type=Path)
    man.add_argument('out_json', type=Path)
    man.add_argument('--tag', required=True)
    man.add_argument('--commit', required=True)
    man.add_argument('--profile', required=True, choices=('linux', 'public', 'full'))
    chk = sub.add_parser('verify-mirror')
    chk.add_argument('dist_dir', type=Path)
    chk.add_argument('manifest_json', type=Path)
    chk.add_argument('--tag', required=True)
    args = parser.parse_args()
    try:
        if args.command == 'expected-files':
            print('\n'.join(expected_files(args.version, args.tag, args.components or list(COMPONENTS))))
        elif args.command == 'manifest-v2':
            document = manifest_v2(args.dist_dir, args.out_json, args.tag, args.commit, args.profile)
            print(f"[PASS] release manifest: {len(document['files'])} files, tag {document['tag']}")
        else:
            document = verify_mirror(args.dist_dir, args.manifest_json, args.tag)
            print(f"[PASS] release assets verified: {len(document['files'])} files, tag {document['tag']}")
    except (GateError, OSError, ValueError, KeyError, TypeError) as error:
        print(f'[FAIL] {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
