FROM alpine:3.23@sha256:85fe1e81d6758c208f3e1eed4338a1997e19d4be002d4dd32d3100c9a8c010a0
RUN apk add --no-cache openvpn iproute2 python3 py3-dnspython ca-certificates
COPY helper.py up.py /opt/buggy-vpn/
RUN chmod 555 /opt/buggy-vpn/up.py
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1
ENTRYPOINT ["python3", "-I", "/opt/buggy-vpn/helper.py"]
