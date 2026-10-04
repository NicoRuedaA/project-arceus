//! Checked numerical port of update-main's framed rotation paths.
//!
//! This is NOT a player or the importer reference. The caller must supply the
//! imported quaternion epsilon and a controlled FPCR profile. Actual game FPCR,
//! reached FP context, clocks and looping remain unresolved. The update SDK
//! constant is statically qualified, but no runtime default is assumed.
//! See `reports/skeleton/update-v262144-native-animation-special-values.md`
//! (dec103), the dec104 FPSR and dec105 ISA reports, and the historical dec102 report.

mod cache;
pub use cache::TrAnmNativeFramedRotationCache;
mod flags;
use flags::{FloatStatus, HostStatusGuard};

use super::{TrAnmChannel, TrAnmEncoding, TrAnmKey};

/// Conditions outside the checked, explicitly controlled numerical contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrAnmNativeError {
    UnsupportedFpcr,
    UnsupportedFpsrState,
    UnsupportedHostFloatingEnvironment,
    InvalidEpsilon,
    UnsupportedEncoding,
    InvalidGuardRecords,
    TooManyRecords,
    InvalidFrameOrder,
    FrameWidthExceeded,
    NonfinitePosition,
    PositionOutsideGuards,
}

/// Controlled numerical result, independent of the host's exception flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrAnmNativeEvaluation {
    pub rotation: [f32; 4],
    /// Initial FPSR OR cumulative native exception flags. Non-exception bits
    /// (QC) are retained, not interpreted; reserved bits are rejected. This
    /// path has no QC-writing instructions.
    pub fpsr: u32,
}

/// Explicit numerical context, not a claim about the game's runtime settings.
///
/// Only FPCR=0, positive finite epsilon and a checked x86_64/AArch64 host
/// round-to-nearest/no-flush environment are supported. No default epsilon is
/// provided. Numerical output and cumulative FPSR flags are modeled for FPCR0;
/// All public evaluation wrappers restore incoming MXCSR (x86_64) or FPSR
/// (AArch64) bits on return/unwind. AArch64 host code is not cross-build verified.
/// This is a partial port; see the dec104/dec105 reports for proof limitations.
#[derive(Debug, Clone, Copy)]
pub struct TrAnmNativeContext {
    quaternion_epsilon: f32,
}

impl TrAnmNativeContext {
    pub fn new(fpcr: u64, quaternion_epsilon: f32) -> Result<Self, TrAnmNativeError> {
        if fpcr != 0 {
            return Err(TrAnmNativeError::UnsupportedFpcr);
        }
        check_host_environment()?;
        let _host_status = HostStatusGuard::new();
        if !quaternion_epsilon.is_finite() || quaternion_epsilon <= 0.0 {
            return Err(TrAnmNativeError::InvalidEpsilon);
        }
        Ok(Self { quaternion_epsilon })
    }

    /// Packed decode required by the mapped framed path; no host square root.
    pub fn unpack_rotation(self, words: [u16; 3]) -> Result<[f32; 4], TrAnmNativeError> {
        Ok(self.unpack_rotation_with_fpsr(words, 0)?.rotation)
    }

    pub fn unpack_rotation_with_fpsr(
        self,
        words: [u16; 3],
        initial_fpsr: u32,
    ) -> Result<TrAnmNativeEvaluation, TrAnmNativeError> {
        if initial_fpsr & !0x0800_009f != 0 {
            return Err(TrAnmNativeError::UnsupportedFpsrState);
        }
        check_host_environment()?;
        let _host_status = HostStatusGuard::new();
        let status = FloatStatus::new(initial_fpsr);
        Ok(TrAnmNativeEvaluation {
            rotation: unpack_rotation(words, &status),
            fpsr: status.bits(),
        })
    }
}

/// Borrowed checked view for native framed-u16/u8 numerical evaluation.
///
/// Requires two repeated boundary frames and strictly increasing interior
/// frames, matching the observed guard layout. Guard VALUES are not collapsed
/// or required to match. Out-of-range/nonfinite positions and malformed data
/// are rejected safely, not claimed to be native caller guarantees.
#[derive(Debug)]
pub struct TrAnmNativeFramedRotation<'a> {
    keys: &'a [TrAnmKey<[u16; 3]>],
}

