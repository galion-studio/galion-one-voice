# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Send one live frame per datagram to a loopback UDP port.

The visualiser binds. This process only sends, and only to a loopback
address. A frame that does not fit in one datagram is refused.
"""

from __future__ import annotations

import ipaddress
import socket

from .feed import FeedError

MAX_DATAGRAM = 4096
DEFAULT_UDP = "127.0.0.1:8765"


def local_host(host: str) -> tuple[str, socket.AddressFamily]:
    """Accept a loopback address and return the IP plus its socket family."""
    if not isinstance(host, str) or not host.strip():
        raise FeedError("UDP host must be a loopback address")
    name = host.strip()
    if name.lower() == "localhost":
        name = "127.0.0.1"
    try:
        address = ipaddress.ip_address(name)
    except ValueError as exc:
        raise FeedError(f"UDP host {host!r} is not a loopback address") from exc
    if not address.is_loopback:
        raise FeedError("nebula feed UDP is local only")
    family = socket.AF_INET6 if address.version == 6 else socket.AF_INET
    return str(address), family


def parse_udp(spec: str) -> tuple[str, int]:
    """Parse ``127.0.0.1:8765`` or ``[::1]:8765``."""
    if not isinstance(spec, str) or not spec.strip():
        raise FeedError("UDP address must be host:port")
    text = spec.strip()
    if text.startswith("["):
        host, separator, rest = text[1:].partition("]")
        if separator != "]" or not rest.startswith(":"):
            raise FeedError("UDP address must be [host]:port")
        port_text = rest[1:]
    else:
        host, separator, port_text = text.rpartition(":")
        if separator != ":":
            raise FeedError("UDP address must be host:port")
    if not port_text.isdigit():
        raise FeedError("UDP port must be an integer")
    port = int(port_text)
    if not 1 <= port <= 65535:
        raise FeedError("UDP port is outside 1..65535")
    local_host(host)
    return host, port


class UdpSink:
    """One JSON object per datagram, without a trailing newline."""

    def __init__(self, host: str, port: int) -> None:
        ip, family = local_host(host)
        if not isinstance(port, int) or isinstance(port, bool) or not 1 <= port <= 65535:
            raise FeedError("UDP port is outside 1..65535")
        self.address = (ip, port)
        self._socket = socket.socket(family, socket.SOCK_DGRAM)

    def write(self, line: str) -> None:
        payload = line.encode("utf-8")
        if len(payload) > MAX_DATAGRAM:
            raise FeedError(
                f"frame is {len(payload)} bytes; local UDP limit is {MAX_DATAGRAM}"
            )
        self._socket.sendto(payload, self.address)

    def close(self) -> None:
        self._socket.close()
