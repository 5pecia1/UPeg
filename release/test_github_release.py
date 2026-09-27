#!/usr/bin/env python3
"""Local API simulations for the release draft and finalization protocol."""
from __future__ import annotations

import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from urllib.parse import parse_qs, urlparse
from urllib.request import Request

sys.path.insert(0, str(Path(__file__).parent))
from artifacts import GateError, manifest_v2
from github_release import APIError, Client, PrivateRedirect, digest, finalize, stage

REPO = '5pecia1/UPeg'
TAG = 'v1.2.3'
COMMIT = 'a' * 40
BUILD = 'b' * 40
BASE = f'https://api.github.com/repos/{REPO}'


class FakeClient:
    def __init__(self):
        self.release = None
        self.assets = []
        self.bytes = {}
        self.tag = None
        self.next_asset_id = 200
        self.patches = 0
        self.uploads = []

    def json(self, method, url, payload=None):
        path = url.removeprefix(BASE)
        if method == 'GET' and path.startswith('/releases?'):
            return [dict(self.release)] if self.release else []
        if method == 'POST' and path == '/releases':
            if self.release:
                raise APIError(422, b'already exists')
            self.release = dict(payload, id=100,
                                upload_url=f'https://uploads.github.com/repos/{REPO}/releases/100/assets{{?name,label}}')
            return dict(self.release)
        if method == 'POST' and path == '/git/refs':
            if self.tag is not None:
                raise APIError(422, b'tag already exists')
            self.tag = payload['sha']
            return {'ref': payload['ref'], 'object': {'type': 'commit', 'sha': self.tag}}
        if method == 'GET' and path.startswith('/releases/') and path.count('/') == 2:
            if path != '/releases/100':
                raise APIError(404, b'not found')
            return dict(self.release)
        if method == 'GET' and path.startswith('/releases/100/assets?'):
            page = int(parse_qs(urlparse(url).query).get('page', ['1'])[0])
            start = (page - 1) * 100
            return [dict(item) for item in self.assets[start:start + 100]]
        if method == 'GET' and path.startswith('/git/ref/tags/'):
            if self.tag is None:
                raise APIError(404, b'not found')
            return {'object': {'type': 'commit', 'sha': self.tag}}
        if method == 'PATCH' and path == '/releases/100':
            self.patches += 1
            self.release.update(payload)
            self.tag = self.release['target_commitish']
            return dict(self.release)
        raise AssertionError((method, url, payload))

    def request(self, method, url, data=None, accept='application/vnd.github+json', content_type=None):
        path = urlparse(url).path.removeprefix(f'/repos/{REPO}')
        if method == 'POST' and path == '/releases/100/assets':
            name = parse_qs(urlparse(url).query)['name'][0]
            asset_id = self.next_asset_id
            self.next_asset_id += 1
            self.assets.append({'id': asset_id, 'name': name, 'size': len(data), 'state': 'uploaded'})
            self.bytes[asset_id] = data
            self.uploads.append(name)
            return b'{}'
        if method == 'GET' and path.startswith('/releases/assets/'):
            return self.bytes[int(path.rsplit('/', 1)[1])]
        if method == 'DELETE' and path.startswith('/releases/assets/'):
            asset_id = int(path.rsplit('/', 1)[1])
            self.assets = [item for item in self.assets if item['id'] != asset_id]
            self.bytes.pop(asset_id, None)
            return b''
        raise AssertionError((method, url, data))


class ReleaseProtocolTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.files = self.root / 'files'
        self.files.mkdir()
        from artifacts import profile_files
        for name in profile_files('1.2.3', TAG, 'public'):
            (self.files / name).write_bytes(f'contents of {name}\n'.encode())
        self.manifest = self.root / 'release-manifest.json'
        manifest_v2(self.files, self.manifest, TAG, BUILD, 'public')
        self.client = FakeClient()

    def staged(self):
        return stage(self.client, REPO, TAG, COMMIT, self.files, self.manifest)

    def test_stage_finalize_and_replay_identical_release(self):
        result = self.staged()
        self.assertEqual(self.client.tag, COMMIT)
        self.assertEqual(result['manifest_sha256'], digest(self.manifest.read_bytes()))
        self.assertTrue(self.client.release['draft'])
        self.assertEqual(len(self.client.assets), len(list(self.files.iterdir())) + 1)
        self.assertEqual(finalize(self.client, REPO, TAG, COMMIT, result['release_id'],
                                  result['manifest_sha256'])['published'], True)
        self.assertEqual(self.client.patches, 1)
        self.staged()
        finalize(self.client, REPO, TAG, COMMIT, result['release_id'], result['manifest_sha256'])
        self.assertEqual(self.client.patches, 1)
        self.assertEqual(len(self.client.uploads), len(list(self.files.iterdir())) + 1)

    def test_resume_uploads_only_missing_assets(self):
        result = self.staged()
        removed = self.client.assets.pop()
        self.client.bytes.pop(removed['id'])
        uploads = len(self.client.uploads)
        self.assertEqual(self.staged(), result)
        self.assertEqual(len(self.client.uploads), uploads + 1)
        self.assertEqual(self.client.uploads[-1], removed['name'])

    def test_resume_discards_only_incomplete_starter(self):
        self.staged()
        item = self.client.assets[0]
        item['state'] = 'starter'
        uploads = len(self.client.uploads)
        self.staged()
        self.assertEqual(len(self.client.uploads), uploads + 1)
        self.assertEqual(len(self.client.assets), len(list(self.files.iterdir())) + 1)

    def test_changed_existing_asset_is_rejected(self):
        self.staged()
        item = self.client.assets[0]
        self.client.bytes[item['id']] = b'X' * item['size']
        with self.assertRaisesRegex(GateError, 'bytes differ'):
            self.staged()

    def test_finalize_requires_manifest_hash_and_exact_asset_set(self):
        result = self.staged()
        with self.assertRaisesRegex(GateError, 'manifest digest'):
            finalize(self.client, REPO, TAG, COMMIT, 100, '0' * 64)
        self.client.assets.append({'id': 999, 'name': 'extra', 'size': 1, 'state': 'uploaded'})
        self.client.bytes[999] = b'x'
        with self.assertRaisesRegex(GateError, 'asset set mismatch|asset set differs'):
            finalize(self.client, REPO, TAG, COMMIT, 100, result['manifest_sha256'])
        self.assertEqual(self.client.patches, 0)

    def test_wrong_tag_target_is_rejected_before_publish(self):
        result = self.staged()
        self.client.tag = 'c' * 40
        with self.assertRaisesRegex(GateError, 'another commit'):
            finalize(self.client, REPO, TAG, COMMIT, 100, result['manifest_sha256'])
        self.assertEqual(self.client.patches, 0)

    def test_missing_tag_blocks_finalization(self):
        result = self.staged()
        self.client.tag = None
        with self.assertRaisesRegex(GateError, 'tag .* missing'):
            finalize(self.client, REPO, TAG, COMMIT, 100, result['manifest_sha256'])
        self.assertEqual(self.client.patches, 0)

    def test_existing_published_release_with_missing_tag_is_rejected(self):
        result = self.staged()
        finalize(self.client, REPO, TAG, COMMIT, 100, result['manifest_sha256'])
        self.client.tag = None
        with self.assertRaisesRegex(GateError, 'published release is missing its tag'):
            self.staged()

    def test_target_and_release_id_are_bound(self):
        result = self.staged()
        with self.assertRaisesRegex(GateError, 'HTTP 404'):
            finalize(self.client, REPO, TAG, COMMIT, 101, result['manifest_sha256'])
        self.client.release['target_commitish'] = 'c' * 40
        with self.assertRaisesRegex(GateError, 'target commit'):
            finalize(self.client, REPO, TAG, COMMIT, 100, result['manifest_sha256'])

    def test_public_destination_is_fixed(self):
        with self.assertRaisesRegex(GateError, 'destination'):
            stage(self.client, 'other/repository', TAG, COMMIT, self.files, self.manifest)

    def test_upload_headers_and_cross_host_redirect_token(self):
        class Response(io.BytesIO):
            def __enter__(self):
                return self

            def __exit__(self, *_):
                self.close()

        class Opener:
            def __init__(self):
                self.request = None

            def open(self, request, timeout):
                self.request = request
                return Response(b'{}')

        client = Client('test-token')
        opener = Opener()
        client.opener = opener
        client.request('POST', 'https://uploads.github.com/repos/5pecia1/UPeg/releases/100/assets?name=a',
                       b'asset', content_type='application/octet-stream')
        request = opener.request
        self.assertEqual(request.get_header('Accept'), 'application/vnd.github+json')
        self.assertEqual(request.get_header('Content-type'), 'application/octet-stream')
        self.assertEqual(request.get_header('Authorization'), 'Bearer test-token')
        redirected = PrivateRedirect().redirect_request(
            Request('https://api.github.com/repos/5pecia1/UPeg/releases/assets/1',
                    headers={'Authorization': 'Bearer test-token'}),
            None, 302, 'Found', {}, 'https://release-assets.githubusercontent.com/asset?sig=abc')
        self.assertIsNone(redirected.get_header('Authorization'))


if __name__ == '__main__':
    unittest.main()
