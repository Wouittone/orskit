//! Typed, participant-qualified measurement data structures.

use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::length::meter;
use units::Length;

use crate::{ParticipantId, SignalPath};

/// Signal event whose epoch is attached to an observation.
///
/// Transmit and receive refer to the first and last entries in the observation's
/// [`SignalPath`]. An intermediate index refers to neither endpoint and is
/// validated when the measurement is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ObservationTimeTag {
    /// Epoch at the first participant's transmission event.
    Transmit,
    /// Epoch at the last participant's reception event.
    Receive,
    /// Epoch at an intermediate participant event, such as a relay or turnaround.
    AtIntermediateParticipant {
        /// Zero-based index into the signal path.
        index: usize,
    },
}

/// Meaning of the scalar stored in a range observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RangeConvention {
    /// Sum of geometric path lengths over every leg in the signal path.
    PathLength,
    /// Half of a two-leg returning path length.
    ///
    /// This convention requires exactly three signal events whose first and
    /// last participant identities are equal.
    RoundTripOneWayEquivalent,
}

/// A participant-qualified scalar range observation and one-sigma uncertainty.
///
/// The signal path records participant event order. `time_tag` states which
/// event owns `epoch`, and `convention` states whether `range` is the complete
/// path length or the conventional one-way equivalent of a two-leg round trip.
/// No transmit, receive, or turnaround time is inferred.
///
/// ```
/// use hifitime::Epoch;
/// use measurements::{
///     ObservationTimeTag, ParticipantId, RangeConvention, RangeMeasurement, SignalPath,
/// };
/// use units::{uom::si::length::meter, Length};
///
/// let station = ParticipantId::new("DSS-14")?;
/// let spacecraft = ParticipantId::new("SC-01")?;
/// let path = SignalPath::new(vec![station, spacecraft])?;
/// let range = RangeMeasurement::new(
///     path,
///     Epoch::from_tai_seconds(0.0),
///     ObservationTimeTag::Receive,
///     RangeConvention::PathLength,
///     Length::new::<meter>(42_000_000.0),
///     Length::new::<meter>(3.0),
/// )?;
/// assert_eq!(range.tagged_participant().as_str(), "SC-01");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RangeMeasurement {
    path: SignalPath,
    epoch: Epoch,
    time_tag: ObservationTimeTag,
    convention: RangeConvention,
    range: Length,
    uncertainty: Length,
}

impl RangeMeasurement {
    /// Constructs a fully qualified range observation.
    pub fn new(
        path: SignalPath,
        epoch: Epoch,
        time_tag: ObservationTimeTag,
        convention: RangeConvention,
        range: Length,
        uncertainty: Length,
    ) -> Result<Self, MeasurementError> {
        let participant_count = path.participant_count();
        if let ObservationTimeTag::AtIntermediateParticipant { index } = time_tag {
            if index == 0 || index >= participant_count - 1 {
                return Err(MeasurementError::InvalidIntermediateTimeTag {
                    index,
                    participant_count,
                });
            }
        }
        if convention == RangeConvention::RoundTripOneWayEquivalent
            && (participant_count != 3
                || path.participant(0) != path.participant(participant_count - 1))
        {
            return Err(MeasurementError::InvalidRoundTripPath);
        }

        let range_m = range.get::<meter>();
        let uncertainty_m = uncertainty.get::<meter>();
        if !range_m.is_finite() || !uncertainty_m.is_finite() {
            return Err(MeasurementError::NonFinite);
        }
        if range_m < 0.0 {
            return Err(MeasurementError::NegativeRange);
        }
        if uncertainty_m <= 0.0 {
            return Err(MeasurementError::NotPositiveUncertainty);
        }
        Ok(Self {
            path,
            epoch,
            time_tag,
            convention,
            range,
            uncertainty,
        })
    }

    /// Returns the ordered measurement signal path.
    #[must_use]
    pub const fn path(&self) -> &SignalPath {
        &self.path
    }

    /// Returns the observation epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns which signal event owns the observation epoch.
    #[must_use]
    pub const fn time_tag(&self) -> ObservationTimeTag {
        self.time_tag
    }

    /// Returns the index of the participant event owning the observation epoch.
    #[must_use]
    pub fn tagged_participant_index(&self) -> usize {
        match self.time_tag {
            ObservationTimeTag::Transmit => 0,
            ObservationTimeTag::Receive => self.path.participant_count() - 1,
            ObservationTimeTag::AtIntermediateParticipant { index } => index,
        }
    }

    /// Returns the participant whose signal event owns the observation epoch.
    #[must_use]
    pub fn tagged_participant(&self) -> &ParticipantId {
        self.path
            .participant(self.tagged_participant_index())
            .expect("validated observation time tag must identify a path participant")
    }

    /// Returns the declared scalar range convention.
    #[must_use]
    pub const fn convention(&self) -> RangeConvention {
        self.convention
    }

    /// Returns the measured range.
    #[must_use]
    pub const fn range(&self) -> Length {
        self.range
    }

