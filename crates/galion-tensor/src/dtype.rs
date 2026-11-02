// Galion Voice — Perpetual Open MIT License
//! Element types. f32 is the compute type; bf16 and i8 are storage placeholders.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    F32,
    /// Storage-only placeholder; ops upcast to f32.
    BF16,
    /// Placeholder for symmetric per-tensor quant (future `.gq` blocks will refine this).
    I8,
}

impl DType {
    pub const fn size_of(self) -> usize {
        match self {
            DType::F32 => 4,
            DType::BF16 => 2,
            DType::I8 => 1,
        }
    }
}

/// Minimal bfloat16 (upper 16 bits of an IEEE f32), round-to-nearest-even.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct Bf16(pub u16);

impl Bf16 {
    pub fn from_f32(x: f32) -> Self {
        let bits = x.to_bits();
        if x.is_nan() {
            return Bf16(((bits >> 16) as u16) | 0x0040); // quiet NaN
        }
        let lsb = (bits >> 16) & 1;
        let rounded = bits.wrapping_add(0x7FFF + lsb);
        Bf16((rounded >> 16) as u16)
    }
    pub fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bf16_roundtrip() {
        for &v in &[0.0f32, 1.0, -2.5, 3.140625, 65536.0, -0.0078125] {
            assert_eq!(Bf16::from_f32(v).to_f32(), v);
        }
        let x = 1.0f32 + 1.0 / 256.0; // not representable, rounds
        assert!((Bf16::from_f32(x).to_f32() - x).abs() < 1e-2);
        assert!(Bf16::from_f32(f32::NAN).to_f32().is_nan());
    }
}
