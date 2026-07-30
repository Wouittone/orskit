#![forbid(unsafe_code)]

//! Epoch- and frame-explicit spacecraft attitude providers.
//!
//! Providers consume a complete epoch-qualified orbit rather than a bare
//! timestamp so prescribed pointing laws can depend on the selected state
//! representation without hidden state lookup. This first slice supplies a
//! constant provider and a bounded tabulated provider. Tabulated orientations
//! use shortest-arc spherical linear interpolation while body angular velocity
//! is linearly interpolated in the shared body axes.
//!
//! Spherical quaternion interpolation follows Ken Shoemake,
//! ["Animating Rotation with Quaternion
//! Curves"](https://doi.org/10.1145/325334.325242), *Computer Graphics*
//! 19(3), 1985. Only the published interpolation concept is used; the
//! implementation delegates the numerical kernel to `nalgebra` through the
//! domain-level [`orskit_core::Orientation::slerp`] operation.

use std::fmt;

use hifitime::Epoch;
use orskit_core::{
    Attitude, AttitudeError, BodyAngularVelocity, Orbit, OrientationInterpolationError,
    QuaternionAttitude, SpacecraftState,
};
use thiserror::Error;
use units::uom::si::ratio::ratio;
use units::{AngularVelocityVector, Ratio};

/// Produces an owned attitude for one epoch-qualified orbit.
///
/// Implementations own every immutable sample, law, transform, or ephemeris
/// dependency they require. The returned orientation must map from the
/// provider's spacecraft body frame into the orbit state frame.
pub trait AttitudeProvider<S>: fmt::Debug + Send + Sync
where
    S: SpacecraftState,
{
    /// Concrete attitude representation returned by this provider.
    type Attitude: Attitude;
    /// Typed coverage, frame, or evaluation failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Evaluates the attitude at the orbit epoch.
    fn attitude(&self, orbit: &Orbit<S>) -> Result<Self::Attitude, Self::Error>;
}

/// A constant quaternion attitude provider.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedAttitudeProvider {
    attitude: QuaternionAttitude,
}

impl FixedAttitudeProvider {
    /// Creates a provider from one validated body-to-reference attitude.
    ///
    /// A constant orientation must have zero angular velocity relative to its
    /// target frame.
    pub fn new(attitude: QuaternionAttitude) -> Result<Self, FixedAttitudeProviderError> {
        if attitude
            .angular_velocity()
            .value()
            .to_radians_per_second()
            .into_iter()
            .any(|component| component != 0.0)
        {
            return Err(FixedAttitudeProviderError::NonZeroAngularVelocity);
        }
        Ok(Self { attitude })
    }

    /// Returns the constant attitude.
    #[must_use]
    pub const fn fixed_attitude(&self) -> &QuaternionAttitude {
        &self.attitude
    }
}

impl<S> AttitudeProvider<S> for FixedAttitudeProvider
where
    S: SpacecraftState,
{
    type Attitude = QuaternionAttitude;
    type Error = FixedAttitudeProviderError;

    fn attitude(&self, orbit: &Orbit<S>) -> Result<Self::Attitude, Self::Error> {
        if self.attitude.orientation().target_frame() != orbit.as_ref().frame() {
            return Err(FixedAttitudeProviderError::ReferenceFrameMismatch);
        }
        Ok(self.attitude.clone())
    }
}

/// Failure evaluating a fixed attitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FixedAttitudeProviderError {
    /// A constant orientation was paired with a changing body angle.
    #[error("fixed attitude must have zero body angular velocity")]
    NonZeroAngularVelocity,
    /// The constant attitude is expressed relative to another orbit frame.
    #[error("fixed-attitude reference frame does not match the orbit frame")]
    ReferenceFrameMismatch,
}

/// One attitude sample valid at an exact epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct AttitudeSample {
    epoch: Epoch,
    attitude: QuaternionAttitude,
}

impl AttitudeSample {
    /// Associates a validated quaternion attitude with its epoch.
    #[must_use]
    pub const fn new(epoch: Epoch, attitude: QuaternionAttitude) -> Self {
        Self { epoch, attitude }
    }

