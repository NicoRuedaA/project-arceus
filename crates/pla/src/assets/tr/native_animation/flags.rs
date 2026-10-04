//! Local FPCR0 exception accounting. No host status is used as ARM evidence.
use super::{arm_add, arm_mul, arm_sub, reciprocal_sqrt_estimate, reciprocal_sqrt_step};
use std::cell::Cell;

const IOC: u32 = 1;
const DZC: u32 = 2;
const OFC: u32 = 4;
const UFC: u32 = 8;
const IXC: u32 = 16;

pub(super) struct FloatStatus(Cell<u32>);
impl FloatStatus {
    pub(super) fn new(initial: u32) -> Self {
        Self(Cell::new(initial))
    }
    pub(super) fn bits(&self) -> u32 {
        self.0.get()
    }
    fn raise(&self, flags: u32) {
        self.0.set(self.bits() | flags);
    }
    fn signaling(&self, inputs: &[f32]) {
        if inputs
            .iter()
            .any(|x| x.is_nan() && x.to_bits() & 0x0040_0000 == 0)
        {
            self.raise(IOC);
        }
    }
    fn rounded(&self, exact: Exact, output: f32) {
        if output.is_infinite() {
            self.raise(OFC | IXC);
        } else if !exact.same_value(Exact::float(output)) {
            // Arm FPRoundBase: tininess BEFORE rounding, FPCR.AH=0/UFE=0.
            self.raise(
                IXC | if exact.abs_less(Exact::power(-126)) {
                    UFC
                } else {
                    0
                },
            );
        }
    }
    pub(super) fn add(&self, a: f32, b: f32) -> f32 {
        let out = arm_add(a, b);
        self.signaling(&[a, b]);
        if a.is_finite() && b.is_finite() {
            self.rounded(Exact::float(a).add(Exact::float(b)), out);
        } else if !a.is_nan() && !b.is_nan() && out.is_nan() {
            self.raise(IOC);
        }
        out
    }
    pub(super) fn sub(&self, a: f32, b: f32) -> f32 {
        let out = arm_sub(a, b);
        self.signaling(&[a, b]);
        if a.is_finite() && b.is_finite() {
            self.rounded(Exact::float(a).add(Exact::float(b).neg()), out);
        } else if !a.is_nan() && !b.is_nan() && out.is_nan() {
            self.raise(IOC);
        }
        out
    }
    pub(super) fn mul(&self, a: f32, b: f32) -> f32 {
        let out = arm_mul(a, b);
        self.signaling(&[a, b]);
        if a.is_finite() && b.is_finite() {
            self.rounded(Exact::product(a, b), out);
        } else if !a.is_nan() && !b.is_nan() && out.is_nan() {
            self.raise(IOC);
        }
        out
    }
    pub(super) fn div(&self, a: f32, b: f32) -> f32 {
        // Only finite numerator/nonzero positive integral denominator is
        // reachable after the checked framed-channel/position boundary.
        debug_assert!(a.is_finite() && b.is_finite() && b > 0.0);
        let out = a / b;
        if out.is_infinite() {
            self.raise(OFC | IXC);
        } else if !Exact::product(out, b).same_value(Exact::float(a)) {
            let tiny = Exact::float(a).abs_less(Exact::float(b).scale(-126));
            self.raise(IXC | if tiny { UFC } else { 0 });
        }
        out
    }
    pub(super) fn fma(&self, a: f32, b: f32, c: f32) -> f32 {
        // Native packed expansion operands are all finite; this method is
        // private and deliberately not a general-purpose IEEE FMA API.
        debug_assert!(a.is_finite() && b.is_finite() && c.is_finite());
        let out = a.mul_add(b, c);
        self.rounded(Exact::product(a, b).add(Exact::float(c)), out);
        out
    }
    pub(super) fn estimate(&self, a: f32) -> f32 {
        self.signaling(&[a]);
        if a.to_bits() & 0x7fff_ffff == 0 {
            self.raise(DZC);
        } else if !a.is_nan() && a.is_sign_negative() {
            self.raise(IOC);
        }
        // The finite estimate is approximate but does not call FPRound and
        // therefore does NOT generate IXC. FPCR0 does not flush input denorms.
        reciprocal_sqrt_estimate(a)
    }
    pub(super) fn step(&self, a: f32, b: f32) -> f32 {
        self.signaling(&[a, b]);
        let out = reciprocal_sqrt_step(a, b);
        if a.is_finite() && b.is_finite() {
            let exact = Exact::float(3.0).add(Exact::product(a, b).neg()).scale(-1);
            self.rounded(exact, out);
        }
        // Inf*0 is the instruction's exact 1.5 special case, not IOC.
        out
    }
    pub(super) fn max_number(&self, a: f32, b: f32) -> f32 {
        self.signaling(&[a, b]);
        // FPMaxNum suppresses a lone QNaN, NOT an SNaN. Signaling
        // priority and the first of two QNaNs must remain explicit.
        let signaling = |x: f32| x.is_nan() && x.to_bits() & 0x0040_0000 == 0;
        if signaling(a) || signaling(b) || (a.is_nan() && b.is_nan()) {
            super::operand_nan(a, b).expect("classified NaN operand")
        } else if a.is_nan() {
            b
        } else if b.is_nan() {
            a
        } else if a == 0.0 && b == 0.0 {
            // FPMax: most positive zero sign, independent of host max ties.
            f32::from_bits(a.to_bits() & b.to_bits() & 0x8000_0000)
        } else {
            a.max(b)
        }
    }
    pub(super) fn greater(&self, a: f32, b: f32) -> bool {
        // SIMD FCMGT signals both quiet and signaling NaNs.
        if a.is_nan() || b.is_nan() {
            self.raise(IOC);
            false
        } else {
            a > b
        }
    }
}

