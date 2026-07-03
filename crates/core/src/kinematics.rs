use orskit_frames::ReferenceFrame;
use orskit_units::{Acceleration, AccelerationVector, Length, Position, Velocity, VelocityVector};
use thiserror::Error;

/// A position vector expressed in its attached reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedPosition {
    value: Position,
    frame: ReferenceFrame,
}

impl FramedPosition {
    /// Attaches a frame to a finite position vector.
    pub fn new(value: Position, frame: ReferenceFrame) -> Result<Self, KinematicError> {
        if !value.is_finite() {
            return Err(KinematicError::NonFinitePosition);
        }
        Ok(Self { value, frame })
    }

    /// Returns the typed position components.
    #[must_use]
    pub const fn value(self) -> Position {
        self.value
    }

    /// Returns the frame in which the position is expressed.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the Euclidean distance from this frame's origin.
    #[must_use]
    pub fn norm(self) -> Length {
        self.value.norm()
    }
}

/// A velocity vector expressed in its attached reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedVelocity {
    value: VelocityVector,
    frame: ReferenceFrame,
}

impl FramedVelocity {
    /// Attaches a frame to a finite velocity vector.
    pub fn new(value: VelocityVector, frame: ReferenceFrame) -> Result<Self, KinematicError> {
        if !value.is_finite() {
            return Err(KinematicError::NonFiniteVelocity);
        }
        Ok(Self { value, frame })
    }

    /// Returns the typed velocity components.
    #[must_use]
    pub const fn value(self) -> VelocityVector {
        self.value
    }

    /// Returns the frame in which the velocity is expressed.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the velocity magnitude.
    #[must_use]
    pub fn speed(self) -> Velocity {
        self.value.speed()
    }
}

/// An acceleration vector expressed in its attached reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramedAcceleration {
    value: AccelerationVector,
    frame: ReferenceFrame,
}

impl FramedAcceleration {
    /// Attaches a frame to a finite acceleration vector.
    pub fn new(value: AccelerationVector, frame: ReferenceFrame) -> Result<Self, KinematicError> {
        if !value.is_finite() {
            return Err(KinematicError::NonFiniteAcceleration);
        }
        Ok(Self { value, frame })
    }

    /// Returns the typed acceleration components.
    #[must_use]
    pub const fn value(self) -> AccelerationVector {
        self.value
    }

    /// Returns the frame in which the acceleration is expressed.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the acceleration magnitude.
    #[must_use]
    pub fn magnitude(self) -> Acceleration {
        self.value.magnitude()
    }
}

/// Invalid kinematic vector input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KinematicError {
    /// At least one position component is NaN or infinite.
    #[error("position components must be finite")]
    NonFinitePosition,
    /// At least one velocity component is NaN or infinite.
    #[error("velocity components must be finite")]
    NonFiniteVelocity,
    /// At least one acceleration component is NaN or infinite.
    #[error("acceleration components must be finite")]
    NonFiniteAcceleration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_part_of_kinematic_value_identity() {
        let vector = Position::from_metres(1.0, 2.0, 3.0);
        let gcrf = FramedPosition::new(vector, ReferenceFrame::GCRF).expect("finite position");
        let eme2000 =
            FramedPosition::new(vector, ReferenceFrame::EME2000).expect("finite position");

        assert_ne!(gcrf, eme2000);
        assert_eq!(gcrf.frame(), ReferenceFrame::GCRF);
    }
}
