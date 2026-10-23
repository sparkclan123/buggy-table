"""Native sidecar contract tests against disposable fixture B, never user profiles."""
import argparse
import json
import os
from pathlib import Path
import queue
import socket
import subprocess
import threading

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / 'artifacts' / 'vpn-fixtures'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('binary', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    # Neither Docker nor a separately installed OpenVPN executable is discoverable.
    environment = dict(os.environ, PATH='')
    subprocess.run([str(binary), '--self-test'], env=environment, check=True)
    config = (FIXTURE / 'client-1.ovpn').read_text()
    token = '1234567890abcdef' * 4

    def start(host='vpn-db.test', password='vpn-fixture-password', profile=config, key_password=''):
        process = subprocess.Popen([str(binary)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, env=environment, text=True)
        payload = dict(config=profile, username='vpn-fixture-user', password=password, keyPassword=key_password,
                       host=host, port=5432, dns=[], token=token)
        process.stdin.write(json.dumps(payload) + '\n')
        process.stdin.flush()
        events = queue.Queue()
        def consume():
            for line in process.stdout:
                events.put(json.loads(line))
            events.put({'state': 'exited'})
        threading.Thread(target=consume, daemon=True).start()
        return process, events

    def finish(process):
        process.stdin.close()
        process.wait(timeout=3)
        errors = process.stderr.read()
        assert not errors, errors

    def ready(events):
        event = events.get(timeout=50)
        assert event['state'] == 'ready', event
        assert 0 < event['port'] <= 65535
        return event['port']

    def exchange(port, auth=token, mode=b'P'):
        with socket.create_connection(('127.0.0.1', port), timeout=20) as connection:
            connection.sendall(auth.encode() + b'\n' + mode)
            response = connection.makefile('rb')
            status = response.readline(80)
            if status != b'OK\n':
                return status
            if mode == b'P':
                assert int(response.readline(32)) >= 0
            else:
                # PostgreSQL SSLRequest, proving relay framing doesn't eat DB bytes.
                connection.sendall(bytes.fromhex('0000000804d2162f'))
                assert response.read(1) == b'S'
            return status

    for host in ['vpn-db.test', 'vpn6-db.test', 'fd77::1']:
        process, events = start(host)
        try:
            port = ready(events)
            reply = exchange(port)
            assert reply == b'OK\n', (host, reply)
            assert exchange(port, mode=b'C') == b'OK\n'
            try:
                assert exchange(port, auth='0'*64) != b'OK\n'
            except (ConnectionResetError, BrokenPipeError):
                pass
        finally:
            finish(process)
        print('Private DNS/TCP, relay authentication and parent-exit cleanup:', host)

    process, events = start(profile=config + '\npull-filter ignore "dhcp-option DNS"\n')
    try:
        assert exchange(ready(events)) == b'ERR VPN_DNS_REQUIRED\n'
    finally:
        finish(process)
    print('Missing VPN DNS does not use a public/system resolver')

    process, events = start(password='wrong-disposable-password')
    try:
        event = events.get(timeout=50)
        assert event['state'] == 'disconnected' and event['code'] == 'VPN_AUTH', event
    finally:
        finish(process)
    print('Incorrect VPN password rejected')

    # Key decryption remains supported after moving away from OpenVPN 2 containers.
    encrypted = subprocess.check_output(['openssl', 'pkey', '-in', str(FIXTURE / 'client.key'),
                                         '-aes-256-cbc', '-passout', 'pass:disposable-key-password'], text=True)
    begin, end = config.index('<key>'), config.index('</key>')
    encrypted_profile = config[:begin] + '<key>\n' + encrypted + config[end:]
    process, events = start(profile=encrypted_profile, key_password='disposable-key-password')
    try:
        assert exchange(ready(events)) == b'OK\n'
    finally:
        finish(process)
    print('Encrypted client private key accepted')

    for host, expected in [('no-such-database.invalid', b'ERR VPN_DNS\n'), ('127.0.0.1', b'ERR VPN_DESTINATION\n')]:
        process, events = start(host)
        try:
            assert exchange(ready(events)) == expected
        finally:
            finish(process)
    print('DNS and destination failures stay closed')

    process, events = start(profile=config + '\nignore-unknown-option verify-x509-name remote-cert-tls\nverify-x509-name wrong-vpn-identity name\n')
    try:
        event = events.get(timeout=50)
        assert event['state'] == 'disconnected', event
        assert event['code'] == 'VPN_CERTIFICATE', event
    finally:
        finish(process)
    print('Incorrect VPN server certificate identity rejected even with ignore-unknown-option')


if __name__ == '__main__':
    main()
