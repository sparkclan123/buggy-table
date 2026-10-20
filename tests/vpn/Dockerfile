FROM postgres:17-alpine
RUN apk add --no-cache openvpn dnsmasq
COPY start.sh /usr/local/bin/vpn-fixture
RUN chmod +x /usr/local/bin/vpn-fixture
ENTRYPOINT ["vpn-fixture"]
