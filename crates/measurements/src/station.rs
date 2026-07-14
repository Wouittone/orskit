//! Ground participants used by measurement workflows.

use frames::{DerivedFrame, ReferenceFrame};
use units::Position;

use crate::ParticipantId;

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
/// use frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
/// use measurements::{GroundStation, ParticipantId};
/// use units::Position;
///
/// let frame = FrameCatalog::new(FrameNamespace::new(7), [ReferenceFrame::ITRF2020])?
///     .define_parent_aligned(
///         7001,
///         ReferenceFrame::ITRF2020,
///         Position::from_metres(4_201_000.0, 172_000.0, 4_780_000.0),
///     )?;
/// let station = GroundStation::new(
///     ParticipantId::new("TLS-01")?,
///     frame,
/// );
/// assert_eq!(station.parent_frame(), ReferenceFrame::ITRF2020);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GroundStation {
    id: ParticipantId,
    frame: DerivedFrame,
}

impl GroundStation {
    /// Creates a station and its parent-aligned local frame.
    pub const fn new(id: ParticipantId, frame: DerivedFrame) -> Self {
        Self { id, frame }
    }

    /// Returns the stable application-defined station identifier.
    #[must_use]
    pub const fn id(&self) -> &ParticipantId {
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

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{Body, FrameCatalog, FrameNamespace, FrameOrigin};

    fn id(value: &str) -> ParticipantId {
        ParticipantId::new(value).expect("valid test participant")
    }

    #[test]
    fn earth_station_is_a_parent_relative_measurement_participant() {
        let position = Position::from_metres(4_201_000.0, 172_000.0, 4_780_000.0);
        let frame = FrameCatalog::new(FrameNamespace::new(7), [ReferenceFrame::ITRF2020])
            .expect("catalog")
            .define_parent_aligned(7001, ReferenceFrame::ITRF2020, position)
            .expect("finite station definition");
        let station = GroundStation::new(id("TLS-01"), frame);

        assert_eq!(station.id().as_str(), "TLS-01");
        assert_eq!(station.parent_frame(), ReferenceFrame::ITRF2020);
        assert_eq!(station.position_in_parent(), position);
        assert_eq!(
            station.reference_frame().origin(),
            FrameOrigin::Derived(frame.id())
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
                frames::CustomFrameId::new(8000),
                frames::FrameMotion::NonInertial,
            ),
        );
        let frame = FrameCatalog::new(FrameNamespace::new(8), [mars_fixed])
            .expect("Mars catalog")
            .define_parent_aligned(
                8001,
                mars_fixed,
                Position::from_metres(3_390_000.0, 0.0, 0.0),
            )
            .expect("planetary surface site");
        let station = GroundStation::new(id("MARS-SITE"), frame);

        assert_eq!(station.parent_frame(), mars_fixed);
    }
}
