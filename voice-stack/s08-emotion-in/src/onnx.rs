// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! One ONNX graph: Gemm of features[6] × W[6,3] + B[3].
//!
//! This is not a general ONNX runtime. Any other graph is an error, and the
//! caller falls back to the same coefficients in [`crate::map`]. The file is
//! `models/prosody-linear.onnx`, produced by `tools/gen_linear_onnx.py` from
//! those coefficients. Nothing is downloaded.

use crate::error::{EmotionError, EmotionErrorKind};
use std::sync::OnceLock;

const MODEL: &[u8] = include_bytes!("../models/prosody-linear.onnx");

#[derive(Clone, Copy)]
struct Head {
    w: [f32; 18],
    b: [f32; 3],
}

pub fn affine(x: &[f32; 6]) -> Result<[f32; 3], EmotionError> {
    let head = cached()?;
    let mut y = head.b;
    let mut i = 0;
    while i < 6 {
        let mut j = 0;
        while j < 3 {
            y[j] += x[i] * head.w[i * 3 + j];
            j += 1;
        }
        i += 1;
    }
    Ok(y)
}

fn cached() -> Result<Head, EmotionError> {
    static HEAD: OnceLock<Result<Head, EmotionError>> = OnceLock::new();
    *HEAD.get_or_init(|| parse(MODEL))
}

fn parse(bytes: &[u8]) -> Result<Head, EmotionError> {
    let mut model = Cursor::new(bytes);
    let mut graph: Option<&[u8]> = None;
    while model.remaining() > 0 {
        let (field, wire) = model.key()?;
        if field == 7 && wire == 2 {
            graph = Some(model.chunk()?);
        } else {
            model.skip(wire)?;
        }
    }
    let graph = graph.ok_or(bad("onnx model has no graph"))?;
    let mut g = Cursor::new(graph);
    let mut nodes: Vec<&[u8]> = Vec::new();
    let mut tensors: Vec<Tensor> = Vec::new();
    while g.remaining() > 0 {
        let (field, wire) = g.key()?;
        if field == 1 && wire == 2 {
            nodes.push(g.chunk()?);
        } else if field == 5 && wire == 2 {
            tensors.push(parse_tensor(g.chunk()?)?);
        } else {
            g.skip(wire)?;
        }
    }
    if nodes.len() != 1 {
        return Err(bad("onnx graph must contain one node"));
    }
    let node = parse_node(nodes[0])?;
    if node.op != "Gemm" {
        return Err(bad("onnx node must be Gemm"));
    }
    if node.inputs.len() != 3 {
        return Err(bad("onnx Gemm must have three inputs"));
    }
    let w = tensors
        .iter()
        .find(|t| t.name == node.inputs[1])
        .ok_or(bad("onnx W missing"))?;
    let b = tensors
        .iter()
        .find(|t| t.name == node.inputs[2])
        .ok_or(bad("onnx B missing"))?;
    if w.dims.as_slice() != [6, 3] || w.data.len() != 18 {
        return Err(bad("onnx W must be float [6, 3]"));
    }
    if b.data.len() != 3 || !(b.dims.as_slice() == [3] || b.dims.as_slice() == [1, 3]) {
        return Err(bad("onnx B must be float [3]"));
    }
    let mut head = Head {
        w: [0.0; 18],
        b: [0.0; 3],
    };
    head.w.copy_from_slice(&w.data);
    head.b.copy_from_slice(&b.data);
    Ok(head)
}

struct Tensor {
    name: String,
    dims: Vec<usize>,
    data: Vec<f32>,
}

struct Node {
    op: String,
    inputs: Vec<String>,
}

fn parse_node(bytes: &[u8]) -> Result<Node, EmotionError> {
    let mut c = Cursor::new(bytes);
    let mut op = String::new();
    let mut inputs = Vec::new();
    while c.remaining() > 0 {
        let (field, wire) = c.key()?;
        if field == 1 && wire == 2 {
            inputs.push(c.string()?);
        } else if field == 4 && wire == 2 {
            op = c.string()?;
        } else {
            c.skip(wire)?;
        }
    }
    Ok(Node { op, inputs })
}