// Exact dyadic arithmetic for flag residuals, NOT a floating approximation.
// Every finite binary32 value is a <=24-bit integer * 2^e, -149<=e<=104.
// Products have <=48 bits, -298<=e<=208. Aligning a product and binary32
// addend needs at most 426 bits (product e=-298 versus addend high bit127);
// the opposite extreme, product e=208 versus addend e=-149, needs405 bits.
// A carry needs at most 427; halve only changes exponent. Division checks
// compare binary32 products, never perform wide integer division. 512 bits
// therefore suffice for ALL finite binary32 operands, not only fixtures.
// Every shifted-out bit/carry/borrow is checked; truncation is never silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Wide([u64; 8]);
impl Wide {
    fn small(v: u64) -> Self {
        let mut x = Self([0; 8]);
        x.0[0] = v;
        x
    }
    fn zero(self) -> bool {
        self.0 == [0; 8]
    }
    fn shift(self, n: usize) -> Self {
        let mut out = Self([0; 8]);
        for (i, word) in self.0.into_iter().enumerate() {
            if word == 0 {
                continue;
            }
            let j = i + n / 64;
            assert!(j < 8, "exact residual capacity");
            out.0[j] |= word << (n % 64);
            if !n.is_multiple_of(64) {
                let upper = word >> (64 - n % 64);
                if upper != 0 {
                    assert!(j + 1 < 8, "exact residual capacity");
                    out.0[j + 1] |= upper;
                }
            }
        }
        out
    }
    fn cmp(self, rhs: Self) -> std::cmp::Ordering {
        self.0.iter().rev().cmp(rhs.0.iter().rev())
    }
    fn add(self, rhs: Self) -> Self {
        let mut out = Self([0; 8]);
        let mut carry = 0_u128;
        for (i, word) in out.0.iter_mut().enumerate() {
            carry += u128::from(self.0[i]) + u128::from(rhs.0[i]);
            *word = carry as u64;
            carry >>= 64;
        }
        assert_eq!(carry, 0, "exact residual capacity");
        out
    }
    fn sub(self, rhs: Self) -> Self {
        let mut out = Self([0; 8]);
        let mut borrow = false;
        for (i, word) in out.0.iter_mut().enumerate() {
            let (v, b1) = self.0[i].overflowing_sub(rhs.0[i]);
            let (v, b2) = v.overflowing_sub(u64::from(borrow));
            *word = v;
            borrow = b1 || b2;
        }
        assert!(!borrow, "exact residual ordering");
        out
    }
}
#[derive(Clone, Copy)]
struct Exact {
    negative: bool,
    magnitude: Wide,
    exponent: i32,
}
impl Exact {
    fn float(value: f32) -> Self {
        let bits = value.to_bits();
        let e = ((bits >> 23) & 255) as i32;
        assert_ne!(e, 255, "nonfinite exact arithmetic");
        Self {
            negative: bits >> 31 != 0,
            magnitude: Wide::small(u64::from(
                (bits & 0x7f_ffff) | if e == 0 { 0 } else { 1 << 23 },
            )),
            exponent: if e == 0 { -149 } else { e - 150 },
        }
    }
    fn power(exponent: i32) -> Self {
        Self {
            negative: false,
            magnitude: Wide::small(1),
            exponent,
        }
    }
    fn neg(mut self) -> Self {
        self.negative = !self.negative;
        self
    }
    fn scale(mut self, n: i32) -> Self {
        self.exponent += n;
        self
    }
    fn product(a: f32, b: f32) -> Self {
        let a = Self::float(a);
        let b = Self::float(b);
        Self {
            negative: a.negative ^ b.negative,
            magnitude: Wide::small(a.magnitude.0[0] * b.magnitude.0[0]),
            exponent: a.exponent + b.exponent,
        }
    }
    fn aligned(self, b: Self) -> (Wide, Wide, i32) {
        // Canonicalizing zeros avoids unnecessary huge zero-only shifts.
        let e = if self.magnitude.zero() {
            b.exponent
        } else if b.magnitude.zero() {
            self.exponent
        } else {
            self.exponent.min(b.exponent)
        };
        let shift = |x: Self| {
            if x.magnitude.zero() {
                x.magnitude
            } else {
                x.magnitude.shift((x.exponent - e) as usize)
            }
        };
        (shift(self), shift(b), e)
    }
    fn add(self, b: Self) -> Self {
        let (a_mag, b_mag, e) = self.aligned(b);
        let (negative, magnitude) = if self.negative == b.negative {
            (self.negative, a_mag.add(b_mag))
        } else if a_mag.cmp(b_mag).is_lt() {
            (b.negative, b_mag.sub(a_mag))
        } else {
            (self.negative, a_mag.sub(b_mag))
        };
        Self {
            negative,
            magnitude,
            exponent: e,
        }
    }
    fn same_value(self, b: Self) -> bool {
        if self.magnitude.zero() && b.magnitude.zero() {
            return true;
        }
        let (a_mag, b_mag, _) = self.aligned(b);
        self.negative == b.negative && a_mag == b_mag
    }
    fn abs_less(self, b: Self) -> bool {
        let (a_mag, b_mag, _) = self.aligned(b);
        a_mag.cmp(b_mag).is_lt()
    }
}

