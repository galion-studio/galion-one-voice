// Galion Voice — Perpetual Open MIT License
use crate::{Result, TensorError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shape(pub Vec<usize>);

impl Shape {
    pub fn new(dims: &[usize]) -> Self { Shape(dims.to_vec()) }
    pub fn dims(&self) -> &[usize] { &self.0 }
    pub fn rank(&self) -> usize { self.0.len() }
    pub fn numel(&self) -> usize { self.0.iter().product() }
    pub fn last(&self) -> usize { *self.0.last().unwrap_or(&1) }
    /// Row-major contiguous strides (in elements).
    pub fn contiguous_strides(&self) -> Vec<usize> {
        let mut s = vec![1usize; self.0.len()];
        for i in (0..self.0.len().saturating_sub(1)).rev() {
            s[i] = s[i + 1] * self.0[i + 1];
        }
        s
    }
    /// NumPy-style broadcast of two shapes.
    pub fn broadcast(&self, other: &Shape) -> Result<Shape> {
        let r = self.rank().max(other.rank());
        let mut out = vec![0; r];
        for i in 0..r {
            let a = if i < r - self.rank() { 1 } else { self.0[i - (r - self.rank())] };
            let b = if i < r - other.rank() { 1 } else { other.0[i - (r - other.rank())] };
            out[i] = match (a, b) {
                (x, y) if x == y => x,
                (1, y) => y,
                (x, 1) => x,
                _ => return Err(TensorError::ShapeMismatch { op: "broadcast", lhs: self.0.clone(), rhs: other.0.clone() }),
            };
        }
        Ok(Shape(out))
    }
}

impl From<&[usize]> for Shape { fn from(d: &[usize]) -> Self { Shape(d.to_vec()) } }
impl From<Vec<usize>> for Shape { fn from(d: Vec<usize>) -> Self { Shape(d) } }
impl<const N: usize> From<[usize; N]> for Shape { fn from(d: [usize; N]) -> Self { Shape(d.to_vec()) } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strides_and_broadcast() {
        let s = Shape::from([2, 3, 4]);
        assert_eq!(s.contiguous_strides(), vec![12, 4, 1]);
        assert_eq!(s.numel(), 24);
        assert_eq!(s.broadcast(&Shape::from([4])).unwrap(), s);
        assert_eq!(Shape::from([2, 1]).broadcast(&Shape::from([1, 5])).unwrap(), Shape::from([2, 5]));
        assert!(Shape::from([3]).broadcast(&Shape::from([4])).is_err());
    }
}