impl<'a> TrAnmNativeFramedRotation<'a> {
    pub fn new(channel: &'a TrAnmChannel<[u16; 3]>) -> Result<Self, TrAnmNativeError> {
        let max_frame = match channel.encoding {
            TrAnmEncoding::Framed16 => u16::MAX as u32,
            TrAnmEncoding::Framed8 => u8::MAX as u32,
            _ => return Err(TrAnmNativeError::UnsupportedEncoding),
        };
        let keys = &channel.keys;
        if keys.len() > 1_000_000 {
            return Err(TrAnmNativeError::TooManyRecords);
        }
        if keys.len() < 4
            || keys[0].frame != keys[1].frame
            || keys[keys.len() - 2].frame != keys[keys.len() - 1].frame
        {
            return Err(TrAnmNativeError::InvalidGuardRecords);
        }
        if keys.iter().any(|k| k.frame > max_frame) {
            return Err(TrAnmNativeError::FrameWidthExceeded);
        }
        if keys[1..keys.len() - 1]
            .windows(2)
            .any(|p| p[0].frame >= p[1].frame)
        {
            return Err(TrAnmNativeError::InvalidFrameOrder);
        }
        Ok(Self { keys })
    }

    /// Mapped to stateless entries 027b6c08 (u16) and 027b7924 (u8).
    ///
    /// Endpoint returns bypass normalization. Interior interpolation uses four
    /// decoded controls, unfused f32 cubic arithmetic and ARM reciprocal-root
    /// estimate/refinement, with the supplied epsilon's strict comparison.
    pub fn evaluate(
        &self,
        position: f32,
        context: TrAnmNativeContext,
    ) -> Result<[f32; 4], TrAnmNativeError> {
        Ok(self.evaluate_with_fpsr(position, context, 0)?.rotation)
    }

    /// The same checked path, with cumulative FPSR state under FPCR0.
    /// Incoming host FP status is restored on return or unwind; controls are
    /// checked read-only and never changed. No native traps are modeled.
    pub fn evaluate_with_fpsr(
        &self,
        position: f32,
        context: TrAnmNativeContext,
        initial_fpsr: u32,
    ) -> Result<TrAnmNativeEvaluation, TrAnmNativeError> {
        if initial_fpsr & !0x0800_009f != 0 {
            return Err(TrAnmNativeError::UnsupportedFpsrState);
        }
        check_host_environment()?;
        let _host_status = HostStatusGuard::new();
        let status = FloatStatus::new(initial_fpsr);
        if !position.is_finite() {
            return Err(TrAnmNativeError::NonfinitePosition);
        }
        if position < self.keys[1].frame as f32
            || position > self.keys[self.keys.len() - 2].frame as f32
        {
            return Err(TrAnmNativeError::PositionOutsideGuards);
        }
        // The native search starts at index TWO, excluding the final guard.
        // Starting at one would divide by zero at the duplicated first frame.
        let mut start = 2;
        let mut count = self.keys.len() - 3;
        while count != 0 {
            let half = count / 2;
            let mid = start + half;
            if (self.keys[mid].frame as f32) < position {
                start = mid + 1;
                count -= half + 1;
            } else {
                count = half;
            }
        }
        let k = start;
        // All frame and 15-bit field conversions are exact in binary32.
        // Native FSUB/FDIV precede the first decode, even at endpoints.
        let alpha = status.div(
            status.sub(position, self.keys[k - 1].frame as f32),
            (self.keys[k].frame - self.keys[k - 1].frame) as f32,
        );
        let a = unpack_rotation(self.keys[k - 1].value, &status);
        if alpha <= 0.0 {
            return Ok(TrAnmNativeEvaluation {
                rotation: a,
                fpsr: status.bits(),
            });
        }
        let b = unpack_rotation(self.keys[k].value, &status);
        if alpha >= 1.0 {
            return Ok(TrAnmNativeEvaluation {
                rotation: b,
                fpsr: status.bits(),
            });
        }
        let previous = unpack_rotation(self.keys[k - 2].value, &status);
        let next = unpack_rotation(self.keys[k + 1].value, &status);
        let mut result = [0.0; 4];
        for lane in 0..4 {
            let t0 = status.mul(status.sub(b[lane], previous[lane]), 0.5);
            let t1 = status.mul(status.sub(next[lane], a[lane]), 0.5);
            let cubic = status.add(
                status.sub(status.add(a[lane], a[lane]), status.add(b[lane], b[lane])),
                t0,
            );
            let cubic = status.add(cubic, t1);
            let quadratic = status.add(status.mul(a[lane], -3.0), status.mul(b[lane], 3.0));
            let quadratic = status.sub(quadratic, status.add(t0, t0));
            let quadratic = status.sub(quadratic, t1);
            let v = status.add(quadratic, status.mul(cubic, alpha));
            let v = status.add(t0, status.mul(v, alpha));
            result[lane] = status.add(a[lane], status.mul(v, alpha));
        }
        let square = result.map(|x| status.mul(x, x));
        // Native vector FADD's first operands are Z/W, not X/Y. This matters
        // when more than one lane has a NaN with a different sign/payload.
        let norm = status.add(
            status.add(square[2], square[0]),
            status.add(square[3], square[1]),
        );
        let mut reciprocal = status.estimate(norm);
        reciprocal = status.mul(
            reciprocal,
            status.step(reciprocal, status.mul(reciprocal, norm)),
        );
        reciprocal = status.mul(
            reciprocal,
            status.step(reciprocal, status.mul(norm, reciprocal)),
        );
        // Native FMUL uses the reciprocal as its FIRST operand. Do not commute
        // it: an unordered norm makes that NaN determine every output lane.
        let normalized = result.map(|x| status.mul(reciprocal, x));
        let rotation = if status.greater(context.quaternion_epsilon, norm) {
            [0.0; 4]
        } else {
            normalized
        };
        Ok(TrAnmNativeEvaluation {
            rotation,
            fpsr: status.bits(),
        })
    }
}

