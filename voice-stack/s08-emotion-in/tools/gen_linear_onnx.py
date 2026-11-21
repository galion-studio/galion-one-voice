# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Write models/prosody-linear.onnx.

One Gemm: y = x @ W + b, x is 1x6, W is 6x3, b is 3.
The numbers are the coefficients in src/map.rs. This script does not
download anything and does not import an ONNX library. It writes the
protobuf wire format for that one graph.
"""

import struct
from pathlib import Path

W = [
    [0.50, 0.05, 0.40],
    [0.22, 0.10, 0.05],
    [0.22, 0.12, 0.15],
    [0.08, 0.55, 0.05],
    [-0.28, -0.70, -0.30],
    [0.00, 0.25, 0.00],
]
B = [0.05, -0.05, 0.25]


def varint(n: int) -> bytes:
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def key(field: int, wire: int) -> bytes:
    return varint((field << 3) | wire)


def ld(field: int, payload: bytes) -> bytes:
    return key(field, 2) + varint(len(payload)) + payload


def vi(field: int, n: int) -> bytes:
    return key(field, 0) + varint(n)


def tensor(name: str, dims: list[int], values: list[float]) -> bytes:
    dim_bytes = b"".join(varint(d) for d in dims)
    raw = b"".join(struct.pack("<f", v) for v in values)
    body = b""
    body += ld(1, dim_bytes)
    body += vi(2, 1)  # FLOAT
    body += ld(4, raw)  # packed float_data
    body += ld(8, name.encode())
    return body


def node() -> bytes:
    body = b""
    for name in (b"features", b"W", b"B"):
        body += ld(1, name)
    body += ld(2, b"y")
    body += ld(4, b"Gemm")
    return body


def dimension(v: int) -> bytes:
    return vi(1, v)


def shape(dims: list[int]) -> bytes:
    body = b""
    for d in dims:
        body += ld(1, dimension(d))
    return body


def value_info(name: str, dims: list[int]) -> bytes:
    tensor_type = vi(1, 1) + ld(2, shape(dims))  # elem_type FLOAT, shape
    type_proto = ld(1, tensor_type)
    return ld(1, name.encode()) + ld(2, type_proto)


def graph() -> bytes:
    flat = [w for row in W for w in row]
    body = b""
    body += ld(1, node())
    body += ld(2, b"prosody_linear")
    body += ld(5, tensor("W", [6, 3], flat))
    body += ld(5, tensor("B", [3], B))
    body += ld(11, value_info("features", [1, 6]))
    body += ld(12, value_info("y", [1, 3]))
    return body


def model() -> bytes:
    opset = ld(1, b"") + vi(2, 13)
    body = b""
    body += vi(1, 8)  # ir_version
    body += ld(2, b"galion-emotion-in")
    body += ld(6, b"Linear prosody head. Coefficients live in src/map.rs.")
    body += ld(7, graph())
    body += ld(8, opset)
    return body


def main() -> None:
    out = Path(__file__).resolve().parents[1] / "models" / "prosody-linear.onnx"
    out.parent.mkdir(parents=True, exist_ok=True)
    blob = model()
    out.write_bytes(blob)
    print(f"wrote {out} ({len(blob)} bytes)")


if __name__ == "__main__":
    main()
