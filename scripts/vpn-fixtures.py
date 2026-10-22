"""Disposable local OpenVPN/PG fixtures. Never reads user VPN profiles."""
import argparse
import json
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'artifacts' / 'vpn-fixtures'
NAMES = ['buggy-table-vpn-fixture-a', 'buggy-table-vpn-fixture-b']


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['start', 'stop'])
    args = parser.parse_args()
    if args.action == 'stop':
        subprocess.run(['docker', 'rm', '-f', *NAMES], check=False)
        return
    OUT.mkdir(parents=True, exist_ok=True)
    run('docker', 'build', '-t', 'buggy-table-vpn-fixture', str(ROOT / 'tests/vpn'))
    openssl = '/usr/bin/openssl'
    def cert(name, extensions):
        run(openssl, 'req', '-new', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(OUT / f'{name}.key'), '-out', str(OUT / f'{name}.csr'), '-subj', f'/CN={name}', stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        (OUT / f'{name}.ext').write_text(extensions)
        run(openssl, 'x509', '-req', '-in', str(OUT / f'{name}.csr'), '-CA', str(OUT / 'ca.crt'), '-CAkey', str(OUT / 'ca.key'), '-CAcreateserial', '-out', str(OUT / f'{name}.crt'), '-days', '2', '-extfile', str(OUT / f'{name}.ext'), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run(openssl, 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(OUT / 'ca.key'), '-out', str(OUT / 'ca.crt'), '-days', '2', '-subj', '/CN=buggy-table disposable VPN CA', stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    cert('server', 'basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:vpn-db.test,DNS:vpn6-db.test,IP:10.77.0.1,IP:fd77::1\n')
    cert('client', 'basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\n')
    (OUT / 'server.conf').write_text('port 1194\nproto udp\ndev tun\ntopology subnet\nserver 10.77.0.0 255.255.255.0\nserver-ipv6 fd77::/64\nca /fixtures/ca.crt\ncert /fixtures/server.crt\nkey /fixtures/server.key\ndh none\npush "dhcp-option DNS 10.77.0.1"\nkeepalive 5 20\nduplicate-cn\nverb 3\ndisable-dco\n')
    (OUT / 'auth.sh').write_text('#!/bin/sh\n{ IFS= read -r fixture_user; IFS= read -r fixture_password; } < "$1"\n[ "$fixture_user" = "vpn-fixture-user" ] && [ "$fixture_password" = "vpn-fixture-password" ]\n')
    (OUT / 'auth.sh').chmod(0o700)
    (OUT / 'server-auth.conf').write_text((OUT / 'server.conf').read_text().replace('proto udp\n', 'proto tcp-server\n') + 'data-ciphers AES-256-CBC\nauth SHA256\nscript-security 2\nauth-user-pass-verify /fixtures/auth.sh via-file\n')
    for i, name in enumerate(NAMES):
        run('docker', 'run', '-d', '--rm', '--name', name, '--cap-add=NET_ADMIN', '--device=/dev/net/tun', '--sysctl', 'net.ipv6.conf.all.disable_ipv6=0', '-e', 'VPN_SERVER_CONFIG=/fixtures/' + ('server-auth.conf' if i else 'server.conf'), '-p', '127.0.0.1::1194/' + ('tcp' if i else 'udp'), '-e', 'POSTGRES_PASSWORD=disposable-fixture-admin', '-e', 'POSTGRES_DB=fixture', '-v', f'{OUT}:/fixtures:ro', '-v', f'{ROOT / "tests/vpn/init.sql"}:/docker-entrypoint-initdb.d/init.sql:ro', 'buggy-table-vpn-fixture')
        info = json.loads(run('docker', 'inspect', name, capture_output=True).stdout)[0]
        port = info['NetworkSettings']['Ports']['1194/' + ('tcp' if i else 'udp')][0]['HostPort']
        config = f'client\ndev tun\nproto udp\nremote 127.0.0.1 {port}\nnobind\nremote-cert-tls server\npush-peer-info\n'
        if i: config = config.replace('proto udp\n', 'proto tcp-client\n') + 'auth-user-pass\ncipher AES-256-CBC\nauth SHA256\n'
        for tag, filename in [('ca', 'ca.crt'), ('cert', 'client.crt'), ('key', 'client.key')]:
            config += f'<{tag}>\n' + (OUT / filename).read_text() + f'</{tag}>\n'
        (OUT / f'client-{i}.ovpn').write_text(config)
        for attempt in range(40):
            initialized = subprocess.run(['docker', 'exec', name, 'psql', '-U', 'postgres', '-d', 'fixture', '-c', f"UPDATE public.items SET label = '{chr(65+i)}'"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if initialized.returncode == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError('Disposable PostgreSQL fixture did not initialize')

    print('Fixtures ready; both private databases use vpn-db.test / 10.77.0.1 in separate VPN networks.')


if __name__ == '__main__':
    main()
