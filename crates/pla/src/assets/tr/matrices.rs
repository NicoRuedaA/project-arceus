//! Reference pose math for the demonstrated zero-pivot lane (dec097).
//! Blender mathutils agreement is not proof of native runtime evaluation.
use super::{SklBuffer, TrSkl, TrSklTransform};

/// Affine matrix in explicit row-major notation acting on column vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct TrSklMatrix {
    pub rows: [[f64; 4]; 4],
}

impl TrSklMatrix {
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    fn finite(&self) -> Result<(), String> {
        if self.rows.iter().flatten().any(|v| !v.is_finite()) {
            return Err("non-finite skeleton matrix".into());
        }
        Ok(())
    }

    pub fn multiply(&self, right: &Self) -> Result<Self, String> {
        self.finite()?;
        right.finite()?;
        let rows = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                (0..4)
                    .map(|i| self.rows[row][i] * right.rows[i][column])
                    .sum()
            })
        });
        let result = Self { rows };
        result.finite()?;
        Ok(result)
    }

    pub fn max_abs_difference(&self, other: &Self) -> Result<f64, String> {
        self.finite()?;
        other.finite()?;
        let difference = self
            .rows
            .iter()
            .flatten()
            .zip(other.rows.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        if !difference.is_finite() {
            return Err("non-finite skeleton matrix difference".into());
        }
        Ok(difference)
    }

    /// Inverse of an affine matrix (linear block plus translation). Used only by
    /// export-time bind reconstruction; a singular linear block fails loudly.
    pub fn invert(&self) -> Result<Self, String> {
        self.finite()?;
        let m = &self.rows;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        if !det.is_finite() || det.abs() < 1.0e-12 {
            return Err("singular skeleton matrix".into());
        }
        let inv = 1.0 / det;
        let adjugate = [
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
            ],
        ];
        let translation = [m[0][3], m[1][3], m[2][3]];
        let mut rows = TrSklMatrix::IDENTITY.rows;
        for row in 0..3 {
            rows[row][..3].copy_from_slice(&adjugate[row]);
            rows[row][3] = -(adjugate[row][0] * translation[0]
                + adjugate[row][1] * translation[1]
                + adjugate[row][2] * translation[2]);
        }
        let result = Self { rows };
        result.finite()?;
        Ok(result)
    }
}

