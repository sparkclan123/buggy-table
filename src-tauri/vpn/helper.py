"""One OpenVPN + one fixed database destination per network namespace.

Only the VPN transport uses the namespace's normal network. Every database and
database-DNS socket is bound to tun0 AND an explicit tun-only policy table. A
terminal unreachable rule prevents falling through when the tunnel disappears.
No shell, host mount, Docker socket, system resolver fallback or generic proxy.
"""
import asyncio
import contextlib
import hmac
import ipaddress
import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

import dns.exception
import dns.message
import dns.query
import dns.rcode
import dns.rdatatype

ROOT = Path('/run/vpn')
PORT = 17353
LIMIT = 1024 * 1024


def emit(**value):
    print(json.dumps(value, separators=(',', ':')), flush=True)


def command(*args, optional=False):
    result = subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if result.returncode and not optional:
        raise RuntimeError('VPN_ROUTE')


def bound_socket(family, kind):
    s = socket.socket(family, kind)
    try:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, b'tun0\0')
        return s
    except BaseException:
        s.close()
        raise


class Tunnel:
    def __init__(self, config):
        self.config = config
        self.ready = False
        self.writers = set()
        self.active = 0
        self.addresses = []
        self.expires = 0
        self.resolving = asyncio.Lock()
        self.routes = False

    def close_flows(self):
        self.ready = False
        self.addresses = []
        self.expires = 0
        for writer in tuple(self.writers):
            writer.close()

    def ready_routes(self):
        # oif policy applies to SO_BINDTODEVICE sockets only. OpenVPN control
        # traffic and replies to the host's local relay keep their original route.
        for version in ('-4', '-6'):
            if self.routes:
                command('ip', version, 'rule', 'del', 'priority', '100', optional=True)
                command('ip', version, 'rule', 'del', 'priority', '101', optional=True)
            command('ip', version, 'rule', 'add', 'priority', '100', 'oif', 'tun0', 'lookup', '200')
            command('ip', version, 'rule', 'add', 'priority', '101', 'oif', 'tun0', 'unreachable')
        self.routes = True
        command('ip', '-4', 'route', 'replace', 'default', 'dev', 'tun0', 'table', '200')
        command('ip', '-6', 'route', 'replace', 'default', 'dev', 'tun0', 'table', '200', optional=True)
        self.ready = True

    def dns_lookup(self):
        host = self.config['host']
        try:
            return [str(ipaddress.ip_address(host))], 60
        except ValueError:
            pass
        servers = self.config.get('dns', []) or json.loads((ROOT / 'network.json').read_text())['dns']
        if not servers:
            raise RuntimeError('VPN_DNS_REQUIRED')
        addresses, ttl = [], 60
        for server in servers[:3]:
            ip = ipaddress.ip_address(server)
            family = socket.AF_INET6 if ip.version == 6 else socket.AF_INET
            for typ in ('A', 'AAAA'):
                query = dns.message.make_query(host, typ)
                try:
                    with bound_socket(family, socket.SOCK_DGRAM) as sock:
                        sock.bind(('::' if ip.version == 6 else '0.0.0.0', 0))
                        sock.setblocking(False)
                        answer = dns.query.udp(query, str(ip), timeout=2, sock=sock)
                    if answer.flags & 0x200:  # DNS truncation: retry over this tunnel's TCP.
                        with bound_socket(family, socket.SOCK_STREAM) as sock:
                            sock.settimeout(2)
                            sock.connect((str(ip), 53))
                            sock.setblocking(False)
                            answer = dns.query.tcp(query, str(ip), timeout=2, sock=sock)
                    if answer.rcode() != dns.rcode.NOERROR:
                        continue
                    for rrset in answer.answer:
                        if rrset.rdtype in (dns.rdatatype.A, dns.rdatatype.AAAA):
                            ttl = min(ttl, rrset.ttl)
                            addresses.extend(str(item.address) for item in rrset)
                except (OSError, dns.exception.DNSException):
                    continue
            if addresses:
                return list(dict.fromkeys(addresses))[:8], ttl
        raise RuntimeError('VPN_DNS')

    async def destination(self):
        if not self.ready:
            raise RuntimeError('VPN_DOWN')
        async with self.resolving:
            if time.monotonic() >= self.expires:
                self.addresses, ttl = await asyncio.to_thread(self.dns_lookup)
                self.expires = time.monotonic() + ttl
        for address in self.addresses:
            ip = ipaddress.ip_address(address)
            if ip.is_loopback or ip.is_unspecified or ip.is_multicast:
                continue
            sock = bound_socket(socket.AF_INET6 if ip.version == 6 else socket.AF_INET, socket.SOCK_STREAM)
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            sock.setblocking(False)
            try:
                await asyncio.wait_for(asyncio.get_running_loop().sock_connect(sock, (address, self.config['port'])), 4)
                if not self.ready:
                    raise RuntimeError('VPN_DOWN')
                return await asyncio.open_connection(sock=sock)
            except BaseException:
                sock.close()
        raise RuntimeError('VPN_DESTINATION')

    async def handle(self, reader, writer):
        if self.active >= 24:
            writer.close()
            return
        self.active += 1
        remote_writer = None
        self.writers.add(writer)
        try:
            auth = await asyncio.wait_for(reader.readexactly(65), 3)
            if not hmac.compare_digest(auth, (self.config['token'] + '\n').encode()):
                return
            mode = await asyncio.wait_for(reader.readexactly(1), 3)
            if mode not in (b'C', b'P'):
                return
            started = time.monotonic()
            remote_reader, remote_writer = await asyncio.wait_for(self.destination(), 16)
            self.writers.add(remote_writer)
            writer.write(b'OK\n')
            await writer.drain()
            if mode == b'P':
                writer.write((str(round((time.monotonic() - started) * 1000)) + '\n').encode())
                await writer.drain()
                return

            async def copy(source, target):
                while data := await source.read(65536):
                    target.write(data)
                    await target.drain()
                with contextlib.suppress(OSError, NotImplementedError):
                    target.write_eof()

            tasks = [asyncio.create_task(copy(reader, remote_writer)), asyncio.create_task(copy(remote_reader, writer))]
            try:
                await asyncio.gather(*tasks)
            finally:
                for task in tasks:
                    task.cancel()
                await asyncio.gather(*tasks, return_exceptions=True)
        except (OSError, RuntimeError, asyncio.TimeoutError, asyncio.IncompleteReadError) as error:
            code = str(error) if str(error) in ('VPN_DNS_REQUIRED', 'VPN_DNS', 'VPN_DOWN', 'VPN_DESTINATION') else 'VPN_DESTINATION'
            with contextlib.suppress(OSError):
                writer.write(('ERR ' + code + '\n').encode())
                await writer.drain()
        finally:
            self.active -= 1
            for value in (writer, remote_writer):
                if value:
                    self.writers.discard(value)
                    value.close()


