use std::fmt;

use frames::ReferenceFrame;
use hifitime::Epoch;

/// Open contract for one frame-qualified spacecraft-state representation.
///
/// State implementations live in dedicated crates or in applications. The
/// contract deliberately says nothing about coordinates, force models, or
/// conversion algorithms: those are selected through composition.
pub trait SpacecraftState: fmt::Debug + Clone + Send + Sync {
    /// Returns the frame in which this state representation is expressed.
    fn frame(&self) -> ReferenceFrame;
}

/// A spacecraft state qualified by the epoch at which it is valid.
///
/// `Orbit` owns no coordinate implementation. Its state type is selected by
/// the caller, preserving the native representation through generic workflows.
#[derive(Debug, Clone, PartialEq)]
pub struct Orbit<S: SpacecraftState> {
    epoch: Epoch,
    state: S,
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

    /// Returns the native state representation.
    #[must_use]
    pub fn state(&self) -> S {
        self.state.clone()
    }

    /// Consumes this orbit and returns the native state representation.
    #[must_use]
    pub fn into_state(self) -> S {
        self.state
    }
}