    /// Returns the sample epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the sampled attitude.
    #[must_use]
    pub const fn attitude(&self) -> &QuaternionAttitude {
        &self.attitude
    }
}

/// Bounded piecewise attitude interpolation over ordered samples.
#[derive(Debug, Clone, PartialEq)]
pub struct TabulatedAttitudeProvider {
    samples: Vec<AttitudeSample>,
}

impl TabulatedAttitudeProvider {
    /// Validates a non-empty, strictly increasing attitude table.
    ///
    /// Every sample must use the same spacecraft body capability and the same
    /// body-to-reference frame endpoints. A one-sample table has coverage only
    /// at that exact epoch.
    pub fn new(samples: Vec<AttitudeSample>) -> Result<Self, TabulatedAttitudeConfigurationError> {
        let Some(first) = samples.first() else {
            return Err(TabulatedAttitudeConfigurationError::Empty);
        };
        if samples
            .windows(2)
            .any(|pair| pair[1].epoch <= pair[0].epoch)
        {
            return Err(TabulatedAttitudeConfigurationError::EpochsNotStrictlyIncreasing);
        }

        let source_frame = first.attitude.orientation().source_frame();
        let target_frame = first.attitude.orientation().target_frame();
        let body = first.attitude.angular_velocity().body_frame_capability();
        for sample in samples.iter().skip(1) {
            if sample.attitude.orientation().source_frame() != source_frame {
                return Err(TabulatedAttitudeConfigurationError::SourceFrameMismatch);
            }
            if sample.attitude.orientation().target_frame() != target_frame {
                return Err(TabulatedAttitudeConfigurationError::TargetFrameMismatch);
            }
            if sample.attitude.angular_velocity().body_frame_capability() != body {
                return Err(TabulatedAttitudeConfigurationError::BodyCapabilityMismatch);
            }
        }
        Ok(Self { samples })
    }

    /// Returns the ordered source samples.
    #[must_use]
    pub fn samples(&self) -> &[AttitudeSample] {
        &self.samples
    }

    /// Returns the closed epoch coverage interval.
    #[must_use]
    pub fn coverage(&self) -> (Epoch, Epoch) {
        (
            self.samples
                .first()
                .expect("validated provider has at least one sample")
                .epoch,
            self.samples
                .last()
                .expect("validated provider has at least one sample")
                .epoch,
        )
    }
}

impl<S> AttitudeProvider<S> for TabulatedAttitudeProvider
where
    S: SpacecraftState,
{
    type Attitude = QuaternionAttitude;
    type Error = TabulatedAttitudeProviderError;

    fn attitude(&self, orbit: &Orbit<S>) -> Result<Self::Attitude, Self::Error> {
        let epoch = orbit.epoch();
        let (coverage_start, coverage_end) = self.coverage();
        if epoch < coverage_start || epoch > coverage_end {
            return Err(TabulatedAttitudeProviderError::OutsideCoverage {
                requested: epoch,
                coverage_start,
                coverage_end,
            });
        }
        let target_frame = self.samples[0].attitude.orientation().target_frame();
        if target_frame != orbit.as_ref().frame() {
            return Err(TabulatedAttitudeProviderError::ReferenceFrameMismatch);
        }

        match self
            .samples
            .binary_search_by_key(&epoch, AttitudeSample::epoch)
        {
            Ok(index) => Ok(self.samples[index].attitude.clone()),
            Err(right) => {
                interpolate_samples(&self.samples[right - 1], &self.samples[right], epoch)
            }
        }
    }
}

