use frames::ReferenceFrame;
use thiserror::Error;
use units::{Acceleration, AccelerationVector, Length, Position, Velocity, VelocityVector};

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

/// Cartesian translational coordinates.
///
/// Each component retains its own frame. The coordinate fields of formats such
/// as CCSDS OEM map to this type; their epoch is attached separately with
/// [`crate::CoordinateSample`] because those formats do not provide all
/// physical properties required by a complete [`crate::SpacecraftView`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianCoordinates {
    position: FramedPosition,
    velocity: FramedVelocity,
    acceleration: Option<FramedAcceleration>,
}

impl CartesianCoordinates {
    /// Constructs Cartesian position/velocity coordinates.
    #[must_use]
    pub const fn new(position: FramedPosition, velocity: FramedVelocity) -> Self {
        Self {
            position,
            velocity,
            acceleration: None,
        }
    }

    /// Adds or replaces the optional acceleration.
    #[must_use]
    pub const fn with_acceleration(mut self, acceleration: FramedAcceleration) -> Self {
        self.acceleration = Some(acceleration);
        self
    }

    /// Returns the framed position.
    #[must_use]
    pub const fn position(self) -> FramedPosition {
        self.position
    }

    /// Returns the framed velocity.
    #[must_use]
    pub const fn velocity(self) -> FramedVelocity {
        self.velocity
    }

    /// Returns the optional framed acceleration.
    #[must_use]
    pub const fn acceleration(self) -> Option<FramedAcceleration> {
        self.acceleration
    }
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

    #[test]
    fn cartesian_coordinates_do_not_invent_spacecraft_properties() {
        let position = FramedPosition::new(
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite position");
        let velocity = FramedVelocity::new(
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite velocity");
        let state = CartesianCoordinates::new(position, velocity);

        assert_eq!(state.position().frame(), ReferenceFrame::GCRF);
        assert!(state.acceleration().is_none());
    }
}
