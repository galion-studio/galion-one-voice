# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Loopback session for the UI emotion feed.

The server checks every line before it writes. A line that is not a legal
live frame is not forwarded. The socket gets an explicit ``estimate: none``
instead, with ``reason: rejected``.

The client reconnects with a fixed backoff. The first wait is one 80 ms
heartbeat. Each further failure doubles the wait, and the wait stops growing
at 1280 ms. A dropped socket is reported as ``estimate: none`` with
``reason: disconnected``. It is not the last emotion, and it is not Neutral.

This process is not the live voice server. It binds loopback only.
"""

from __future__ import annotations

import json
import socket
import threading
import time
from typing import Callable

from .contract import absence_message, encode_ui, project_text, validate_ui
from .feed import DEFAULT_TAU_MS, HEARTBEAT_MS, FeedError
from .udp import local_host, parse_udp

BACKOFF_START_MS = HEARTBEAT_MS
BACKOFF_FACTOR = 2
BACKOFF_CAP_MS = HEARTBEAT_MS * 16


def backoff_ms(attempt: int) -> int:
    """Delay before the next connect, in milliseconds.

    ``attempt`` 0 waits one heartbeat (80 ms). The wait doubles each time and
    does not grow past 16 heartbeats (1280 ms).
    """
    if isinstance(attempt, bool) or not isinstance(attempt, int) or attempt < 0:
        raise FeedError("backoff attempt must be an integer >= 0")
    delay = BACKOFF_START_MS * (BACKOFF_FACTOR ** attempt)
    if delay > BACKOFF_CAP_MS:
        return BACKOFF_CAP_MS
    return delay


class FeedServer:
    """One loopback TCP listener. Each accepted socket gets newline-delimited JSON."""

    def __init__(self, host: str, port: int) -> None:
        ip, family = local_host(host)
        if isinstance(port, bool) or not isinstance(port, int) or not 0 <= port <= 65535:
            raise FeedError("TCP port is outside 0..65535")
        self.host = ip
        sock = socket.socket(family, socket.SOCK_STREAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind((ip, port))
        sock.listen(4)
        sock.settimeout(0.2)
        self._sock = sock
        self.port = sock.getsockname()[1]
        self._clients: list[socket.socket] = []
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._accept_loop, name="nebula-feed", daemon=True)
        self._seq = 0
        self._started = False

    def start(self) -> None:
        if self._started:
            return
        self._started = True
        self._thread.start()

    def client_count(self) -> int:
        with self._lock:
            return len(self._clients)

    def wait_clients(self, count: int, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.client_count() >= count:
                return
            time.sleep(0.01)
        raise FeedError(f"wanted {count} UI clients, have {self.client_count()}")

    def accept_text(self, line: str) -> dict[str, object]:
        """Validate one live-feed line and send the UI message.

        A bad line still sends ``estimate: none`` / ``reason: rejected``, then
        raises ``FeedError``. The bad weights are not on the socket.
        """
        with self._lock:
            self._seq += 1
            seq = self._seq
        message, error = interpret_line(line, seq)
        self.send(message)
        if error is not None:
            raise error
        return message

    def send(self, message: dict[str, object]) -> None:
        """Send one message that already passes :func:`encode_ui`."""
        self._broadcast(message)

    def _broadcast(self, message: dict[str, object]) -> None:
        payload = (encode_ui(message) + "\n").encode("utf-8")
        dead: list[socket.socket] = []
        with self._lock:
            clients = list(self._clients)
        for client in clients:
            try:
                client.sendall(payload)
            except OSError:
                dead.append(client)
        if dead:
            with self._lock:
                for client in dead:
                    if client in self._clients:
                        self._clients.remove(client)
                    _close(client)

    def close(self) -> None:
        self._stop.set()
        _close(self._sock)
        with self._lock:
            clients = list(self._clients)
            self._clients.clear()
        for client in clients:
            _close(client)
        if self._started:
            self._thread.join(timeout=2)

    def _accept_loop(self) -> None:
        while not self._stop.is_set():
            try:
                conn, _addr = self._sock.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            conn.settimeout(2.0)
            with self._lock:
                self._clients.append(conn)


class FeedClient:
    """Reconnect to a :class:`FeedServer` and surface an absence on every gap."""

    def __init__(
        self,
        host: str,
        port: int,
        *,
        sleeper: Callable[[float], None] | None = None,
        timeout_s: float = 2.0,
    ) -> None:
        ip, _family = local_host(host)
        if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
            raise FeedError("TCP port is outside 1..65535")
        self.host = ip
        self.port = port
        self.timeout_s = timeout_s
        self._sleeper = time.sleep if sleeper is None else sleeper
        self.attempt = 0
        self._sock: socket.socket | None = None
        self._buf = bytearray()
        self.voice_id: str | None = None
        self.t_ms = 0
        self.kind = "frame"
        self.tau_ms = DEFAULT_TAU_MS

    def open_attempt(self) -> bool:
        """Try once. On failure, wait the current backoff and return False."""
        self.close_socket()
        try:
            sock = socket.create_connection((self.host, self.port), timeout=self.timeout_s)
        except OSError:
            delay = backoff_ms(self.attempt)
            self.attempt += 1
            self._sleeper(delay / 1000.0)
            return False
        sock.settimeout(self.timeout_s)
        self._sock = sock
        self._buf.clear()
        self.attempt = 0
        return True

    def read(self) -> dict[str, object] | None:
        """Read one validated UI message. ``None`` means the socket closed."""
        sock = self._sock
        if sock is None:
            raise FeedError("client is not connected")
        while b"\n" not in self._buf:
            try:
                chunk = sock.recv(4096)
            except socket.timeout:
                raise
            except OSError:
                return None
            if not chunk:
                return None
            self._buf.extend(chunk)
        raw, _, rest = bytes(self._buf).partition(b"\n")
        self._buf = bytearray(rest)
        text = raw.decode("utf-8")
        try:
            payload = json.loads(text)
        except json.JSONDecodeError as exc:
            raise FeedError(f"UI line is not JSON: {exc}") from exc
        checked = validate_ui(payload)
        if checked.get("origin") == "server":
            voice = checked.get("voice_id")
            if isinstance(voice, str):
                self.voice_id = voice
            stamp = checked.get("t_ms")
            if isinstance(stamp, int) and not isinstance(stamp, bool):
                self.t_ms = stamp
            kind = checked.get("kind")
            if kind in ("frame", "utterance"):
                self.kind = kind
            tau = checked.get("tau_ms")
            if isinstance(tau, (int, float)) and not isinstance(tau, bool):
                self.tau_ms = float(tau)
        return checked

    def gap_message(self) -> dict[str, object]:
        """Local absence after a failed connect or a dropped socket."""
        return absence_message(
            voice_id=self.voice_id,
            t_ms=self.t_ms,
            kind=self.kind,
            tau_ms=self.tau_ms,
            seq=None,
            origin="client",
            reason="disconnected",
            detail=None,
        )

    def pump(self, output: object, *, frames: int | None) -> None:
        """Write UI lines until ``frames`` server messages, or forever when ``frames`` is None."""
        seen = 0
        while frames is None or seen < frames:
            if not self.open_attempt():
                output.write(encode_ui(self.gap_message()) + "\n")
                output.flush()
                continue
            while frames is None or seen < frames:
                message = self.read()
                if message is None:
                    output.write(encode_ui(self.gap_message()) + "\n")
                    output.flush()
                    break
                output.write(encode_ui(message) + "\n")
                output.flush()
                if message.get("origin") == "server":
                    seen += 1
            self.close_socket()

    def close_socket(self) -> None:
        if self._sock is not None:
            _close(self._sock)
            self._sock = None

    def close(self) -> None:
        self.close_socket()


def interpret_line(line: str, seq: int) -> tuple[dict[str, object], FeedError | None]:
    """Project one live-feed line, or an explicit rejection. Nothing is sent."""
    try:
        return project_text(line, seq=seq), None
    except FeedError as exc:
        voice, stamp, kind, tau = _hint(line)
        message = absence_message(
            voice_id=voice,
            t_ms=stamp,
            kind=kind,
            tau_ms=tau,
            seq=seq,
            origin="server",
            reason="rejected",
            detail=str(exc),
        )
        return message, exc


def _hint(line: str) -> tuple[str | None, int, str, float]:
    """Pull a voice and a clock off a bad line, without trusting its weights."""
    voice: str | None = None
    stamp = 0
    kind = "frame"
    tau = DEFAULT_TAU_MS
    try:
        raw = json.loads(line)
    except json.JSONDecodeError:
        return voice, stamp, kind, tau
    if not isinstance(raw, dict):
        return voice, stamp, kind, tau
    candidate = raw.get("voice_id")
    if isinstance(candidate, str) and candidate.strip().lower() in ("samantha", "jarvis"):
        voice = candidate.strip().lower()
    t_ms = raw.get("t_ms")
    if isinstance(t_ms, int) and not isinstance(t_ms, bool) and t_ms >= 0:
        stamp = t_ms
    if raw.get("kind") in ("frame", "utterance"):
        kind = raw["kind"]
    raw_tau = raw.get("tau_ms")
    if isinstance(raw_tau, (int, float)) and not isinstance(raw_tau, bool) and raw_tau > 0:
        tau = float(raw_tau)
    return voice, stamp, kind, tau


def _close(sock: socket.socket) -> None:
    try:
        sock.close()
    except OSError:
        return


def parse_tcp(spec: str) -> tuple[str, int]:
    """Parse ``127.0.0.1:8766``. The host must be loopback."""
    try:
        return parse_udp(spec)
    except FeedError as exc:
        raise FeedError(str(exc).replace("UDP", "TCP")) from exc
