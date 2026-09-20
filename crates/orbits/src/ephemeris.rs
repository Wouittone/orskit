use std::{error::Error as StdError, fmt};

use bodies::Body;
use frames::{FrameKinematics, ReferenceFrame};
use hifitime::Epoch;
use thiserror::Error;

/// Position and velocity of one celestial body at one epoch.
///
/// The kinematics are expressed in the complete reference frame carried by
/// [`FrameKinematics`], including both its origin and orientation. This value
/// does not imply a gravitational parameter, interpolation method, data set,
/// or force model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyState {
    body: Body,
    epoch: Epoch,
    kinematics: FrameKinematics,
}

impl BodyState {
    /// Associates a body and epoch with validated, frame-qualified kinematics.
    #[must_use]
    pub const fn new(body: Body, epoch: Epoch, kinematics: FrameKinematics) -> Self {
        Self {
            body,
            epoch,
            kinematics,
        }
    }

    /// Returns the body whose center is represented.
    #[must_use]
    pub const fn body(self) -> Body {
        self.body
    }

    /// Returns the epoch at which the state is valid.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Returns the finite position and velocity with their expression frame.
    #[must_use]
    pub const fn kinematics(self) -> FrameKinematics {
        self.kinematics
    }
}

/// Resolves celestial-body states from a caller-selected ephemeris.
///
/// The requested body, epoch (including its time scale), and complete
/// expression frame are explicit on every evaluation. Implementations own
/// their immutable samples, interpolation, coverage, caching, and data
/// provenance. They must not select or download scientific data through
/// ambient process state.
///
/// A successful result must identify the requested `body` and `epoch`, and its
/// kinematics must carry the requested `frame`. Consumers may reject a
/// provider that violates those result invariants.
///
/// The trait is object-safe, so an application can supply
/// `Box<dyn BodyEphemerisProvider>` without coupling consumers to a file
/// format, network client, or concrete almanac.
///
/// ```
/// use bodies::Body;
/// use frames::ReferenceFrame;
/// use hifitime::Epoch;
/// use orbits::cartesian::{BodyEphemerisError, BodyEphemerisProvider, BodyState};
///
/// fn moon_state(
///     ephemeris: &dyn BodyEphemerisProvider,
///     epoch: Epoch,
/// ) -> Result<BodyState, BodyEphemerisError> {
///     ephemeris.state_at(Body::MOON, epoch, ReferenceFrame::ICRF)
/// }
/// ```
pub trait BodyEphemerisProvider: fmt::Debug + Send + Sync {
    /// Evaluates one body's center at `epoch` in `frame`.
    fn state_at(
        &self,
        body: Body,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<BodyState, BodyEphemerisError>;
}

/// Failure resolving an explicitly requested celestial-body state.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BodyEphemerisError {
    /// The selected ephemeris does not contain the requested body.
    #[error("ephemeris does not support body {body}")]
    UnsupportedBody {
        /// Body requested by the caller.
        body: Body,
    },
    /// The selected ephemeris cannot express the body state in the requested frame.
    #[error("ephemeris cannot express body {body} in frame {frame:?}")]
    UnsupportedFrame {
        /// Body requested by the caller.
        body: Body,
        /// Expression frame requested by the caller.
        frame: ReferenceFrame,
    },
    /// The requested epoch lies outside the selected ephemeris coverage.
    #[error("ephemeris has no coverage for body {body} at {epoch}")]
    OutsideCoverage {
        /// Body requested by the caller.
        body: Body,
        /// Epoch requested by the caller, boxed to keep the error compact.
        epoch: Box<Epoch>,
    },
    /// The caller-supplied provider failed while evaluating a supported request.
    #[error("ephemeris failed to evaluate body {body} at {epoch} in frame {frame:?}")]
    Evaluation {
        /// Body requested by the caller.
        body: Body,
        /// Epoch requested by the caller, boxed to keep the error compact.
        epoch: Box<Epoch>,
        /// Expression frame requested by the caller.
        frame: ReferenceFrame,
        /// Provider-specific source failure.
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use units::{Position, VelocityVector};

    #[derive(Debug)]
    struct TestEphemeris {
        state: BodyState,
    }

    impl BodyEphemerisProvider for TestEphemeris {
        fn state_at(
            &self,
            body: Body,
            epoch: Epoch,
            frame: ReferenceFrame,
        ) -> Result<BodyState, BodyEphemerisError> {
            if body != self.state.body() {
                return Err(BodyEphemerisError::UnsupportedBody { body });
            }
            if epoch != self.state.epoch() {
                return Err(BodyEphemerisError::OutsideCoverage {
                    body,
                    epoch: Box::new(epoch),
                });
            }
            if frame != self.state.kinematics().frame() {
                return Err(BodyEphemerisError::UnsupportedFrame { body, frame });
            }
            Ok(self.state)
        }
    }

    fn test_state() -> BodyState {
        let epoch = Epoch::from_tai_seconds(123.0);
        let kinematics = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ICRF,
        )
        .expect("finite body state");
        BodyState::new(Body::MOON, epoch, kinematics)
    }

