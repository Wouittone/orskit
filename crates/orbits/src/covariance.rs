use frames::ReferenceFrame;
use nalgebra::{Cholesky, SMatrix};
use thiserror::Error;
use units::uom::si::area::square_meter;
use units::{Area, Position, PositionVelocityCovariance, VelocityVariance, VelocityVector};

/// Full unit-qualified covariance of Cartesian position and velocity.
///
/// Rows and columns use `[x, y, z]` order within the position and velocity
/// blocks. The position/velocity block stores `cov(position_row,
/// velocity_column)`; its transpose supplies the mirrored velocity/position
/// block.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianCovariance {
    frame: ReferenceFrame,
    position_position: [[Area; 3]; 3],
    position_velocity: [[PositionVelocityCovariance; 3]; 3],
    velocity_velocity: [[VelocityVariance; 3]; 3],
}

impl CartesianCovariance {
    /// Creates a diagonal covariance from positive typed standard deviations.
    pub fn from_standard_deviations(
        frame: ReferenceFrame,
        position: Position,
        velocity: VelocityVector,
    ) -> Result<Self, CartesianCovarianceError> {
        let position = position.to_metres();
        let velocity = velocity.to_metres_per_second();
        if position
            .into_iter()
            .chain(velocity)
            .any(|value| !value.is_finite())
        {
            return Err(CartesianCovarianceError::NonFiniteStandardDeviation);
        }
        if position
            .into_iter()
            .chain(velocity)
            .any(|value| value <= 0.0)
        {
            return Err(CartesianCovarianceError::NonPositiveStandardDeviation);
        }
        let position_position = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                Area::new::<square_meter>(if row == column {
                    position[row] * position[row]
                } else {
                    0.0
                })
            })
        });
        let position_velocity =
            [[PositionVelocityCovariance::from_square_metres_per_second(0.0); 3]; 3];
        let velocity_velocity = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                VelocityVariance::from_square_metres_per_square_second(if row == column {
                    velocity[row] * velocity[row]
                } else {
                    0.0
                })
            })
        });
        Self::from_blocks(
            frame,
            position_position,
            position_velocity,
            velocity_velocity,
        )
    }

    /// Creates a correlated covariance from its three unit-qualified blocks.
    pub fn from_blocks(
        frame: ReferenceFrame,
        position_position: [[Area; 3]; 3],
        position_velocity: [[PositionVelocityCovariance; 3]; 3],
        velocity_velocity: [[VelocityVariance; 3]; 3],
    ) -> Result<Self, CartesianCovarianceError> {
        let raw = raw_matrix(&position_position, &position_velocity, &velocity_velocity);
        for row in 0..6 {
            for column in 0..6 {
                if !raw[(row, column)].is_finite() {
                    return Err(CartesianCovarianceError::NonFiniteEntry { row, column });
                }
            }
        }
        validate_symmetric_block(&raw, 0)?;
        validate_symmetric_block(&raw, 3)?;
        if Cholesky::new(raw).is_none() {
            return Err(CartesianCovarianceError::NotPositiveDefinite);
        }
        Ok(Self {
            frame,
            position_position,
            position_velocity,
            velocity_velocity,
        })
    }

    /// Returns the covariance expression frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Returns `cov(position, position)` in square metres.
    #[must_use]
    pub const fn position_position(&self) -> &[[Area; 3]; 3] {
        &self.position_position
    }

    /// Returns `cov(position, velocity)` in square metres per second.
    #[must_use]
    pub const fn position_velocity(&self) -> &[[PositionVelocityCovariance; 3]; 3] {
        &self.position_velocity
    }

    /// Returns `cov(velocity, velocity)` in square metres per square second.
    #[must_use]
    pub const fn velocity_velocity(&self) -> &[[VelocityVariance; 3]; 3] {
        &self.velocity_velocity
    }
}

