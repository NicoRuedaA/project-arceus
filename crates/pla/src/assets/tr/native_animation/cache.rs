//! Channel-bound numerical ring cache; not a native ABI or a bone/SRT player.
use super::{
    check_host_environment, unpack_rotation, FloatStatus, HostStatusGuard, TrAnmChannel, TrAnmKey,
    TrAnmNativeContext, TrAnmNativeError, TrAnmNativeEvaluation, TrAnmNativeFramedRotation,
};

/// Checked stateful counterpart of cached entries 027b69e8/027b7704 and
/// refill entries 027b7168/027b7e7c. The non-null dispatch branch uses these.
///
/// State belongs to one immutable validated channel. Rebinding reconstructs
/// it explicitly; native raw state has no channel identity/generation check.
/// Only finite guarded positions and the supplied FPCR0 context are supported.
/// This does not implement the enclosing bone cache's fixed/S/T flags.
#[derive(Debug)]
pub struct TrAnmNativeFramedRotationCache<'a> {
    keys: &'a [TrAnmKey<[u16; 3]>],
    reserved: [u8; 4],
    cursor: u32,
    last_position: f32,
    index: u32,
    times: [f32; 8],
    rotations: [[f32; 4]; 8],
}

impl<'a> TrAnmNativeFramedRotationCache<'a> {
    pub fn new(channel: &'a TrAnmChannel<[u16; 3]>) -> Result<Self, TrAnmNativeError> {
        let view = TrAnmNativeFramedRotation::new(channel)?;
        Ok(Self {
            keys: view.keys,
            reserved: [0; 4],
            cursor: 0,
            last_position: f32::MAX,
            index: 1,
            times: [0.0; 8],
            rotations: [[0.0; 4]; 8],
        })
    }

    /// Reconstruct the rotation substate proved by initializer 027cf074.
    pub fn reset(&mut self) {
        self.reserved = [0; 4];
        self.cursor = 0;
        self.last_position = f32::MAX;
        self.index = 1;
        self.times = [0.0; 8];
        self.rotations = [[0.0; 4]; 8];
    }

    /// Validate before changing state; changing a channel is never implicit.
    pub fn rebind(&mut self, channel: &'a TrAnmChannel<[u16; 3]>) -> Result<(), TrAnmNativeError> {
        let view = TrAnmNativeFramedRotation::new(channel)?;
        self.keys = view.keys;
        self.reset();
        Ok(())
    }

    /// Read-only little-endian evidence snapshot of the 176-byte rotation
    /// substate. It is not an FFI representation, and excludes the enclosing
    /// 16 bytes of S/T state. Reserved/tag bytes are untouched by this path.
    pub fn snapshot(&self) -> [u8; 176] {
        let mut bytes = [0; 176];
        bytes[..4].copy_from_slice(&self.reserved);
        bytes[4..8].copy_from_slice(&self.cursor.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.last_position.to_bits().to_le_bytes());
        bytes[12..16].copy_from_slice(&self.index.to_le_bytes());
        for (i, value) in self.times.iter().enumerate() {
            bytes[16 + i * 4..20 + i * 4].copy_from_slice(&value.to_bits().to_le_bytes());
        }
        for (i, value) in self.rotations.iter().flatten().enumerate() {
            bytes[48 + i * 4..52 + i * 4].copy_from_slice(&value.to_bits().to_le_bytes());
        }
        bytes
    }

    pub fn evaluate(
        &mut self,
        position: f32,
        context: TrAnmNativeContext,
    ) -> Result<[f32; 4], TrAnmNativeError> {
        Ok(self.evaluate_with_fpsr(position, context, 0)?.rotation)
    }

