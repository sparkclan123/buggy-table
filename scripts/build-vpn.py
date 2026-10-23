"""Build the bundled, unprivileged OpenVPN engine. Downloads are build-time only."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'native' / 'vpn'
CACHE = ROOT / 'artifacts' / 'native-vpn'
MACOS_MINIMUM = '11.0'


def build_crypto(deps, checksum):
    system = platform.system()
    architecture = platform.machine().lower()
    target = ('darwin64-arm64-cc' if architecture == 'arm64' else 'darwin64-x86_64-cc') if system == 'Darwin' else (
        'VC-WIN64A' if system == 'Windows' else 'linux-x86_64')
    prefix = CACHE / 'openssl-install'
    identity = f'{checksum}:{target}:macos{MACOS_MINIMUM}:static-v1'
    marker = prefix / '.built-identity'
    if marker.exists() and marker.read_text() == identity:
        return prefix
    build = CACHE / 'openssl-build'
    if build.exists():
        shutil.rmtree(build)
    build.mkdir()
    environment = dict(os.environ)
    if system == 'Darwin':
        environment['MACOSX_DEPLOYMENT_TARGET'] = MACOS_MINIMUM
    subprocess.run(['perl', str(deps / 'openssl' / 'Configure'), target, 'no-shared', 'no-tests', 'no-module',
                    '--prefix=' + prefix.as_posix(), '--libdir=lib'], cwd=build, env=environment, check=True)
    make = ['nmake'] if system == 'Windows' else ['make', '-j3']
    subprocess.run(make + ['build_libs'], cwd=build, env=environment, check=True)
    subprocess.run(make + ['install_dev'], cwd=build, env=environment, check=True)
    marker.write_text(identity)
    return prefix


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--sanitize', action='store_true')
    args = parser.parse_args()
    deps = CACHE / 'deps'
    deps.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((SOURCE / 'deps.json').read_text())
    for name, spec in manifest.items():
        destination = deps / name
        marker = destination / '.verified-sha256'
        if marker.exists() and marker.read_text() == spec['sha256']:
            continue
        archive = CACHE / (name + '.tar.gz')
        data = archive.read_bytes() if archive.exists() else b''
        if hashlib.sha256(data).hexdigest() != spec['sha256']:
            print('Downloading', name, flush=True)
            data = urllib.request.urlopen(spec['url'], timeout=120).read()
        if hashlib.sha256(data).hexdigest() != spec['sha256']:
            raise RuntimeError('Source checksum mismatch: ' + name)
        archive.write_bytes(data)
        with tarfile.open(fileobj=io.BytesIO(data)) as tar:
            members = tar.getmembers()
            root = members[0].name.split('/')[0]
            for member in members:
                path = Path(member.name)
                if path.is_absolute() or '..' in path.parts or member.issym() or member.islnk():
                    raise RuntimeError('Unsafe archive path: ' + name)
            unpack = deps / (name + '-unpack')
            unpack.mkdir(exist_ok=True)
            tar.extractall(unpack)
        if destination.exists():
            shutil.rmtree(destination)
        (unpack / root).rename(destination)
        unpack.rmdir()
        marker.write_text(spec['sha256'])
    # Upstream 3.11.8 includes Windows driver-only DCO types even when using an
    # external TUN factory. Generate narrowly adapted headers from the verified
    # originals; preserve upstream licenses and fail if the pinned code changes.
    external_tun_guard = '#if defined(OPENVPN_PLATFORM_WIN) && !defined(OPENVPN_EXTERNAL_TUN_FACTORY)'
    adapters = {
        'openvpn/transport/dco.hpp': ('#if defined(OPENVPN_PLATFORM_WIN)', external_tun_guard, 2),
        'openvpn/client/cliopt.hpp': (
            '#if defined(OPENVPN_PLATFORM_WIN)\n            if (config.clientconf.tunPersist)',
            external_tun_guard + '\n            if (config.clientconf.tunPersist)', 1),
    }
    for path, (original, replacement, count) in adapters.items():
        content = (deps / 'openvpn3' / path).read_text()
        if content.count(original) != count:
            raise RuntimeError('OpenVPN external TUN adapter no longer matches: ' + path)
        output_header = CACHE / 'overrides' / path
        output_header.parent.mkdir(parents=True, exist_ok=True)
        output_header.write_text('// Modified by buggy-table: omit Windows OS driver types for external TUN.\n'
                                 + content.replace(original, replacement))
    build = CACHE / ('asan' if args.sanitize else 'build')
    command = ['cmake', '-S', str(SOURCE), '-B', str(build), '-DCMAKE_BUILD_TYPE=Release',
               '-DVPN_DEPS=' + deps.as_posix(), '-DVPN_SANITIZE=' + ('ON' if args.sanitize else 'OFF')]
    openssl = build_crypto(deps, manifest['openssl']['sha256'])
    extension = '.lib' if platform.system() == 'Windows' else '.a'
    # FindOpenSSL on Windows also uses the lib prefix for OpenSSL 3's MSVC build.
    library = 'lib'
    command += ['-DOPENSSL_ROOT_DIR=' + openssl.as_posix(),
                '-DOPENSSL_INCLUDE_DIR=' + (openssl / 'include').as_posix(),
                '-DOPENSSL_CRYPTO_LIBRARY=' + (openssl / 'lib' / (library + 'crypto' + extension)).as_posix(),
                '-DOPENSSL_SSL_LIBRARY=' + (openssl / 'lib' / (library + 'ssl' + extension)).as_posix()]
    if platform.system() == 'Darwin':
        command += ['-DCMAKE_OSX_DEPLOYMENT_TARGET=' + MACOS_MINIMUM]
    subprocess.run(command, check=True)
    subprocess.run(['cmake', '--build', str(build), '--config', 'Release', '--parallel', '3'], check=True)
    suffix = '.exe' if platform.system() == 'Windows' else ''
    binary = build / ('Release' if suffix else '') / ('buggy-vpn' + suffix)
    subprocess.run([str(binary), '--self-test'], check=True)
    host = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]
    output = ROOT / 'src-tauri' / 'binaries'
    output.mkdir(exist_ok=True)
    if not args.sanitize:
        shutil.copy2(binary, output / ('buggy-vpn-' + host + suffix))
    notices = []
    licenses = {'openssl': 'LICENSE.txt', 'openvpn3': 'LICENSES/MPL-2.0.txt', 'lwip': 'COPYING', 'asio': 'asio/LICENSE_1_0.txt',
                'lz4': 'lib/LICENSE', 'xxhash': 'LICENSE', 'jsoncpp': 'LICENSE'}
    for name, path in licenses.items():
        notices.append(name + '\nSource: ' + manifest[name]['url'] + '\nSHA-256: ' + manifest[name]['sha256']
                       + '\n\n' + (deps / name / path).read_text())
    version = json.loads((ROOT / 'package.json').read_text())['version']
    notices.append('OpenVPN build adaptations: scripts/build-vpn.py guards Windows driver-only DCO types; '
                   'native/vpn/overrides omits optional host hardware metadata. Corresponding source: '
                   f'https://github.com/sparkclan123/buggy-table/tree/preview/v{version}/native/vpn '
                   f'and https://github.com/sparkclan123/buggy-table/blob/preview/v{version}/scripts/build-vpn.py')
    (output / 'VPN-NOTICES.txt').write_text('\n\n'.join(notices))
    print('Bundled VPN engine built:', binary)


if __name__ == '__main__':
    main()