fn unpack_rotation(words: [u16; 3], status: &FloatStatus) -> [f32; 4] {
    let packed = u64::from(words[0]) | (u64::from(words[1]) << 16) | (u64::from(words[2]) << 32);
    let c = [3, 18, 33].map(|shift| {
        status.fma(
            ((packed >> shift) & 0x7fff) as f32,
            f32::from_bits(0x3849_116d),
            f32::from_bits(0xbf49_0fdb),
        )
    });
    let square = c.map(|x| status.mul(x, x));
    let radicand = status.max_number(
        status.sub(
            1.0,
            status.add(status.add(square[0], square[1]), status.add(square[2], 0.0)),
        ),
        0.0,
    );
    // Native FMAXNM clamps the radicand, but FRSQRTE(0)*0 then produces its
    // default NaN. Do not substitute the importer's finite sqrt(max(r,0)).
    let root0 = status.mul(status.estimate(radicand), radicand);
    let adjusted = status.sub(
        radicand,
        status.mul(status.sub(status.mul(root0, root0), radicand), 0.5),
    );
    let root1 = status.mul(status.estimate(adjusted), adjusted);
    let corrected = status.sub(
        adjusted,
        status.mul(status.sub(status.mul(root1, root1), radicand), 0.5),
    );
    let root = status.mul(status.estimate(corrected), corrected);
    let omitted = (packed & 3) as usize;
    let mut result = [0.0; 4];
    let mut component = 0;
    for (lane, value) in result.iter_mut().enumerate() {
        *value = if lane == omitted {
            root
        } else {
            let value = c[component];
            component += 1;
            value
        };
        if packed & 4 != 0 {
            *value = f32::from_bits(value.to_bits() ^ 0x8000_0000);
        }
    }
    result
}

// Clean-room integer implementation of Arm DDI0596 ID121321 pp3207–3209,
// FPRSqrtEstimate/RecipSqrtEstimate (ordinary 8-bit precision, FPCR=0).
// Exceptional outputs are explicit; the finite integer loop is bounded by 512 steps.
fn reciprocal_sqrt_estimate(value: f32) -> f32 {
    let bits = value.to_bits();
    if value.is_nan() {
        return quiet_nan(value);
    }
    if bits & 0x7fff_ffff == 0 {
        return f32::from_bits((bits & 0x8000_0000) | 0x7f80_0000);
    }
    if bits & 0x8000_0000 != 0 {
        return default_nan();
    }
    if value.is_infinite() {
        return 0.0;
    }
    let mut exponent = ((bits >> 23) & 255) as i32;
    let mut fraction = u64::from(bits & 0x7f_ffff) << 29;
    if exponent == 0 {
        while fraction & (1 << 51) == 0 {
            fraction <<= 1;
            exponent -= 1;
        }
        fraction = (fraction & ((1 << 51) - 1)) << 1;
    }
    let bucket = if exponent & 1 == 0 {
        256 | ((fraction >> 44) as u32)
    } else {
        128 | ((fraction >> 45) as u32)
    };
    let midpoint = if bucket < 256 {
        bucket * 2 + 1
    } else {
        ((bucket & !1) + 1) * 2
    };
    let mut candidate = 512_u64;
    while u64::from(midpoint) * (candidate + 1) * (candidate + 1) < 1 << 28 {
        candidate += 1;
    }
    let estimate = candidate.div_ceil(2);
    f32::from_bits((((380 - exponent) / 2) as u32) << 23 | ((estimate as u32 & 255) << 15))
}

fn reciprocal_sqrt_step(estimate: f32, product: f32) -> f32 {
    // The instruction negates operand ONE before selecting/quieting NaNs.
    let negated = f32::from_bits(estimate.to_bits() ^ 0x8000_0000);
    if let Some(nan) = operand_nan(negated, product) {
        return nan;
    }
    if (estimate.is_infinite() && product == 0.0) || (estimate == 0.0 && product.is_infinite()) {
        return 1.5;
    }
    if estimate.is_infinite() || product.is_infinite() {
        return f32::from_bits(
            ((negated.to_bits() ^ product.to_bits()) & 0x8000_0000) | 0x7f80_0000,
        );
    }
    // Halving this positive normal estimate is exact. Moving that exact scale
    // into the multiplicand implements the ISA's fully fused add AND halve,
    // not a rounded multiply/subtract followed by a divide.
    (-estimate * 0.5).mul_add(product, 1.5)
}

