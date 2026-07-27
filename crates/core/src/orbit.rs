use std::fmt;

use frames::ReferenceFrame;
use hifitime::Epoch;

/// Open contract for one frame-qualified spacecraft-state representation.
///
/// State implementations live in dedicated crates or in applications. The
/// contract deliberately says nothing about coordinates, force models, or
/// conversion algorithms: those are selected through composition.
pub trait SpacecraftState: fmt::Debug + Send + Sync {
    /// Returns the frame in which this state representation is expressed.
    fn frame(&self) -> ReferenceFrame;
}

/// A spacecraft state qualified by the epoch at which it is valid.
///
/// `Orbit` owns no coordinate implementation. Its state type is selected by
/// the caller, preserving the native representation through generic workflows.
///
/// ```
/// use frames::ReferenceFrame;
/// use hifitime::Epoch;
/// use orskit_core::{Orbit, SpacecraftState};
///
/// #[derive(Debug)]
/// struct ApplicationState(ReferenceFrame);
///
/// impl SpacecraftState for ApplicationState {
///     fn frame(&self) -> ReferenceFrame {
///         self.0
///     }
/// }
///
/// let epoch = Epoch::from_gregorian_tai_at_midnight(2026, 1, 1);
/// let orbit = Orbit::new(epoch, ApplicationState(ReferenceFrame::GCRF));
///
/// assert_eq!(orbit.epoch(), epoch);
/// assert_eq!(orbit.as_ref().frame(), ReferenceFrame::GCRF);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Orbit<S: SpacecraftState> {
    epoch: Epoch,
    state: S,
}

/// Owned orbit components obtained by consuming an [`Orbit`].
///
/// This is the standard-conversion target for workflows that need to move the
/// selected state representation without cloning it.
#[derive(Debug, PartialEq)]
pub struct OrbitParts<S: SpacecraftState> {
    /// Epoch at which `state` is valid.
    pub epoch: Epoch,
    /// Native selected state representation.
    pub state: S,
}

impl<S: SpacecraftState> From<Orbit<S>> for OrbitParts<S> {
    fn from(orbit: Orbit<S>) -> Self {
        Self {
            epoch: orbit.epoch,
            state: orbit.state,
        }
    }
}

impl<S: SpacecraftState> AsRef<S> for Orbit<S> {
    fn as_ref(&self) -> &S {
        &self.state
    }
}

impl<S: SpacecraftState> Orbit<S> {
    /// Associates a state representation with its epoch.
    #[must_use]
    pub const fn new(epoch: Epoch, state: S) -> Self {
        Self { epoch, state }
    }

    /// Returns the epoch at which the state is valid.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Maps this orbit into another state representation while preserving its
    /// epoch.
    ///
    /// This operation only changes the stored state value. Any scientific
    /// conversion context, such as a gravity provider, belongs to the mapping
    /// closure so the core contract never selects data implicitly.
    #[must_use]
    pub fn map_state<T: SpacecraftState>(self, map: impl FnOnce(S) -> T) -> Orbit<T> {
        Orbit::new(self.epoch, map(self.state))
    }

    /// Fallibly maps this orbit into another state representation while
    /// preserving its epoch.
    ///
    /// The mapper's error is returned unchanged. This permits concrete state
    /// crates to retain their typed conversion and singularity errors without
    /// making this implementation-neutral crate depend on them.
    pub fn try_map_state<T: SpacecraftState, E>(
        self,
        map: impl FnOnce(S) -> Result<T, E>,
    ) -> Result<Orbit<T>, E> {
        Ok(Orbit::new(self.epoch, map(self.state)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestState(ReferenceFrame);

    impl SpacecraftState for TestState {
        fn frame(&self) -> ReferenceFrame {
            self.0
        }
    }

    #[test]
    fn state_mapping_preserves_epoch() {
        let epoch = Epoch::from_tai_seconds(42.0);
        let orbit = Orbit::new(epoch, TestState(ReferenceFrame::GCRF));

        let mapped = orbit.map_state(|state| TestState(state.frame()));

        assert_eq!(mapped.epoch(), epoch);
        assert_eq!(mapped.as_ref(), &TestState(ReferenceFrame::GCRF));
    }

    #[test]
    fn fallible_state_mapping_preserves_epoch() {
        let epoch = Epoch::from_tai_seconds(42.0);
        let orbit = Orbit::new(epoch, TestState(ReferenceFrame::GCRF));

        let mapped = orbit
            .try_map_state(|state| Ok::<_, &'static str>(TestState(state.frame())))
            .expect("conversion succeeds");

        assert_eq!(mapped.epoch(), epoch);
        assert_eq!(mapped.as_ref(), &TestState(ReferenceFrame::GCRF));
    }

    #[test]
    fn fallible_state_mapping_returns_the_original_error() {
        let orbit = Orbit::new(
            Epoch::from_tai_seconds(42.0),
            TestState(ReferenceFrame::GCRF),
        );

        let result = orbit.try_map_state(|_| Err::<TestState, _>("conversion failed"));

        assert_eq!(result, Err("conversion failed"));
    }
}
