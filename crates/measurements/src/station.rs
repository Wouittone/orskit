//! Ground participants used by measurement workflows.

use frames::{CustomFrameId, DerivedFrame, FrameDefinitionError, ReferenceFrame};
use thiserror::Error;
use units::Position;

/// A fixed ground measurement participant defined relative to a parent frame.
///
/// The station location is a Cartesian offset from the parent origin, expressed
/// in the parent axes. For an Earth site the parent will commonly be an
/// Earth-fixed frame such as [`ReferenceFrame::ITRF2020`]. This initial type is
/// body-agnostic: lunar and planetary sites use the same contract.
///
/// Geodetic coordinates, ellipsoid conversion, plate displacement, clocks,
/// weather, and antenna phase centers are deliberately not inferred by this
/// constructor and remain future measurement-domain capabilities.
///
/// ```
/// use frames::{CustomFrameId, ReferenceFrame};
/// use measurements::GroundStation;
/// use units::Position;
///
/// let station = GroundStation::new(
///     "TLS-01",
///     CustomFrameId::new(7001),
///     ReferenceFrame::ITRF2020,
///     Position::from_metres(4_201_000.0, 172_000.0, 4_780_000.0),
/// )?;
/// assert_eq!(station.parent_frame(), ReferenceFrame::ITRF2020);
/// # Ok::<(), measurements::GroundStationError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GroundStation {
    id: String,
    frame: DerivedFrame,
}

impl GroundStation {
    /// Creates a station and its parent-aligned local frame.
    pub fn new(
        id: impl Into<String>,
        frame_id: CustomFrameId,
        parent: ReferenceFrame,
        position_in_parent: Position,
    ) -> Result<Self, GroundStationError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(GroundStationError::EmptyId);
        }
        Ok(Self {
            id,
            frame: DerivedFrame::parent_aligned(frame_id, parent, position_in_parent)?,
        })
    }

    /// Returns the stable application-defined station identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the station-local frame identity.
    #[must_use]
    pub const fn reference_frame(&self) -> ReferenceFrame {
        self.frame.reference_frame()
    }

    /// Returns the frame in which the station position is expressed.
    #[must_use]
    pub const fn parent_frame(&self) -> ReferenceFrame {
        self.frame.parent()
    }

    /// Returns the station position in its parent frame.
    #[must_use]
    pub const fn position_in_parent(&self) -> Position {
        self.frame.origin_offset()
    }

    /// Returns the complete parent-relative frame definition.
    #[must_use]
    pub const fn frame_definition(&self) -> DerivedFrame {
        self.frame
    }
}

/// Invalid ground-station definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GroundStationError {
    /// The station identifier contains no non-whitespace characters.
    #[error("ground-station identifier must not be empty")]
    EmptyId,
    /// The station's parent-relative frame definition is invalid.
    #[error(transparent)]
    InvalidFrame(#[from] FrameDefinitionError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{Body, FrameOrigin};

    #[test]
    fn earth_station_is_a_parent_relative_measurement_participant() {
        let position = Position::from_metres(4_201_000.0, 172_000.0, 4_780_000.0);
        let station = GroundStation::new(
            "TLS-01",
            CustomFrameId::new(7001),
            ReferenceFrame::ITRF2020,
            position,
        )
        .expect("finite station definition");

        assert_eq!(station.id(), "TLS-01");
        assert_eq!(station.parent_frame(), ReferenceFrame::ITRF2020);
        assert_eq!(station.position_in_parent(), position);
        assert_eq!(
            station.reference_frame().origin(),
            FrameOrigin::Custom(CustomFrameId::new(7001))
        );
        assert_eq!(
            station.reference_frame().orientation(),
            ReferenceFrame::ITRF2020.orientation()
        );
    }

    #[test]
    fn station_contract_is_body_agnostic() {
        let mars_fixed = ReferenceFrame::new(
            FrameOrigin::Body(Body::MARS),
            frames::FrameOrientation::custom(
                CustomFrameId::new(8000),
                frames::FrameMotion::NonInertial,
            ),
        );
        let station = GroundStation::new(
            "MARS-SITE",
            CustomFrameId::new(8001),
            mars_fixed,
            Position::from_metres(3_390_000.0, 0.0, 0.0),
        )
        .expect("planetary surface site");

        assert_eq!(station.parent_frame(), mars_fixed);
    }

    #[test]
    fn station_rejects_empty_identity_and_non_finite_position() {
        assert_eq!(
            GroundStation::new(
                "   ",
                CustomFrameId::new(1),
                ReferenceFrame::ITRF2020,
                Position::from_metres(1.0, 2.0, 3.0),
            ),
            Err(GroundStationError::EmptyId)
        );
        assert_eq!(
            GroundStation::new(
                "BAD-SITE",
                CustomFrameId::new(2),
                ReferenceFrame::ITRF2020,
                Position::from_metres(1.0, f64::INFINITY, 3.0),
            ),
            Err(GroundStationError::InvalidFrame(
                FrameDefinitionError::NonFiniteOriginOffset
            ))
        );
    }
}