fn parse_tensor(bytes: &[u8]) -> Result<Tensor, EmotionError> {
    let mut c = Cursor::new(bytes);
    let mut name = String::new();
    let mut dims = Vec::new();
    let mut data = Vec::new();
    let mut data_type = 0u32;
    while c.remaining() > 0 {
        let (field, wire) = c.key()?;
        match (field, wire) {
            (1, 0) => dims.push(c.varint()? as usize),
            (1, 2) => {
                let packed = c.chunk()?;
                let mut d = Cursor::new(packed);
                while d.remaining() > 0 {
                    dims.push(d.varint()? as usize);
                }
            }
            (2, 0) => data_type = c.varint()? as u32,
            (4, 5) => data.push(c.f32()?),
            (4, 2) | (9, 2) => {
                let raw = c.chunk()?;
                if raw.len() % 4 != 0 {
                    return Err(bad("onnx float payload is not a multiple of 4"));
                }
                for chunk in raw.chunks_exact(4) {
                    data.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
            }
            (8, 2) => name = c.string()?,
            _ => c.skip(wire)?,
        }
    }
    if data_type != 1 {
        return Err(bad("onnx tensor must be float32"));
    }
    if dims.len() != 1 && dims.len() != 2 {
        return Err(bad("onnx tensor rank must be 1 or 2"));
    }
    Ok(Tensor { name, dims, data })
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }
    fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }
    fn key(&mut self) -> Result<(u32, u32), EmotionError> {
        let k = self.varint()?;
        Ok(((k >> 3) as u32, (k & 7) as u32))
    }
    fn varint(&mut self) -> Result<u64, EmotionError> {
        let mut n = 0u64;
        let mut shift = 0;
        loop {
            if self.i >= self.b.len() || shift > 63 {
                return Err(bad("onnx varint overflow"));
            }
            let c = self.b[self.i];
            self.i += 1;
            n |= u64::from(c & 0x7F) << shift;
            if c & 0x80 == 0 {
                return Ok(n);
            }
            shift += 7;
        }
    }
    fn chunk(&mut self) -> Result<&'a [u8], EmotionError> {
        let n = self.varint()? as usize;
        if self.i + n > self.b.len() {
            return Err(bad("onnx length past end"));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn string(&mut self) -> Result<String, EmotionError> {
        let s = self.chunk()?;
        String::from_utf8(s.to_vec()).map_err(|_| bad("onnx string is not utf-8"))
    }
    fn f32(&mut self) -> Result<f32, EmotionError> {
        if self.i + 4 > self.b.len() {
            return Err(bad("onnx float past end"));
        }
        let x = f32::from_le_bytes([
            self.b[self.i],
            self.b[self.i + 1],
            self.b[self.i + 2],
            self.b[self.i + 3],
        ]);
        self.i += 4;
        Ok(x)
    }
    fn skip(&mut self, wire: u32) -> Result<(), EmotionError> {
        match wire {
            0 => {
                self.varint()?;
                Ok(())
            }
            1 => self.advance(8),
            2 => {
                let n = self.varint()? as usize;
                self.advance(n)
            }
            5 => self.advance(4),
            _ => Err(bad("onnx wire type is not supported")),
        }
    }
    fn advance(&mut self, n: usize) -> Result<(), EmotionError> {
        if self.i + n > self.b.len() {
            return Err(bad("onnx skip past end"));
        }
        self.i += n;
        Ok(())
    }
}

fn bad(detail: &'static str) -> EmotionError {
    EmotionError::new(EmotionErrorKind::Onnx, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map;

    #[test]
    fn gemm_matches_the_rust_coefficients() {
        let vectors = [
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.2, 0.4, 0.6, 0.1, 0.8, 0.3],
            [0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        ];
        for x in vectors {
            let y = affine(&x).unwrap();
            let expect = map::gemm(&x);
            for i in 0..3 {
                assert!(
                    (y[i] - expect[i]).abs() < 1e-5,
                    "{x:?} got {y:?} expect {expect:?}"
                );
            }
        }
    }
}
