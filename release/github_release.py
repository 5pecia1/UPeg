#!/usr/bin/env python3
"""Stage and finalize a checksummed public GitHub release by release ID.

The only permitted destination is the public repository. The caller supplies a
v2 manifest and the immutable public commit separately: build_commit records
the build source, while --commit is the public tag target.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
from urllib.error import HTTPError
from urllib.parse import quote, urlencode, urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener

from artifacts import GateError, PUBLIC_REPO, safe_name, verify_mirror

API = 'https://api.github.com'
SHA = re.compile(r'[0-9a-f]{40}\Z')
SHA256 = re.compile(r'[0-9a-f]{64}\Z')


class APIError(GateError):
    def __init__(self, status: int, body: bytes):
        self.status = status
        super().__init__(f'GitHub API returned HTTP {status}: {body[:300].decode("utf-8", "replace")}')


class PrivateRedirect(HTTPRedirectHandler):
    """Do not send the API token to GitHub's signed asset storage URL."""

    def redirect_request(self, request, fp, code, msg, headers, newurl):
        redirected = super().redirect_request(request, fp, code, msg, headers, newurl)
        if redirected and urlparse(newurl).netloc != urlparse(request.full_url).netloc:
            redirected.remove_header('Authorization')
            redirected.unredirected_hdrs.pop('Authorization', None)
        return redirected


class Client:
    def __init__(self, token: str):
        if not token:
            raise GateError('GH_TOKEN is required')
        self.token = token
        self.opener = build_opener(PrivateRedirect())

    def request(self, method: str, url: str, data: bytes | None = None,
                accept: str = 'application/vnd.github+json',
                content_type: str | None = None) -> bytes:
        headers = {'Accept': accept, 'Authorization': f'Bearer {self.token}',
                   'X-GitHub-Api-Version': '2022-11-28'}
        if data is not None:
            headers['Content-Type'] = content_type or 'application/json'
        request = Request(url, data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=120) as response:
                return response.read()
        except HTTPError as error:
            raise APIError(error.code, error.read()) from error

    def json(self, method: str, url: str, payload: object | None = None) -> object:
        data = None if payload is None else json.dumps(payload).encode()
        return json.loads(self.request(method, url, data))


def repo_url(repo: str) -> str:
    if repo != PUBLIC_REPO:
        raise GateError(f'release destination must be {PUBLIC_REPO}')
    return f'{API}/repos/{repo}'


def releases(client: Client, base: str) -> list[dict]:
    found: list[dict] = []
    for page in range(1, 101):
        batch = client.json('GET', f'{base}/releases?per_page=100&page={page}')
        if not isinstance(batch, list):
            raise GateError('invalid release listing')
        found.extend(batch)
        if len(batch) < 100:
            return found
    raise GateError('release listing exceeded 100 pages')


def assets(client: Client, base: str, release_id: int) -> list[dict]:
    found: list[dict] = []
    for page in range(1, 101):
        batch = client.json('GET', f'{base}/releases/{release_id}/assets?per_page=100&page={page}')
        if not isinstance(batch, list):
            raise GateError('invalid release asset listing')
        found.extend(batch)
        if len(batch) < 100:
            return found
    raise GateError('release asset listing exceeded 100 pages')


def release_by_id(client: Client, base: str, release_id: int) -> dict:
    value = client.json('GET', f'{base}/releases/{release_id}')
    if not isinstance(value, dict) or value.get('id') != release_id:
        raise GateError('release ID mismatch')
    return value


def bound_release(release: dict, release_id: int, tag: str, commit: str) -> None:
    if (release.get('id') != release_id or release.get('tag_name') != tag
            or release.get('target_commitish') != commit):
        raise GateError('release ID, tag, or target commit mismatch')


def tag_target(client: Client, base: str, tag: str) -> str | None:
    try:
        ref = client.json('GET', f'{base}/git/ref/tags/{quote(tag, safe="")}')
    except APIError as error:
        if error.status == 404:
            return None
        raise
    obj = ref.get('object', {})
    for _ in range(8):
        if obj.get('type') == 'commit' and SHA.fullmatch(obj.get('sha', '')):
            return obj['sha']
        if obj.get('type') != 'tag' or not SHA.fullmatch(obj.get('sha', '')):
            break
        annotated = client.json('GET', f'{base}/git/tags/{obj["sha"]}')
        obj = annotated.get('object', {})
    raise GateError('tag does not resolve to a commit')


def check_tag(client: Client, base: str, tag: str, commit: str, require: bool = False) -> None:
    target = tag_target(client, base, tag)
    if target is None and require:
        raise GateError(f'tag {tag} is missing')
    if target is not None and target != commit:
        raise GateError(f'tag {tag} points at another commit')