    #[test]
    fn body_state_preserves_body_epoch_frame_and_typed_kinematics() {
        let state = test_state();

        assert_eq!(state.body(), Body::MOON);
        assert_eq!(state.epoch(), Epoch::from_tai_seconds(123.0));
        assert_eq!(state.kinematics().frame(), ReferenceFrame::ICRF);
        assert_eq!(
            state.kinematics().position(),
            Position::from_metres(1.0, 2.0, 3.0)
        );
        assert_eq!(
            state.kinematics().velocity(),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0)
        );
    }

    #[test]
    fn provider_is_object_safe_and_reports_request_context() {
        let provider: Box<dyn BodyEphemerisProvider> = Box::new(TestEphemeris {
            state: test_state(),
        });

        let state = provider
            .state_at(
                Body::MOON,
                Epoch::from_tai_seconds(123.0),
                ReferenceFrame::ICRF,
            )
            .expect("covered state");
        assert_eq!(state, test_state());

        assert!(matches!(
            provider.state_at(
                Body::SUN,
                Epoch::from_tai_seconds(123.0),
                ReferenceFrame::ICRF
            ),
            Err(BodyEphemerisError::UnsupportedBody { body: Body::SUN })
        ));
        assert!(matches!(
            provider.state_at(
                Body::MOON,
                Epoch::from_tai_seconds(124.0),
                ReferenceFrame::ICRF
            ),
            Err(BodyEphemerisError::OutsideCoverage {
                body: Body::MOON,
                epoch
            }) if *epoch == Epoch::from_tai_seconds(124.0)
        ));
        assert!(matches!(
            provider.state_at(
                Body::MOON,
                Epoch::from_tai_seconds(123.0),
                ReferenceFrame::GCRF
            ),
            Err(BodyEphemerisError::UnsupportedFrame {
                body: Body::MOON,
                frame: ReferenceFrame::GCRF
            })
        ));
    }

    #[test]
    fn evaluation_error_preserves_provider_source_and_request() {
        let epoch = Epoch::from_tai_seconds(456.0);
        let error = BodyEphemerisError::Evaluation {
            body: Body::MARS,
            epoch: Box::new(epoch),
            frame: ReferenceFrame::ICRF,
            source: Box::new(io::Error::new(io::ErrorKind::InvalidData, "bad segment")),
        };

        assert!(error.to_string().contains("MARS"));
        assert_eq!(
            StdError::source(&error).map(ToString::to_string),
            Some("bad segment".to_owned())
        );
        assert!(matches!(
            error,
            BodyEphemerisError::Evaluation {
                body: Body::MARS,
                epoch: actual_epoch,
                frame: ReferenceFrame::ICRF,
                ..
            } if *actual_epoch == epoch
        ));
    }
}