fn interpolate_samples(
    start: &AttitudeSample,
    end: &AttitudeSample,
    epoch: Epoch,
) -> Result<QuaternionAttitude, TabulatedAttitudeProviderError> {
    let interval_seconds = (end.epoch - start.epoch).to_seconds();
    let fraction = (epoch - start.epoch).to_seconds() / interval_seconds;
    let orientation = start
        .attitude
        .orientation()
        .slerp(end.attitude.orientation(), Ratio::new::<ratio>(fraction))
        .map_err(TabulatedAttitudeProviderError::OrientationInterpolation)?;

    let start_rate = start
        .attitude
        .angular_velocity()
        .value()
        .to_radians_per_second();
    let end_rate = end
        .attitude
        .angular_velocity()
        .value()
        .to_radians_per_second();
    let rate: [f64; 3] = std::array::from_fn(|index| {
        start_rate[index] + fraction * (end_rate[index] - start_rate[index])
    });
    let angular_velocity = BodyAngularVelocity::new(
        AngularVelocityVector::from_radians_per_second(rate[0], rate[1], rate[2]),
        start
            .attitude
            .angular_velocity()
            .body_frame_capability()
            .clone(),
        orientation.target_frame(),
    )
    .map_err(TabulatedAttitudeProviderError::Attitude)?;
    QuaternionAttitude::new(orientation, angular_velocity)
        .map_err(TabulatedAttitudeProviderError::Attitude)
}

/// Invalid tabulated-provider construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TabulatedAttitudeConfigurationError {
    /// No source samples were supplied.
    #[error("tabulated attitude requires at least one sample")]
    Empty,
    /// Sample epochs contain a duplicate or move backward.
    #[error("tabulated attitude sample epochs must be strictly increasing")]
    EpochsNotStrictlyIncreasing,
    /// Orientation source frames differ across samples.
    #[error("tabulated attitude source frames must match")]
    SourceFrameMismatch,
    /// Orientation target frames differ across samples.
    #[error("tabulated attitude target frames must match")]
    TargetFrameMismatch,
    /// Samples do not carry the same opaque spacecraft/body ownership proof.
    #[error("tabulated attitude body-frame capabilities must match")]
    BodyCapabilityMismatch,
}