/// Serialized bind record. Flags are retained without invented runtime meaning.
#[derive(Debug, Clone, PartialEq)]
pub struct TrSklBindRecord {
    pub flags: [u8; 2],
    /// Serialized XYZW vectors become affine columns. Not assumed to equal
    /// inverse(reference global) for every asset; item_230 is a counterexample.
    pub matrix: TrSklMatrix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrSklReferencePose {
    pub local: Vec<TrSklMatrix>,
    pub global: Vec<TrSklMatrix>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrSklBindResidual {
    pub node_index: usize,
    pub rig_index: usize,
    pub global_times_bind_error: f64,
    pub bind_times_global_error: f64,
}

impl TrSklTransform {
    /// Blender XYZ Euler reference convention: T * Rz * Ry * Rx * S.
    /// Angles are radians; no coordinate-system conversion is performed.
    pub fn reference_matrix(&self) -> Result<TrSklMatrix, String> {
        if self
            .scale
            .iter()
            .chain(self.rotation.iter())
            .chain(self.translation.iter())
            .any(|v| !v.is_finite())
        {
            return Err("non-finite skeleton SRT".into());
        }
        let [x, y, z] = self.rotation.map(f64::from);
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        let mut result = TrSklMatrix::IDENTITY;
        let rotation = [
            [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
            [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
            [-sy, cy * sx, cy * cx],
        ];
        for (row, components) in rotation.iter().enumerate() {
            for (column, component) in components.iter().enumerate() {
                result.rows[row][column] = component * f64::from(self.scale[column]);
            }
            result.rows[row][3] = f64::from(self.translation[row]);
        }
        result.finite()?;
        Ok(result)
    }
}

impl TrSkl {
    /// Checked bind-matrix read, independent of the reference-pose hypothesis.
    pub fn read_bind_records(bytes: &[u8]) -> Result<Vec<TrSklBindRecord>, String> {
        Self::read_bind_records_with_ik(bytes, false)
    }

    /// [`TrSkl::read_bind_records`] for skeletons that carry IK records (see
    /// [`TrSkl::parse_ignoring_ik`]).
    pub fn read_bind_records_ignoring_ik(bytes: &[u8]) -> Result<Vec<TrSklBindRecord>, String> {
        Self::read_bind_records_with_ik(bytes, true)
    }

    pub(crate) fn read_bind_records_with_ik(bytes: &[u8], allow_ik: bool) -> Result<Vec<TrSklBindRecord>, String> {
        // Reuse the demonstrated root/hierarchy gate, not a blind vector scan.
        let skeleton = Self::parse_with(bytes, allow_ik)?;
        let buffer = SklBuffer(bytes);
        let root = buffer.table(buffer.target(0)?, 5)?;
        let mut records = Vec::with_capacity(skeleton.bind_count);
        for at in buffer.vector(&root, 2)? {
            let table = buffer.table(at, 3)?;
            // An omitted flag byte reads as the demonstrated value 1 (SUPPOSED: trainer skeletons such as tr0015
            // captain4 / tr0007 si1 omit fields that equal the common value).
            let flag = |id: usize| -> Result<u8, String> {
                match buffer.field(&table, id, 1)? {
                    Some(at) => Ok(buffer.range(at, 1)?[0]),
                    None => Ok(1),
                }
            };
            let flags = [flag(0)?, flag(1)?];
            if flags != [1, 1] {
                return Err("unsupported skeleton bind flags".into());
            }
            let matrix = buffer.table(buffer.target(buffer.required(&table, 2, 4)?)?, 4)?;
            let mut rows = TrSklMatrix::IDENTITY.rows;
            for column in 0..4 {
                // An omitted column keeps the identity value already in `rows`.
                if buffer.field(&matrix, column, 12)?.is_none() {
                    continue;
                }
                for (row, value) in buffer.vec3(&matrix, column)?.iter().enumerate() {
                    rows[row][column] = f64::from(*value);
                }
            }
            records.push(TrSklBindRecord {
                flags,
                matrix: TrSklMatrix { rows },
            });
        }
        Ok(records)
    }

    /// Evaluate the independently corroborated zero-pivot reference convention.
    /// This API is intentionally named reference: native parity is unverified.
    pub fn reference_pose(&self) -> Result<TrSklReferencePose, String> {
        if self.nodes.is_empty() || self.nodes.len() > 8192 {
            return Err("unsupported reference skeleton node count".into());
        }
        let mut local = Vec::with_capacity(self.nodes.len());
        let mut global: Vec<TrSklMatrix> = Vec::with_capacity(self.nodes.len());
        let mut roots = 0;
        for (i, node) in self.nodes.iter().enumerate() {
            if node
                .scale_pivot
                .iter()
                .chain(node.rotate_pivot.iter())
                .any(|&v| v != 0.0)
            {
                return Err("nonzero/non-finite skeleton pivots unsupported".into());
            }
            let matrix = node.local.reference_matrix()?;
            let world = match node.parent {
                None => {
                    roots += 1;
                    matrix.clone()
                }
                Some(parent) if parent < i => global[parent].multiply(&matrix)?,
                _ => return Err("unsupported reference parent order or cycle".into()),
            };
            local.push(matrix);
            global.push(world);
        }
        if roots != 1 {
            return Err("reference skeleton requires one root".into());
        }
        Ok(TrSklReferencePose { local, global })
    }

    /// Report both product residuals; never silently modify an authored bind.
    pub fn bind_rest_residuals(
        &self,
        records: &[TrSklBindRecord],
    ) -> Result<Vec<TrSklBindResidual>, String> {
        if records.len() != self.bind_count {
            return Err("bind record count mismatch".into());
        }
        let pose = self.reference_pose()?;
        let mut seen = vec![false; records.len()];
        let mut residuals = Vec::with_capacity(records.len());
        for (node_index, node) in self.nodes.iter().enumerate() {
            let Some(rig_index) = node.rig_index else {
                continue;
            };
            let record = records
                .get(rig_index)
                .ok_or("rig index outside bind records")?;
            if record.flags != [1, 1] {
                return Err("unsupported skeleton bind flags".into());
            }
            if seen[rig_index] {
                return Err("duplicate rig-to-node mapping".into());
            }
            seen[rig_index] = true;
            residuals.push(TrSklBindResidual {
                node_index,
                rig_index,
                global_times_bind_error: pose.global[node_index]
                    .multiply(&record.matrix)?
                    .max_abs_difference(&TrSklMatrix::IDENTITY)?,
                bind_times_global_error: record
                    .matrix
                    .multiply(&pose.global[node_index])?
                    .max_abs_difference(&TrSklMatrix::IDENTITY)?,
            });
        }
        if seen.iter().any(|present| !present) {
            return Err("unresolved bind rig".into());
        }
        Ok(residuals)
    }

    pub fn validate_bind_rest_pose(
        &self,
        records: &[TrSklBindRecord],
        epsilon: f64,
    ) -> Result<(), String> {
        if !epsilon.is_finite() || epsilon <= 0.0 {
            return Err("invalid bind residual epsilon".into());
        }
        for residual in self.bind_rest_residuals(records)? {
            let error = residual
                .global_times_bind_error
                .max(residual.bind_times_global_error);
            if error > epsilon {
                return Err(format!("bind rig {} node {} disagrees with reference rest pose: residual {error} > {epsilon}", residual.rig_index, residual.node_index));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_is_the_two_sided_inverse() {
        let matrix = TrSklMatrix {
            rows: [
                [0.0, 1.0, 0.0, 0.5],
                [1.0, 0.0, 0.0, -0.25],
                [0.0, 0.0, 1.0, 2.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let inverse = matrix.invert().expect("invertible");
        let product = matrix.multiply(&inverse).expect("multiply");
        assert!(
            product
                .max_abs_difference(&TrSklMatrix::IDENTITY)
                .expect("difference")
                < 1.0e-12
        );
        let other = inverse.multiply(&matrix).expect("multiply");
        assert!(
            other
                .max_abs_difference(&TrSklMatrix::IDENTITY)
                .expect("difference")
                < 1.0e-12
        );
    }

    #[test]
    fn invert_rejects_singular_and_non_finite() {
        let singular = TrSklMatrix {
            rows: [
                [1.0, 2.0, 3.0, 0.0],
                [2.0, 4.0, 6.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        assert!(singular.invert().is_err());
        let mut non_finite = TrSklMatrix::IDENTITY.clone();
        non_finite.rows[0][3] = f64::NAN;
        assert!(non_finite.invert().is_err());
    }
}