    /// Returns the one-sigma range uncertainty.
    #[must_use]
    pub const fn uncertainty(&self) -> Length {
        self.uncertainty
    }
}

/// Invalid measurement input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MeasurementError {
    /// An intermediate time tag referred to an endpoint or an absent event.
    #[error(
        "observation time-tag index {index} is not intermediate in a {participant_count}-participant path"
    )]
    InvalidIntermediateTimeTag {
        /// Requested signal-event index.
        index: usize,
        /// Number of participant events in the path.
        participant_count: usize,
    },
    /// One-way-equivalent round-trip semantics were attached to another topology.
    #[error(
        "round-trip one-way-equivalent range requires a three-event path with matching endpoints"
    )]
    InvalidRoundTripPath,
    /// The measurement or uncertainty is NaN or infinite.
    #[error("measurement values must be finite")]
    NonFinite,
    /// A geometric range cannot be negative.
    #[error("measurement range must be non-negative")]
    NegativeRange,
    /// One-sigma uncertainty is zero or negative.
    #[error("measurement uncertainty must be strictly positive")]
    NotPositiveUncertainty,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ParticipantId {
        ParticipantId::new(value).expect("test participant identity is valid")
    }

    fn one_way_path() -> SignalPath {
        SignalPath::new(vec![id("DSS-14"), id("SC-01")]).expect("one-way path")
    }

    fn measurement(
        path: SignalPath,
        time_tag: ObservationTimeTag,
        convention: RangeConvention,
    ) -> Result<RangeMeasurement, MeasurementError> {
        RangeMeasurement::new(
            path,
            Epoch::from_tai_seconds(0.0),
            time_tag,
            convention,
            Length::new::<meter>(1.0),
            Length::new::<meter>(0.1),
        )
    }

    #[test]
    fn participant_order_and_time_tag_are_measurement_identity() {
        let forward = measurement(
            one_way_path(),
            ObservationTimeTag::Receive,
            RangeConvention::PathLength,
        )
        .expect("qualified range");
        let reverse = measurement(
            SignalPath::new(vec![id("SC-01"), id("DSS-14")]).expect("reverse path"),
            ObservationTimeTag::Receive,
            RangeConvention::PathLength,
        )
        .expect("qualified reverse range");
        let transmit_tagged = measurement(
            one_way_path(),
            ObservationTimeTag::Transmit,
            RangeConvention::PathLength,
        )
        .expect("transmit-tagged range");

        assert_ne!(forward, reverse);
        assert_ne!(forward, transmit_tagged);
        assert_eq!(forward.tagged_participant().as_str(), "SC-01");
        assert_eq!(transmit_tagged.tagged_participant().as_str(), "DSS-14");
    }

    #[test]
    fn intermediate_time_tag_must_name_an_intermediate_event() {
        for index in [0, 1, 2] {
            assert_eq!(
                measurement(
                    one_way_path(),
                    ObservationTimeTag::AtIntermediateParticipant { index },
                    RangeConvention::PathLength,
                ),
                Err(MeasurementError::InvalidIntermediateTimeTag {
                    index,
                    participant_count: 2,
                })
            );
        }

        let station = id("DSS-14");
        let returning_path =
            SignalPath::new(vec![station.clone(), id("SC-01"), station]).expect("return path");
        let observation = measurement(
            returning_path,
            ObservationTimeTag::AtIntermediateParticipant { index: 1 },
            RangeConvention::RoundTripOneWayEquivalent,
        )
        .expect("turnaround-tagged two-way range");
        assert_eq!(observation.tagged_participant().as_str(), "SC-01");
    }

    #[test]
    fn one_way_equivalent_convention_requires_two_leg_return_path() {
        assert_eq!(
            measurement(
                one_way_path(),
                ObservationTimeTag::Receive,
                RangeConvention::RoundTripOneWayEquivalent,
            ),
            Err(MeasurementError::InvalidRoundTripPath)
        );
        assert_eq!(
            measurement(
                SignalPath::new(vec![id("DSS-14"), id("SC-01"), id("RELAY")])
                    .expect("three-event path"),
                ObservationTimeTag::Receive,
                RangeConvention::RoundTripOneWayEquivalent,
            ),
            Err(MeasurementError::InvalidRoundTripPath)
        );
    }

    #[test]
    fn range_uncertainty_must_be_positive() {
        assert_eq!(
            RangeMeasurement::new(
                one_way_path(),
                Epoch::from_tai_seconds(0.0),
                ObservationTimeTag::Receive,
                RangeConvention::PathLength,
                Length::new::<meter>(1.0),
                Length::new::<meter>(0.0),
            ),
            Err(MeasurementError::NotPositiveUncertainty)
        );
    }

    #[test]
    fn range_must_not_be_negative() {
        assert_eq!(
            RangeMeasurement::new(
                one_way_path(),
                Epoch::from_tai_seconds(0.0),
                ObservationTimeTag::Receive,
                RangeConvention::PathLength,
                Length::new::<meter>(-1.0),
                Length::new::<meter>(1.0),
            ),
            Err(MeasurementError::NegativeRange)
        );
    }
}