// FPCR=0 output semantics only, derived from Arm FPProcessNaNs, FPAdd, FPSub,
// FPMul and FPDefaultNaN. Host arithmetic must never choose the NaN operand,
// sign or payload; flags are accounted separately by the local software model; traps are unported.
fn default_nan() -> f32 {
    f32::from_bits(0x7fc0_0000)
}

fn quiet_nan(value: f32) -> f32 {
    f32::from_bits(value.to_bits() | 0x0040_0000)
}

fn operand_nan(first: f32, second: f32) -> Option<f32> {
    for value in [first, second] {
        if value.is_nan() && value.to_bits() & 0x0040_0000 == 0 {
            return Some(quiet_nan(value));
        }
    }
    [first, second].into_iter().find(|value| value.is_nan())
}

fn arm_add(first: f32, second: f32) -> f32 {
    if let Some(nan) = operand_nan(first, second) {
        return nan;
    }
    let value = first + second;
    if value.is_nan() {
        default_nan()
    } else {
        value
    }
}

fn arm_sub(first: f32, second: f32) -> f32 {
    // Unlike FNEG, FPSub selects NaNs BEFORE subtracting finite values.
    if let Some(nan) = operand_nan(first, second) {
        return nan;
    }
    let value = first - second;
    if value.is_nan() {
        default_nan()
    } else {
        value
    }
}

fn arm_mul(first: f32, second: f32) -> f32 {
    if let Some(nan) = operand_nan(first, second) {
        return nan;
    }
    let value = first * second;
    if value.is_nan() {
        default_nan()
    } else {
        value
    }
}

fn check_host_environment() -> Result<(), TrAnmNativeError> {
    #[cfg(target_arch = "x86_64")]
    {
        let mut mxcsr = 0_u32;
        // SAFETY: STMXCSR only writes four bytes to this valid local. It does
        // not change control registers, global configuration or rounding mode.
        unsafe {
            std::arch::asm!("stmxcsr [{ptr}]", ptr = in(reg) &mut mxcsr, options(nostack, preserves_flags));
        }
        // RNE, no FTZ/DAZ, all exceptions masked. Status flags are irrelevant.
        if mxcsr & 0xffc0 == 0x1f80 {
            return Ok(());
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        let fpcr: u64;
        // SAFETY: Read-only architectural register access in user mode.
        unsafe {
            std::arch::asm!("mrs {value}, fpcr", value = out(reg) fpcr, options(nomem, nostack, preserves_flags));
        }
        if fpcr == 0 {
            return Ok(());
        }
    }
    Err(TrAnmNativeError::UnsupportedHostFloatingEnvironment)
}

#[cfg(test)]
mod special_value_tests {
    use super::*;

    #[test]
    fn isa_nan_operand_priority_and_sign_are_not_host_choices() {
        let first = f32::from_bits(0xffc0_0123);
        let second = f32::from_bits(0x7fc0_0456);
        let signaling = f32::from_bits(0x7f80_0789);
        for op in [arm_add, arm_sub, arm_mul] {
            assert_eq!(op(first, second).to_bits(), first.to_bits());
            assert_eq!(op(1.0, first).to_bits(), first.to_bits());
            assert_eq!(op(first, signaling).to_bits(), 0x7fc0_0789);
        }
        assert_eq!(arm_mul(0.0, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(arm_sub(f32::INFINITY, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(reciprocal_sqrt_step(first, second).to_bits(), 0x7fc0_0123);
    }

    #[test]
    fn isa_reciprocal_root_exceptional_outputs() {
        assert_eq!(reciprocal_sqrt_estimate(0.0), f32::INFINITY);
        assert_eq!(reciprocal_sqrt_estimate(-0.0), f32::NEG_INFINITY);
        assert_eq!(reciprocal_sqrt_estimate(f32::INFINITY).to_bits(), 0);
        assert_eq!(reciprocal_sqrt_estimate(-1.0).to_bits(), 0x7fc0_0000);
        assert_eq!(
            reciprocal_sqrt_estimate(f32::from_bits(0xff80_0123)).to_bits(),
            0xffc0_0123
        );
        assert_eq!(reciprocal_sqrt_step(f32::INFINITY, 0.0), 1.5);
        assert_eq!(reciprocal_sqrt_step(0.0, f32::INFINITY), 1.5);
    }
}
