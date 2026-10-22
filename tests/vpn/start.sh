#!/bin/sh
set -eu
cp /fixtures/server.crt /tmp/server.crt
cp /fixtures/server.key /tmp/server.key
chown postgres:postgres /tmp/server.crt /tmp/server.key
chmod 600 /tmp/server.key
openvpn --config "${VPN_SERVER_CONFIG:-/fixtures/server.conf}" --daemon --log /tmp/openvpn.log
dnsmasq --no-resolv --no-hosts --address=/vpn-db.test/10.77.0.1 --address=/vpn6-db.test/fd77::1 --local=/vpn6-db.test/ --user=root
exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key