fn raw_matrix(
    position_position: &[[Area; 3]; 3],
    position_velocity: &[[PositionVelocityCovariance; 3]; 3],
    velocity_velocity: &[[VelocityVariance; 3]; 3],
) -> SMatrix<f64, 6, 6> {
    SMatrix::from_fn(|row, column| match (row < 3, column < 3) {
        (true, true) => position_position[row][column].get::<square_meter>(),
        (true, false) => position_velocity[row][column - 3].as_square_metres_per_second(),
        (false, true) => position_velocity[column][row - 3].as_square_metres_per_second(),
        (false, false) => {
            velocity_velocity[row - 3][column - 3].as_square_metres_per_square_second()
        }
    })
}

fn validate_symmetric_block(
    raw: &SMatrix<f64, 6, 6>,
    offset: usize,
) -> Result<(), CartesianCovarianceError> {
    for row in 0..3 {
        for column in 0..row {
            let left = raw[(offset + row, offset + column)];
            let right = raw[(offset + column, offset + row)];
            let scale = left.abs().max(right.abs()).max(1.0);
            if (left - right).abs() > 32.0 * f64::EPSILON * scale {
                return Err(CartesianCovarianceError::NotSymmetric {
                    row: offset + row,
                    column: offset + column,
                });
            }
        }
    }
    Ok(())
}

/// Invalid Cartesian covariance input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CartesianCovarianceError {
    /// A standard-deviation component is NaN or infinite.
    #[error("Cartesian standard deviations must be finite")]
    NonFiniteStandardDeviation,
    /// A standard-deviation component is zero or negative.
    #[error("Cartesian standard deviations must be strictly positive")]
    NonPositiveStandardDeviation,
    /// One covariance entry is NaN or infinite.
    #[error("Cartesian covariance entry ({row}, {column}) must be finite")]
    NonFiniteEntry {
        /// Zero-based row.
        row: usize,
        /// Zero-based column.
        column: usize,
    },
    /// A diagonal covariance block is not symmetric.
    #[error("Cartesian covariance entries ({row}, {column}) and ({column}, {row}) differ")]
    NotSymmetric {
        /// Zero-based row.
        row: usize,
        /// Zero-based column.
        column: usize,
    },
    /// The full covariance cannot be factored.
    #[error("Cartesian covariance must be strictly positive definite")]
    NotPositiveDefinite,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_covariance_retains_typed_blocks() {
        let covariance = CartesianCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(10.0, 20.0, 30.0),
            VelocityVector::from_metres_per_second(1.0, 2.0, 3.0),
        )
        .expect("positive covariance");
        assert_eq!(covariance.frame(), ReferenceFrame::GCRF);
        assert_eq!(
            covariance.position_position()[1][1],
            Area::new::<square_meter>(400.0)
        );
        assert_eq!(
            covariance.velocity_velocity()[2][2],
            VelocityVariance::from_square_metres_per_square_second(9.0)
        );
    }

    #[test]
    fn correlated_covariance_rejects_asymmetry_and_indefiniteness() {
        let mut position = [[Area::new::<square_meter>(0.0); 3]; 3];
        let cross = [[PositionVelocityCovariance::from_square_metres_per_second(0.0); 3]; 3];
        let mut velocity = [[VelocityVariance::from_square_metres_per_square_second(0.0); 3]; 3];
        for index in 0..3 {
            position[index][index] = Area::new::<square_meter>(1.0);
            velocity[index][index] = VelocityVariance::from_square_metres_per_square_second(1.0);
        }
        position[0][1] = Area::new::<square_meter>(0.5);
        assert!(matches!(
            CartesianCovariance::from_blocks(ReferenceFrame::GCRF, position, cross, velocity,),
            Err(CartesianCovarianceError::NotSymmetric { .. })
        ));

        position[1][0] = Area::new::<square_meter>(0.5);
        position[0][0] = Area::new::<square_meter>(0.1);
        assert_eq!(
            CartesianCovariance::from_blocks(ReferenceFrame::GCRF, position, cross, velocity,),
            Err(CartesianCovarianceError::NotPositiveDefinite)
        );
    }
}