async def main():
    os.umask(0o077)
    ROOT.mkdir(exist_ok=True)
    stream = asyncio.StreamReader(limit=LIMIT + 1)
    await asyncio.get_running_loop().connect_read_pipe(lambda: asyncio.StreamReaderProtocol(stream), sys.stdin)
    raw = await asyncio.wait_for(stream.readline(), 20)
    if len(raw) > LIMIT:
        raise RuntimeError('VPN_CONFIG')
    config = json.loads(raw)
    del raw
    (ROOT / 'client.ovpn').write_text(config.pop('config'))
    args = ['openvpn', '--config', str(ROOT / 'client.ovpn'), '--dev', 'tun0', '--dev-type', 'tun',
            '--route-noexec', '--tmp-dir', str(ROOT), '--script-security', '2', '--up', '/opt/buggy-vpn/up.py',
            '--down', '/opt/buggy-vpn/up.py', '--down-pre', '--up-restart', '--auth-nocache',
            '--auth-retry', 'none', '--connect-retry-max', '1', '--connect-timeout', '15',
            '--resolv-retry', '5', '--verb', '3', '--disable-dco', '--ping', '10', '--ping-exit', '40']
    if config.get('username'):
        (ROOT / 'auth').write_text(config.pop('username') + '\n' + config.pop('password', '') + '\n')
        args += ['--auth-user-pass', str(ROOT / 'auth')]
    if config.get('keyPassword'):
        (ROOT / 'key-pass').write_text(config.pop('keyPassword') + '\n')
        args += ['--askpass', str(ROOT / 'key-pass')]
    proc = await asyncio.create_subprocess_exec(*args, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT, stdin=asyncio.subprocess.DEVNULL)
    tunnel = Tunnel(config)
    server = None
    error_code = 'VPN_DISCONNECTED'

    async def logs():
        nonlocal server, error_code
        async for raw in proc.stdout:
            text = raw.decode(errors='replace')
            if 'AUTH_FAILED' in text or 'auth username' in text:
                error_code = 'VPN_AUTH'
            elif 'VERIFY ERROR' in text or 'certificate verify failed' in text:
                error_code = 'VPN_CERTIFICATE'
            elif 'Options error' in text or 'Cannot load' in text:
                error_code = 'VPN_CONFIG'
            if 'Initialization Sequence Completed' in text:
                tunnel.ready_routes()
                if server is None:
                    server = await asyncio.start_server(tunnel.handle, '0.0.0.0', PORT, limit=65536)
                emit(state='ready')
            elif 'Restart pause' in text or 'SIGUSR1' in text or 'SIGTERM' in text:
                tunnel.close_flows()
                emit(state='disconnected', code=error_code)
        await proc.wait()
        tunnel.close_flows()
        emit(state='disconnected', code=error_code)

    async def owner():
        # Parent app owns stdin. Heartbeats also bound orphan lifetime if a CLI
        # attachment is lost without promptly closing the container's input.
        while True:
            line = await asyncio.wait_for(stream.readline(), 30)
            if line != b'keepalive\n':
                return

    tasks = [asyncio.create_task(logs()), asyncio.create_task(owner())]
    try:
        done, _ = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
        for task in done:
            if not task.cancelled() and task.exception():
                emit(state='disconnected', code='VPN_RUNTIME')
    finally:
        tunnel.close_flows()
        if server:
            server.close()
            await server.wait_closed()
        if proc.returncode is None:
            proc.terminate()
            try:
                await asyncio.wait_for(proc.wait(), 3)
            except asyncio.TimeoutError:
                proc.kill()
                await proc.wait()
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)


if __name__ == '__main__':
    try:
        asyncio.run(main())
    except BaseException:
        # Never put configuration, credentials, endpoints or raw OpenVPN logs on stdout.
        emit(state='disconnected', code='VPN_RUNTIME')
        sys.exit(1)