/// Restores thread-local status on normal return and Rust unwind. Controls are
/// never altered. These registers are not the software native-flag oracle.
pub(super) struct HostStatusGuard {
    #[cfg(target_arch = "x86_64")]
    saved: u32,
    #[cfg(target_arch = "aarch64")]
    saved: u64,
}
impl HostStatusGuard {
    pub(super) fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let mut saved = 0;
            unsafe {
                std::arch::asm!("stmxcsr [{p}]",p=in(reg) &mut saved,options(nostack,preserves_flags));
            }
            Self { saved }
        }
        #[cfg(target_arch = "aarch64")]
        {
            let saved;
            unsafe {
                std::arch::asm!("mrs {v}, fpsr",v=out(reg) saved,options(nomem,nostack,preserves_flags));
            }
            Self { saved }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self {}
        }
    }
}
impl Drop for HostStatusGuard {
    fn drop(&mut self) {
        // SAFETY: restore this thread's previously read status; x86 control
        // bits are copied unchanged, AArch64 FPSR contains no FPCR controls.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            std::arch::asm!("ldmxcsr [{p}]",p=in(reg) &self.saved,options(nostack,preserves_flags));
        }
        #[cfg(target_arch = "aarch64")]
        unsafe {
            std::arch::asm!("msr fpsr, {v}",v=in(reg) self.saved,options(nomem,nostack,preserves_flags));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_capacity_tininess_and_fused_residuals() {
        let status = FloatStatus::new(0);
        let max = f32::MAX;
        let tiny = f32::from_bits(1);
        // Opposite ends of the 426-bit alignment bound, plus carry/sign.
        status.fma(max, max, tiny);
        assert_eq!(status.bits(), OFC | IXC);
        let status = FloatStatus::new(0);
        status.fma(tiny, tiny, max);
        assert_eq!(status.bits(), IXC);
        let status = FloatStatus::new(0);
        let min_normal = f32::MIN_POSITIVE;
        assert_eq!(
            status.mul(min_normal, f32::from_bits(0x3f7f_ffff)),
            min_normal
        );
        assert_eq!(status.bits(), UFC | IXC); // rounds to normal but tiny BEFORE
        let status = FloatStatus::new(0);
        assert_eq!(status.mul(min_normal, 0.5).to_bits(), 0x0040_0000);
        assert_eq!(status.bits(), 0); // exact subnormal: no UFC or IDC at FPCR0
        let status = FloatStatus::new(0);
        assert_eq!(status.div(tiny, 2.0).to_bits(), 0);
        assert_eq!(status.bits(), UFC | IXC);
    }

    #[test]
    fn max_number_zero_sign_and_nan_priority_are_explicit() {
        let status = FloatStatus::new(0);
        assert_eq!(status.max_number(0.0, -0.0).to_bits(), 0);
        assert_eq!(status.max_number(-0.0, 0.0).to_bits(), 0);
        assert_eq!(status.max_number(-0.0, -0.0).to_bits(), 0x8000_0000);
        let first = f32::from_bits(0xffc0_0123);
        let second = f32::from_bits(0x7fc0_0456);
        let signaling = f32::from_bits(0x7f80_0789);
        assert_eq!(status.max_number(first, second).to_bits(), first.to_bits());
        assert_eq!(status.max_number(first, 1.0), 1.0);
        assert_eq!(status.max_number(first, signaling).to_bits(), 0x7fc0_0789);
        assert_eq!(status.bits(), IOC);
    }

    #[test]
    fn native_special_flags_not_nan_inference() {
        let status = FloatStatus::new(0x0800_0084);
        status.estimate(0.0);
        status.mul(f32::INFINITY, 0.0);
        assert_eq!(status.bits(), 0x0800_0087);
        let status = FloatStatus::new(0);
        status.step(f32::INFINITY, 0.0);
        assert_eq!(status.bits(), 0);
        assert!(!status.greater(1.0, f32::NAN));
        assert_eq!(status.bits(), IOC); // quiet NaN FCMGT is signaling
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn incoming_thread_status_restored_on_return_and_unwind() {
        fn read() -> (u32, u16) {
            let mut mxcsr = 0;
            let x87: u16;
            // SAFETY: read thread-local status registers into valid locals.
            unsafe {
                std::arch::asm!("stmxcsr [{p}]", p=in(reg) &mut mxcsr, options(nostack,preserves_flags));
                std::arch::asm!("fnstsw ax",out("ax") x87,options(nomem,nostack,preserves_flags));
            }
            (mxcsr, x87)
        }
        let _original = HostStatusGuard::new();
        let controls = read().0 & !0x3f;
        for bits in [0, 5, 0x20, 0x3f] {
            let incoming = controls | bits;
            // SAFETY: only status bits change in this test; controls stay intact.
            unsafe {
                std::arch::asm!("ldmxcsr [{p}]",p=in(reg) &incoming,options(nostack,preserves_flags));
            }
            let before = read();
            let context = super::super::TrAnmNativeContext::new(0, 1e-12).unwrap();
            context.unpack_rotation([0xffff; 3]).unwrap();
            assert_eq!(read(), before);
            let data = super::super::TrAnmChannel {
                encoding: super::super::TrAnmEncoding::Framed16,
                keys: [0, 0, 65535, 65535]
                    .map(|frame| super::super::TrAnmKey {
                        frame,
                        value: [0xffff; 3],
                    })
                    .to_vec(),
            };
            let view = super::super::TrAnmNativeFramedRotation::new(&data).unwrap();
            std::hint::black_box(view.evaluate(f32::from_bits(1), context).unwrap());
            assert_eq!(read(), before);
            assert!(view.evaluate(f32::from_bits(0x7f80_0001), context).is_err());
            assert_eq!(read(), before);
            let mut cache = super::super::TrAnmNativeFramedRotationCache::new(&data).unwrap();
            std::hint::black_box(cache.evaluate(f32::from_bits(1), context).unwrap());
            assert_eq!(read(), before);
            assert!(cache
                .evaluate(f32::from_bits(0x7f80_0001), context)
                .is_err());
            assert_eq!(read(), before);
            let unwind = std::panic::catch_unwind(|| {
                let _restore = HostStatusGuard::new();
                std::hint::black_box(FloatStatus::new(0).div(f32::from_bits(1), 2.0));
                panic!("authored unwind");
            });
            assert!(unwind.is_err());
            assert_eq!(read(), before);
        }
    }

    /// Independent authored ISA-only instruction oracle, not game execution.
    #[test]
    fn optional_instruction_flags_oracle() {
        let Some(path) = std::env::var_os("PLA_NATIVE_FLOAT_FLAGS_ORACLE") else {
            eprintln!("[skip] PLA_NATIVE_FLOAT_FLAGS_ORACLE absent; no ISA execution proof");
            return;
        };
        super::super::check_host_environment().expect("controlled host RNE/no-flush profile");
        let _restore_host_status = HostStatusGuard::new();
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 64 * 1024 && bytes.len() >= 12);
        assert_eq!(&bytes[..8], b"DEC104IF");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert!(count <= 1024 && bytes.len() == 12 + count * 28);
        for (i, record) in bytes[12..].as_chunks::<28>().0.iter().enumerate() {
            let words: Vec<u32> = record
                .as_chunks::<4>()
                .0
                .iter()
                .map(|x| u32::from_le_bytes(*x))
                .collect();
            let [a, b, c] = [words[1], words[2], words[3]].map(f32::from_bits);
            let status = FloatStatus::new(words[4]);
            let result = match words[0] {
                0 => status.add(a, b).to_bits(),
                1 => status.sub(a, b).to_bits(),
                2 => status.mul(a, b).to_bits(),
                3 => status.div(a, b).to_bits(),
                4 => status.fma(a, b, c).to_bits(),
                5 => status.estimate(a).to_bits(),
                6 => status.step(a, b).to_bits(),
                7 => {
                    if status.greater(a, b) {
                        u32::MAX
                    } else {
                        0
                    }
                }
                8 => status.max_number(a, b).to_bits(),
                _ => panic!("unknown author opcode"),
            };
            assert_eq!(result, words[5], "ISA result case {i}");
            assert_eq!(status.bits(), words[6], "ISA FPSR case {i}");
        }
        eprintln!("independent ISA output/flags comparisons={count}");
    }
}