def ensure_tag(client: Client, base: str, tag: str, commit: str,
               existing: dict | None) -> None:
    target = tag_target(client, base, tag)
    if target is None:
        if existing and existing.get('draft') is not True:
            raise GateError('published release is missing its tag')
        client.json('POST', f'{base}/git/refs', {'ref': f'refs/tags/{tag}', 'sha': commit})
    check_tag(client, base, tag, commit, require=True)


def names_by_id(items: list[dict]) -> dict[str, dict]:
    result: dict[str, dict] = {}
    ids: set[int] = set()
    for item in items:
        name, asset_id = item.get('name'), item.get('id')
        safe_name(name)
        if not isinstance(asset_id, int):
            raise GateError('invalid release asset identity')
        if name in result or asset_id in ids:
            raise GateError('duplicate release asset name or ID')
        result[name] = item
        ids.add(asset_id)
    return result


def expected(manifest: Path, files: Path, tag: str) -> dict[str, tuple[int, str, Path]]:
    document = verify_mirror(files, manifest, tag)
    result = {entry['name']: (entry['bytes'], entry['sha256'], files / entry['name'])
              for entry in document['files']}
    result['release-manifest.json'] = (manifest.stat().st_size, digest(manifest.read_bytes()), manifest)
    return result


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def download_and_check(client: Client, base: str, item: dict, size: int, checksum: str) -> None:
    if item.get('state') != 'uploaded' or item.get('size') != size:
        raise GateError(f'release asset metadata differs: {item.get("name")}')
    raw = client.request('GET', f'{base}/releases/assets/{item["id"]}',
                         accept='application/octet-stream')
    if len(raw) != size or digest(raw) != checksum:
        raise GateError(f'release asset bytes differ: {item.get("name")}')


def upload_url(release: dict, repo: str, release_id: int, name: str) -> str:
    template = release.get('upload_url', '')
    url = template.split('{', 1)[0]
    parsed = urlparse(url)
    if (parsed.scheme != 'https' or parsed.netloc != 'uploads.github.com'
            or parsed.path != f'/repos/{repo}/releases/{release_id}/assets'
            or parsed.query):
        raise GateError('unexpected release upload URL')
    return f'{url}?{urlencode({"name": name})}'


def checked_assets(client: Client, base: str, release_id: int,
                   wanted: dict[str, tuple[int, str, Path]], allow_missing: bool) -> dict[str, dict]:
    present = names_by_id(assets(client, base, release_id))
    extra = set(present) - set(wanted)
    if extra:
        raise GateError(f'unexpected release assets: {sorted(extra)}')
    missing = set(wanted) - set(present)
    if missing and not allow_missing:
        raise GateError(f'missing release assets: {sorted(missing)}')
    for name, item in present.items():
        size, checksum, _ = wanted[name]
        download_and_check(client, base, item, size, checksum)
    return present


def stage(client: Client, repo: str, tag: str, commit: str, files: Path,
          manifest: Path, prerelease: bool = False) -> dict:
    base = repo_url(repo)
    if not SHA.fullmatch(commit):
        raise GateError('public target must be a full commit SHA')
    wanted = expected(manifest, files, tag)
    matches = [item for item in releases(client, base) if item.get('tag_name') == tag]
    if len(matches) > 1:
        raise GateError('multiple releases have the requested tag')
    if matches:
        release = release_by_id(client, base, matches[0]['id'])
        bound_release(release, matches[0]['id'], tag, commit)
        if bool(release.get('prerelease')) != prerelease:
            raise GateError('existing release prerelease setting differs')
        ensure_tag(client, base, tag, commit, release)
    else:
        ensure_tag(client, base, tag, commit, None)
        release = client.json('POST', f'{base}/releases', {
            'tag_name': tag, 'target_commitish': commit, 'name': tag,
            'draft': True, 'prerelease': prerelease,
        })
        if not isinstance(release, dict) or not isinstance(release.get('id'), int):
            raise GateError('release creation returned no numeric ID')
        bound_release(release, release['id'], tag, commit)
        if release.get('draft') is not True:
            raise GateError('new release was not created as a draft')
    release_id = release['id']
    # Interrupted GitHub uploads can leave a "starter" reservation. Only that
    # incomplete state may be removed on a draft; uploaded bytes are immutable.
    for item in assets(client, base, release_id):
        if item.get('state') == 'starter' and item.get('name') in wanted:
            if release.get('draft') is not True:
                raise GateError('published release has an incomplete asset')
            client.request('DELETE', f'{base}/releases/assets/{item["id"]}')
    present = checked_assets(client, base, release_id, wanted, allow_missing=True)
    if release.get('draft') is not True and len(present) != len(wanted):
        raise GateError('published release is missing expected assets')
    for name in sorted(set(wanted) - set(present)):
        _, _, path = wanted[name]
        client.request('POST', upload_url(release, repo, release_id, name),
                       path.read_bytes(), content_type='application/octet-stream')
    checked_assets(client, base, release_id, wanted, allow_missing=False)
    bound_release(release_by_id(client, base, release_id), release_id, tag, commit)
    check_tag(client, base, tag, commit, require=True)
    return {'release_id': release_id, 'manifest_sha256': wanted['release-manifest.json'][1]}