/// Failure evaluating a tabulated attitude.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TabulatedAttitudeProviderError {
    /// The requested epoch lies outside the closed sample interval.
    #[error("attitude epoch {requested} lies outside coverage [{coverage_start}, {coverage_end}]")]
    OutsideCoverage {
        /// Requested orbit epoch.
        requested: Epoch,
        /// First available sample epoch.
        coverage_start: Epoch,
        /// Last available sample epoch.
        coverage_end: Epoch,
    },
    /// The table reference frame differs from the requested orbit frame.
    #[error("tabulated-attitude reference frame does not match the orbit frame")]
    ReferenceFrameMismatch,
    /// Framed quaternion interpolation failed.
    #[error("tabulated orientation interpolation failed")]
    OrientationInterpolation(#[source] OrientationInterpolationError),
    /// Interpolated angular state failed domain validation.
    #[error("interpolated attitude failed validation")]
    Attitude(#[source] AttitudeError),
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_1_SQRT_2;

    use hifitime::Unit;
    use orskit_core::frames::{
        CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin, ReferenceFrame,
    };
    use orskit_core::{
        BodyAngularVelocity, FramedForce, Orientation, OrientationQuaternion, SpacecraftBodyFrame,
    };
    use units::uom::si::ratio::ratio;

    use super::*;

    #[derive(Debug)]
    struct TestState(ReferenceFrame);

    impl SpacecraftState for TestState {
        fn frame(&self) -> ReferenceFrame {
            self.0
        }
    }

    fn body(id: u64, spacecraft_id: &str) -> SpacecraftBodyFrame {
        let id = CustomFrameId::new(id);
        SpacecraftBodyFrame::new(
            spacecraft_id.to_owned(),
            ReferenceFrame::new(
                FrameOrigin::Custom(id),
                FrameOrientation::custom(id, FrameMotion::NonInertial),
            ),
        )
        .expect("spacecraft-owned body frame")
    }

    fn attitude(
        body: &SpacecraftBodyFrame,
        quaternion: [f64; 4],
        rate: [f64; 3],
        target: ReferenceFrame,
    ) -> QuaternionAttitude {
        let orientation = Orientation::try_from(OrientationQuaternion {
            source_frame: body.reference_frame(),
            target_frame: target,
            components: quaternion.map(Ratio::new::<ratio>),
        })
        .expect("finite non-zero quaternion");
        let angular_velocity = BodyAngularVelocity::new(
            AngularVelocityVector::from_radians_per_second(rate[0], rate[1], rate[2]),
            body.clone(),
            target,
        )
        .expect("consistent angular velocity");
        QuaternionAttitude::new(orientation, angular_velocity).expect("consistent attitude")
    }

    fn orbit(epoch: Epoch, frame: ReferenceFrame) -> Orbit<TestState> {
        Orbit::new(epoch, TestState(frame))
    }

    #[test]
    fn fixed_provider_checks_the_requested_reference_frame() {
        let body = body(1, "fixed");
        let provider = FixedAttitudeProvider::new(attitude(
            &body,
            [1.0, 0.0, 0.0, 0.0],
            [0.0; 3],
            ReferenceFrame::GCRF,
        ))
        .expect("zero-rate fixed attitude");
        let epoch = Epoch::from_tai_seconds(100.0);

        assert!(provider
            .attitude(&orbit(epoch, ReferenceFrame::GCRF))
            .is_ok());
        assert_eq!(
            provider.attitude(&orbit(epoch, ReferenceFrame::EME2000)),
            Err(FixedAttitudeProviderError::ReferenceFrameMismatch)
        );
        assert_eq!(
            FixedAttitudeProvider::new(attitude(
                &body,
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                ReferenceFrame::GCRF,
            )),
            Err(FixedAttitudeProviderError::NonZeroAngularVelocity)
        );
    }

    #[test]
    fn table_slerps_orientation_and_linearly_interpolates_body_rate() {
        let body = body(2, "table");
        let start = Epoch::from_tai_seconds(200.0);
        let provider = TabulatedAttitudeProvider::new(vec![
            AttitudeSample::new(
                start,
                attitude(
                    &body,
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    ReferenceFrame::GCRF,
                ),
            ),
            AttitudeSample::new(
                start + 10.0 * Unit::Second,
                attitude(
                    &body,
                    [0.0, 0.0, 0.0, 1.0],
                    [0.0, 0.0, 2.0],
                    ReferenceFrame::GCRF,
                ),
            ),
        ])
        .expect("valid table");

        let midpoint = provider
            .attitude(&orbit(start + 5.0 * Unit::Second, ReferenceFrame::GCRF))
            .expect("covered interpolation");
        let body_force = FramedForce::new(
            [
                units::Force::new::<units::uom::si::force::newton>(1.0),
                units::Force::new::<units::uom::si::force::newton>(0.0),
                units::Force::new::<units::uom::si::force::newton>(0.0),
            ],
            body.reference_frame(),
        )
        .expect("finite body force");
        let rotated = midpoint
            .orientation()
            .rotate_force(body_force)
            .expect("matching source frame");
        assert_eq!(rotated.frame(), ReferenceFrame::GCRF);
        let values = rotated
            .components()
            .map(|value| value.get::<units::uom::si::force::newton>());
        assert!(values[0].abs() < 2.0e-15);
        assert!((values[1] - 1.0).abs() < 2.0e-15);
        assert!(values[2].abs() < 2.0e-15);
        let wrong_frame_force = FramedForce::new(
            [
                units::Force::new::<units::uom::si::force::newton>(1.0),
                units::Force::new::<units::uom::si::force::newton>(0.0),
                units::Force::new::<units::uom::si::force::newton>(0.0),
            ],
            ReferenceFrame::GCRF,
        )
        .expect("finite force");
        assert!(matches!(
            midpoint.orientation().rotate_force(wrong_frame_force),
            Err(orskit_core::OrientationForceError::SourceFrameMismatch)
        ));
        assert_eq!(
            midpoint.angular_velocity().value().to_radians_per_second(),
            [0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn equivalent_quaternion_signs_do_not_create_a_spurious_rotation() {
        let body = body(3, "sign");
        let start = Epoch::from_tai_seconds(300.0);
        let provider = TabulatedAttitudeProvider::new(vec![
            AttitudeSample::new(
                start,
                attitude(
                    &body,
                    [FRAC_1_SQRT_2, 0.0, 0.0, FRAC_1_SQRT_2],
                    [0.0; 3],
                    ReferenceFrame::GCRF,
                ),
            ),
            AttitudeSample::new(
                start + 2.0 * Unit::Second,
                attitude(
                    &body,
                    [-FRAC_1_SQRT_2, 0.0, 0.0, -FRAC_1_SQRT_2],
                    [0.0; 3],
                    ReferenceFrame::GCRF,
                ),
            ),
        ])
        .expect("valid table");

        let midpoint = provider
            .attitude(&orbit(start + 1.0 * Unit::Second, ReferenceFrame::GCRF))
            .expect("equivalent endpoints");
        let quaternion = midpoint.orientation().quaternion();
        let norm = quaternion
            .map(|value| value.get::<ratio>())
            .into_iter()
            .map(|value| value * value)
            .sum::<f64>();
        assert!((norm - 1.0).abs() < 2.0e-15);
        assert!(
            (midpoint.angles()[2].get::<units::uom::si::angle::radian>()
                - std::f64::consts::FRAC_PI_2)
                .abs()
                < 2.0e-15
        );
    }

    #[test]
    fn table_rejects_invalid_order_and_reports_closed_coverage() {
        let body = body(4, "coverage");
        let epoch = Epoch::from_tai_seconds(400.0);
        let sample = || {
            AttitudeSample::new(
                epoch,
                attitude(&body, [1.0, 0.0, 0.0, 0.0], [0.0; 3], ReferenceFrame::GCRF),
            )
        };
        assert_eq!(
            TabulatedAttitudeProvider::new(vec![sample(), sample()]),
            Err(TabulatedAttitudeConfigurationError::EpochsNotStrictlyIncreasing)
        );

        let provider =
            TabulatedAttitudeProvider::new(vec![sample()]).expect("one-point table is valid");
        assert!(matches!(
            provider.attitude(&orbit(epoch + 1.0 * Unit::Second, ReferenceFrame::GCRF)),
            Err(TabulatedAttitudeProviderError::OutsideCoverage { .. })
        ));
        assert!(provider
            .attitude(&orbit(epoch, ReferenceFrame::GCRF))
            .is_ok());
    }

    #[test]
    fn table_rejects_frame_and_body_ownership_disagreement() {
        let body_a = body(5, "owner-a");
        let body_b = body(5, "owner-b");
        let start = Epoch::from_tai_seconds(500.0);
        let sample = |epoch, body: &SpacecraftBodyFrame, target| {
            AttitudeSample::new(
                epoch,
                attitude(body, [1.0, 0.0, 0.0, 0.0], [0.0; 3], target),
            )
        };

        assert_eq!(
            TabulatedAttitudeProvider::new(vec![
                sample(start, &body_a, ReferenceFrame::GCRF),
                sample(start + 1.0 * Unit::Second, &body_a, ReferenceFrame::EME2000,),
            ]),
            Err(TabulatedAttitudeConfigurationError::TargetFrameMismatch)
        );
        assert_eq!(
            TabulatedAttitudeProvider::new(vec![
                sample(start, &body_a, ReferenceFrame::GCRF),
                sample(start + 1.0 * Unit::Second, &body_b, ReferenceFrame::GCRF,),
            ]),
            Err(TabulatedAttitudeConfigurationError::BodyCapabilityMismatch)
        );

        let provider = TabulatedAttitudeProvider::new(vec![
            sample(start, &body_a, ReferenceFrame::GCRF),
            sample(start + 1.0 * Unit::Second, &body_a, ReferenceFrame::GCRF),
        ])
        .expect("consistent table");
        assert!(matches!(
            provider.attitude(&orbit(start, ReferenceFrame::EME2000)),
            Err(TabulatedAttitudeProviderError::ReferenceFrameMismatch)
        ));
    }
}