    /// Return per-call cumulative native FPSR; cached decodes do not retain
    /// prior flags. Incoming host status is restored on return or unwind.
    /// Invalid input leaves all cache state untouched.
    pub fn evaluate_with_fpsr(
        &mut self,
        position: f32,
        context: TrAnmNativeContext,
        initial_fpsr: u32,
    ) -> Result<TrAnmNativeEvaluation, TrAnmNativeError> {
        if initial_fpsr & !0x0800_009f != 0 {
            return Err(TrAnmNativeError::UnsupportedFpsrState);
        }
        check_host_environment()?;
        let _host_status = HostStatusGuard::new();
        if !position.is_finite() {
            return Err(TrAnmNativeError::NonfinitePosition);
        }
        if position < self.keys[1].frame as f32
            || position > self.keys[self.keys.len() - 2].frame as f32
        {
            return Err(TrAnmNativeError::PositionOutsideGuards);
        }
        let status = FloatStatus::new(initial_fpsr);
        let old_position = self.last_position;
        self.last_position = position;
        self.reserved[2] = 0;
        if old_position > position {
            self.cursor = if position > 0.0 { u32::MAX } else { 0 };
            self.index = 1;
            self.refill(position, &status);
        } else {
            let next = (self.index + 1) & 7;
            let second = (self.index + 2) & 7;
            if self.times[next as usize] < position {
                if self.times[second as usize] >= position {
                    self.index = next;
                    if next & 3 == 2 {
                        self.refill(position, &status);
                    }
                } else if second & 3 == 3 {
                    self.index = second;
                    self.reserved[2] = 1;
                    self.refill(position, &status);
                } else if self.times[((self.index + 3) & 7) as usize] >= position {
                    self.index = second;
                    if second & 3 == 2 {
                        self.refill(position, &status);
                    }
                } else {
                    self.cursor = u32::MAX;
                    self.index = 1;
                    self.refill(position, &status);
                }
            }
        }
        let lower = (self.index & 7) as usize;
        let upper = ((self.index + 1) & 7) as usize;
        let alpha = status.div(
            status.sub(position, self.times[lower]),
            status.sub(self.times[upper], self.times[lower]),
        );
        let a = self.rotations[lower];
        let b = self.rotations[upper];
        let rotation = if alpha <= 0.0 {
            a
        } else if alpha >= 1.0 {
            b
        } else {
            let previous = self.rotations[((self.index.wrapping_sub(1)) & 7) as usize];
            let next = self.rotations[((self.index + 2) & 7) as usize];
            cached_cubic(previous, a, b, next, alpha, context, &status)
        };
        Ok(TrAnmNativeEvaluation {
            rotation,
            fpsr: status.bits(),
        })
    }

    fn refill(&mut self, position: f32, status: &FloatStatus) {
        if self.cursor & 0x8000_0000 == 0 {
            if self.cursor as usize >= self.keys.len() {
                return;
            }
            if self.reserved[2] != 0 {
                self.reserved[2] = 0;
                if (self.keys[self.cursor as usize].frame as f32) < position {
                    self.cursor = u32::MAX;
                    self.index = 1;
                }
            }
        }
        let start = if self.cursor & 0x8000_0000 != 0 {
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
            start - 2
        } else {
            self.cursor as usize
        };
        let destination = ((self.index + 2) & 4) as usize;
        for (offset, key) in self.keys.iter().skip(start).take(4).enumerate() {
            self.times[destination + offset] = key.frame as f32;
            self.rotations[destination + offset] = unpack_rotation(key.value, status);
        }
        self.cursor = (start + 4) as u32;
    }
}

fn cached_cubic(
    previous: [f32; 4],
    a: [f32; 4],
    b: [f32; 4],
    next: [f32; 4],
    alpha: f32,
    context: TrAnmNativeContext,
    status: &FloatStatus,
) -> [f32; 4] {
    let mut result = [0.0; 4];
    for lane in 0..4 {
        let t0 = status.mul(status.sub(b[lane], previous[lane]), 0.5);
        let t1 = status.mul(status.sub(next[lane], a[lane]), 0.5);
        // Operand priority is different from the stateless entry.
        let cubic = status.sub(status.add(a[lane], a[lane]), status.add(b[lane], b[lane]));
        let cubic = status.add(t0, cubic);
        let cubic = status.add(t1, cubic);
        let quadratic = status.add(status.mul(b[lane], 3.0), status.mul(a[lane], -3.0));
        let quadratic = status.sub(quadratic, status.add(t0, t0));
        let quadratic = status.sub(quadratic, t1);
        let v = status.add(quadratic, status.mul(cubic, alpha));
        let v = status.add(t0, status.mul(v, alpha));
        result[lane] = status.add(a[lane], status.mul(v, alpha));
    }
    let square = result.map(|x| status.mul(x, x));
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
    let normalized = result.map(|x| status.mul(reciprocal, x));
    if status.greater(context.quaternion_epsilon, norm) {
        [0.0; 4]
    } else {
        normalized
    }
}
