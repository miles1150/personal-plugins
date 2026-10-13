#!/usr/bin/env python3
"""Real distribution smoke tests; synthetic/temp HOME, no real authorization."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

release = Path(sys.argv[1]).resolve()
scripts = Path(__file__).resolve().parent

def run(args, env, ok=True):
    p = subprocess.run(args, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True)
    assert (p.returncode == 0) == ok, (args, p.stdout, p.stderr)
    return p

with tempfile.TemporaryDirectory(prefix='keyenv distribution ') as temp:
    base = Path(temp)
    # Deliberately restricted PATH: neither Go nor shasum is available.
    path = base / 'path'
    path.mkdir()
    for name in ['sh', 'dirname', 'sha256sum', 'awk', 'wc', 'tr', 'cmp', 'mktemp', 'rm', 'cp', 'chmod', 'ln', 'mv', 'mkdir']:
        source = shutil.which(name)
        assert source, name
        (path / name).symlink_to(source)
    home = base / 'home'
    home.mkdir()
    env = {**os.environ, 'HOME': str(home), 'PATH': str(path)}
    env.pop('KEYENV_CLI', None)
    env.pop('KEYENV_OAUTH_BUNDLE', None)
    env.pop('KEYENV_BUILD_FROM_SOURCE', None)
    expected_commit = subprocess.check_output(['git', '-C', str(scripts), 'rev-parse', 'HEAD'], text=True).strip()
    assert (release / 'BUILD-COMMIT').read_text().strip() == expected_commit
    assert set(p.name for p in release.iterdir()) == {'BUILD-COMMIT', 'SHA256SUMS'} | set(p.name for p in release.glob('keyenv-*.tar.gz'))
    manifest_lines = (release / 'SHA256SUMS').read_text().splitlines()
    assert {line.split()[1] for line in manifest_lines} == {p.name for p in release.iterdir() if p.name != 'SHA256SUMS'}
    for line in manifest_lines:
        digest, name = line.split()
        assert hashlib.sha256((release / name).read_bytes()).hexdigest() == digest
    packages = {}
    for archive in release.glob('*.tar.gz'):
        with tarfile.open(archive) as tar:
            names = tar.getnames()
            root = names[0].rstrip('/')
            personal = 'personal-connect' in root
            allowed = {'', 'oauth', 'connect-existing.sh', 'checksums.sh', 'VERSION', 'SHA256SUMS', 'oauth/client.public.json', 'oauth/client.secret.enc.json'} if personal else {'', 'keyenv', 'install-local.sh', 'checksums.sh', 'VERSION', 'SHA256SUMS'}
            for member in tar.getmembers():
                assert member.name == root or member.name.startswith(root + '/')
                relative = member.name[len(root):].lstrip('/')
                assert relative in allowed, relative
                assert member.isfile() or member.isdir()
            tar.extractall(base, filter='data')
            packages[root] = base / root
    assert len(packages) == 5
    for name, directory in packages.items():
        package_version = (directory / 'VERSION').read_text().strip()
        assert name.startswith(f'keyenv-{package_version}-')
    package = next(p for n, p in packages.items() if n.endswith('linux-amd64'))
    personal = next(p for n, p in packages.items() if n.endswith('personal-connect'))
    dest = home / '.local/bin'
    dest.mkdir(parents=True)
    install = [str(path / 'sh'), str(package / 'install-local.sh'), str(package), str(dest)]
    run([str(path / 'sh'), str(package / 'install-local.sh'), 'relative', str(dest)], env, False)
    run([str(path / 'sh'), str(package / 'install-local.sh'), str(package), str(base / 'absent')], env, False)
    run(install, env)
    run(install, env)  # identical install is idempotent
    manifest = (package / 'SHA256SUMS').read_text()
    (package / 'SHA256SUMS').write_text(manifest + '0' * 64 + '  unexpected\n')
    run(install, env, False)
    (package / 'SHA256SUMS').write_text(manifest.replace('  VERSION', '  ../VERSION'))
    run(install, env, False)
    (package / 'SHA256SUMS').write_text(manifest)
    version = json.loads(run([str(dest / 'keyenv'), 'version'], env).stdout)['version']
    assert version == (package / 'VERSION').read_text().strip()
    # Login zsh's keyenv() prepends --home to every command, including version.
    wrapped_version = run([str(dest / 'keyenv'), '--home', str(home / 'absent vault with spaces'), 'version'], env)
    assert json.loads(wrapped_version.stdout)['version'] == version
    assert not (home / 'absent vault with spaces').exists()
    assert package.name == f'keyenv-{version}-linux-amd64'
    (package / 'keyenv').write_bytes(b'corrupt')
    run(install + ['--replace'], env, False)
    assert json.loads(run([str(dest / 'keyenv'), 'version'], env).stdout)['version'] == version
    shutil.copyfile(dest / 'keyenv', package / 'keyenv')
    (dest / 'keyenv').unlink()
    (dest / 'keyenv').write_bytes(b'old installation')
    run(install, env, False)
    run(install + ['--replace'], env)
    (dest / 'keyenv').unlink()
    (dest / 'keyenv').symlink_to(package / 'keyenv')
    run(install + ['--replace'], env, False)
    (dest / 'keyenv').unlink()
    run(install, env)
    # No terminal: must stop before decrypting or authorizing the real bundle.
    p = run([str(path / 'sh'), str(personal / 'connect-existing.sh'), 'synthetic@example.com', 'synthetic-folder', str(home / '.config/keyenv/vault')], env, False)
    assert 'INTERACTION_REQUIRED' in p.stderr, p.stderr
    assert not (home / '.zprofile').exists()
    assert not list(home.rglob('client.json'))
    assert not list(home.rglob('unlock-cache.json'))
    assert not list(home.rglob('oauth-login.json'))
    (personal / 'oauth/client.public.json').write_text('{}')
    p = run([str(path / 'sh'), str(personal / 'connect-existing.sh'), 'synthetic@example.com', 'synthetic-folder'], env, False)
    assert 'Checksum failed' in p.stderr
    env['KEYENV_CLI'] = 'relative/keyenv'
    run([str(path / 'sh'), str(scripts / 'connect-existing.sh'), 'synthetic@example.com', 'synthetic-folder'], env, False)
print('PASS: five archive allowlists, complete checksums, version, no-Go/no-shasum installation, spaces, corruption, repeat/replace/symlink policy, noninteractive connect, no caches/profile/client created')
