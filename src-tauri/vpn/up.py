#!/usr/bin/python3
"""Trusted OpenVPN callback. All files are in the container's private tmpfs."""
import json
import os
from pathlib import Path

p = Path('/run/vpn/network.json')
if os.environ.get('script_type') == 'down':
    p.unlink(missing_ok=True)
else:
    dns = []
    for key, value in os.environ.items():
        words = value.split()
        if key.startswith('foreign_option_') and words[:2] in (
            ['dhcp-option', 'DNS'], ['dhcp-option', 'DNS6']
        ) and len(words) == 3:
            dns.append(words[2])
    p.with_suffix('.tmp').write_text(json.dumps({'dns': dns}))
    p.with_suffix('.tmp').replace(p)