def finalize(client: Client, repo: str, tag: str, commit: str,
             release_id: int, manifest_sha256: str) -> dict:
    base = repo_url(repo)
    if not SHA.fullmatch(commit) or not SHA256.fullmatch(manifest_sha256):
        raise GateError('invalid public commit or manifest digest')
    release = release_by_id(client, base, release_id)
    bound_release(release, release_id, tag, commit)
    present = names_by_id(assets(client, base, release_id))
    manifest_asset = present.get('release-manifest.json')
    if manifest_asset is None:
        raise GateError('release manifest asset is missing')
    if manifest_asset.get('state') != 'uploaded':
        raise GateError('release manifest upload is incomplete')
    manifest_raw = client.request('GET', f'{base}/releases/assets/{manifest_asset["id"]}',
                                  accept='application/octet-stream')
    if digest(manifest_raw) != manifest_sha256 or manifest_asset.get('size') != len(manifest_raw):
        raise GateError('release manifest digest or size differs')
    try:
        document = json.loads(manifest_raw)
    except ValueError as error:
        raise GateError('release manifest is not JSON') from error
    if (not isinstance(document, dict) or document.get('tag') != tag
            or document.get('version') != tag.removeprefix('v')
            or document.get('schema') != 'upeg-release-manifest/v2'):
        raise GateError('release manifest identity differs')
    # Reuse the canonical profile and per-file validator over downloaded bytes.
    import tempfile
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        manifest_path = root / 'release-manifest.json'
        manifest_path.write_bytes(manifest_raw)
        files = root / 'files'
        files.mkdir()
        for name, item in present.items():
            if name == 'release-manifest.json':
                continue
            if item.get('state') != 'uploaded':
                raise GateError(f'release asset upload is incomplete: {name}')
            raw = client.request('GET', f'{base}/releases/assets/{item["id"]}',
                                 accept='application/octet-stream')
            if len(raw) != item.get('size'):
                raise GateError(f'release asset metadata size differs: {name}')
            (files / name).write_bytes(raw)
        verify_mirror(files, manifest_path, tag)
    if set(present) != {entry['name'] for entry in document['files']} | {'release-manifest.json'}:
        raise GateError('release asset set differs from manifest')
    check_tag(client, base, tag, commit, require=True)
    latest = release_by_id(client, base, release_id)
    bound_release(latest, release_id, tag, commit)
    latest_assets = names_by_id(assets(client, base, release_id))
    if {name: (item['id'], item.get('size'), item.get('state')) for name, item in latest_assets.items()} != {
            name: (item['id'], item.get('size'), item.get('state')) for name, item in present.items()}:
        raise GateError('release assets changed during verification')
    if latest.get('draft') is False:
        return {'release_id': release_id, 'published': True}
    if latest.get('draft') is not True:
        raise GateError('release draft state is invalid')
    edited = client.json('PATCH', f'{base}/releases/{release_id}', {'draft': False})
    bound_release(edited, release_id, tag, commit)
    if edited.get('draft') is not False:
        raise GateError('release was not published')
    check_tag(client, base, tag, commit, require=True)
    return {'release_id': release_id, 'published': True}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for command in ('stage', 'finalize'):
        option = sub.add_parser(command)
        option.add_argument('--repo', required=True)
        option.add_argument('--tag', required=True)
        option.add_argument('--commit', required=True)
        if command == 'stage':
            option.add_argument('--files', type=Path, required=True)
            option.add_argument('--manifest', type=Path, required=True)
            option.add_argument('--prerelease', action='store_true')
        else:
            option.add_argument('--release-id', required=True, type=int)
            option.add_argument('--manifest-sha256', required=True)
    args = parser.parse_args()
    try:
        client = Client(os.environ.get('GH_TOKEN', ''))
        if args.command == 'stage':
            result = stage(client, args.repo, args.tag, args.commit, args.files,
                           args.manifest, args.prerelease)
        else:
            result = finalize(client, args.repo, args.tag, args.commit,
                              args.release_id, args.manifest_sha256)
        print(json.dumps(result, sort_keys=True))
    except (APIError, GateError, OSError, ValueError, KeyError, TypeError) as error:
        print(f'[FAIL] {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
