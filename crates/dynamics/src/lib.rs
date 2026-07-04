//! Composable descriptions of dynamical systems and their force models.
//!
//! This crate describes model topology only. It intentionally does not define
//! state derivatives, numerical integration, propagation, events, or
//! variational equations yet. Those evaluation contracts can consume a
//! [`SystemDynamics`] description without making a simplified two-body model
//! the organizing abstraction.
//!
//! ```
//! use orskit_bodies::Body;
//! use orskit_dynamics::{SystemDynamics, ThreeBodyDynamics};
//!
//! let model = ThreeBodyDynamics::new(Body::SUN, Body::EARTH, Body::MOON)?;
//! assert_eq!(model.participants().len(), 3);
//! assert_eq!(model.force_models().len(), 1);
//! # Ok::<(), orskit_dynamics::DynamicsDescriptionError>(())
//! ```

use std::{fmt, sync::Arc};

use orskit_bodies::Body;
use thiserror::Error;

/// Source and target roles declared by one force model.
///
/// Sources generate or parameterize the interaction; targets are the
/// participants whose dynamics the model affects. A mutual interaction may
/// list the same participants in both roles. A source-free model such as a
/// prescribed maneuver may use an empty source slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForceInteraction<'a, Participant> {
    sources: &'a [Participant],
    targets: &'a [Participant],
}

impl<'a, Participant> ForceInteraction<'a, Participant> {
    /// Describes the source and target roles of a force model.
    #[must_use]
    pub const fn new(sources: &'a [Participant], targets: &'a [Participant]) -> Self {
        Self { sources, targets }
    }

    /// Returns the participants generating or parameterizing the interaction.
    #[must_use]
    pub const fn sources(self) -> &'a [Participant] {
        self.sources
    }

    /// Returns the participants affected by the interaction.
    #[must_use]
    pub const fn targets(self) -> &'a [Participant] {
        self.targets
    }
}

/// Pluggable description of one force contribution.
///
/// This trait deliberately has no evaluation method yet. A future evaluator
/// will define state, epoch, frame, model-data, derivative, and error contracts
/// explicitly instead of smuggling them into a descriptive API.
pub trait ForceModel: fmt::Debug + Send + Sync {
    /// Participant identity used by this model.
    type Participant: fmt::Debug + Send + Sync;

    /// Returns a stable human-readable model name for diagnostics.
    fn name(&self) -> &str;

    /// Returns the declared source and target roles.
    fn interaction(&self) -> ForceInteraction<'_, Self::Participant>;
}

/// Shared handle used to plug a force model into a dynamics description.
pub type ForceModelHandle<Participant> =
    Arc<dyn ForceModel<Participant = Participant> + Send + Sync + 'static>;

/// Description of a dynamical system and its composed force models.
///
/// The associated participant type allows future coupled spacecraft, rigid
/// bodies, or estimation states without forcing every dynamics model to use a
/// celestial-body-only enum. Implementations preserve force-model order so a
/// future evaluator can document deterministic accumulation policy.
pub trait SystemDynamics: fmt::Debug + Send + Sync {
    /// Participant identity used throughout this system.
    type Participant: fmt::Debug + Send + Sync;

    /// Returns a stable human-readable system name for diagnostics.
    fn name(&self) -> &str;

    /// Returns all participants whose dynamics belong to this system.
    fn participants(&self) -> &[Self::Participant];

    /// Returns the force models in declared composition order.
    fn force_models(&self) -> &[ForceModelHandle<Self::Participant>];
}

/// Mutual point-mass gravity topology for a set of celestial bodies.
///
/// This type declares which bodies interact; it does not select gravitational
/// parameters, ephemerides, frames, or an evaluation algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutualPointMassGravity {
    bodies: Box<[Body]>,
}

impl MutualPointMassGravity {
    /// Describes mutual point-mass gravity between at least two distinct bodies.
    pub fn new(bodies: impl Into<Box<[Body]>>) -> Result<Self, DynamicsDescriptionError> {
        let bodies = bodies.into();
        validate_distinct_bodies(&bodies, 2)?;
        Ok(Self { bodies })
    }

    /// Returns the interacting bodies.
    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }
}

impl ForceModel for MutualPointMassGravity {
    type Participant = Body;

    fn name(&self) -> &str {
        "mutual point-mass gravity"
    }

    fn interaction(&self) -> ForceInteraction<'_, Self::Participant> {
        ForceInteraction::new(&self.bodies, &self.bodies)
    }
}

/// Simplified two-body dynamics description.
///
/// Mutual point-mass gravity is always the first force model. Additional force
/// descriptions may be attached without changing the system contract.
#[derive(Debug, Clone)]
pub struct TwoBodyDynamics {
    participants: [Body; 2],
    force_models: Vec<ForceModelHandle<Body>>,
}

impl TwoBodyDynamics {
    /// Describes two distinct bodies with mutual point-mass gravity.
    pub fn new(primary: Body, secondary: Body) -> Result<Self, DynamicsDescriptionError> {
        let participants = [primary, secondary];
        let gravity = MutualPointMassGravity::new(participants)?;
        Ok(Self {
            participants,
            force_models: vec![Arc::new(gravity)],
        })
    }

    /// Adds a force description whose participants all belong to this system.
    pub fn with_force_model(
        mut self,
        force_model: ForceModelHandle<Body>,
    ) -> Result<Self, DynamicsDescriptionError> {
        validate_force_membership(&self.participants, force_model.as_ref())?;
        self.force_models.push(force_model);
        Ok(self)
    }
}

impl SystemDynamics for TwoBodyDynamics {
    type Participant = Body;

    fn name(&self) -> &str {
        "two-body dynamics"
    }

    fn participants(&self) -> &[Self::Participant] {
        &self.participants
    }

    fn force_models(&self) -> &[ForceModelHandle<Self::Participant>] {
        &self.force_models
    }
}

/// Simplified three-body dynamics description.
///
/// Mutual point-mass gravity is always the first force model. The description
/// does not choose restricted/full equations or a numerical resolution method.
#[derive(Debug, Clone)]
pub struct ThreeBodyDynamics {
    participants: [Body; 3],
    force_models: Vec<ForceModelHandle<Body>>,
}

impl ThreeBodyDynamics {
    /// Describes three distinct bodies with mutual point-mass gravity.
    pub fn new(first: Body, second: Body, third: Body) -> Result<Self, DynamicsDescriptionError> {
        let participants = [first, second, third];
        let gravity = MutualPointMassGravity::new(participants)?;
        Ok(Self {
            participants,
            force_models: vec![Arc::new(gravity)],
        })
    }

    /// Adds a force description whose participants all belong to this system.
    pub fn with_force_model(
        mut self,
        force_model: ForceModelHandle<Body>,
    ) -> Result<Self, DynamicsDescriptionError> {
        validate_force_membership(&self.participants, force_model.as_ref())?;
        self.force_models.push(force_model);
        Ok(self)
    }
}

impl SystemDynamics for ThreeBodyDynamics {
    type Participant = Body;

    fn name(&self) -> &str {
        "three-body dynamics"
    }

    fn participants(&self) -> &[Self::Participant] {
        &self.participants
    }

    fn force_models(&self) -> &[ForceModelHandle<Self::Participant>] {
        &self.force_models
    }
}

fn validate_distinct_bodies(
    bodies: &[Body],
    minimum: usize,
) -> Result<(), DynamicsDescriptionError> {
    if bodies.len() < minimum {
        return Err(DynamicsDescriptionError::TooFewBodies { minimum });
    }
    for (index, body) in bodies.iter().enumerate() {
        if bodies[index + 1..].contains(body) {
            return Err(DynamicsDescriptionError::DuplicateBody(*body));
        }
    }
    Ok(())
}

fn validate_force_membership(
    participants: &[Body],
    force_model: &dyn ForceModel<Participant = Body>,
) -> Result<(), DynamicsDescriptionError> {
    let interaction = force_model.interaction();
    for body in interaction.sources().iter().chain(interaction.targets()) {
        if !participants.contains(body) {
            return Err(DynamicsDescriptionError::ExternalBody {
                model: force_model.name().to_owned(),
                body: *body,
            });
        }
    }
    Ok(())
}

/// Invalid dynamics or force-model description.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DynamicsDescriptionError {
    /// A simplified gravitational model has too few participants.
    #[error("dynamics description requires at least {minimum} bodies")]
    TooFewBodies {
        /// Minimum body count required by the model.
        minimum: usize,
    },
    /// A body occurs more than once in a system.
    #[error("dynamics description contains duplicate body {0}")]
    DuplicateBody(Body),
    /// A plugged-in force references a body outside the system.
    #[error("force model {model:?} references external body {body}")]
    ExternalBody {
        /// Force-model diagnostic name.
        model: String,
        /// Body absent from the system participant list.
        body: Body,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DirectedForce {
        name: &'static str,
        sources: Box<[Body]>,
        targets: Box<[Body]>,
    }

    impl ForceModel for DirectedForce {
        type Participant = Body;

        fn name(&self) -> &str {
            self.name
        }

        fn interaction(&self) -> ForceInteraction<'_, Body> {
            ForceInteraction::new(&self.sources, &self.targets)
        }
    }

    #[test]
    fn two_body_is_a_system_dynamics_implementation() {
        let dynamics = TwoBodyDynamics::new(Body::EARTH, Body::MOON)
            .expect("Earth and Moon are distinct bodies");

        assert_eq!(dynamics.name(), "two-body dynamics");
        assert_eq!(dynamics.participants(), &[Body::EARTH, Body::MOON]);
        assert_eq!(dynamics.force_models().len(), 1);
        assert_eq!(
            dynamics.force_models()[0].interaction(),
            ForceInteraction::new(&[Body::EARTH, Body::MOON], &[Body::EARTH, Body::MOON])
        );
    }

    #[test]
    fn three_body_is_not_reduced_to_a_two_body_core() {
        let dynamics = ThreeBodyDynamics::new(Body::SUN, Body::EARTH, Body::MOON)
            .expect("fixture bodies are distinct");

        assert_eq!(dynamics.participants().len(), 3);
        assert_eq!(
            dynamics.force_models()[0].interaction().sources(),
            &[Body::SUN, Body::EARTH, Body::MOON]
        );
    }

    #[test]
    fn custom_force_models_are_composed_in_declaration_order() {
        let extra_force: ForceModelHandle<Body> = Arc::new(DirectedForce {
            name: "Earth radiation pressure",
            sources: vec![Body::EARTH].into_boxed_slice(),
            targets: vec![Body::MOON].into_boxed_slice(),
        });
        let dynamics = TwoBodyDynamics::new(Body::EARTH, Body::MOON)
            .expect("fixture bodies are distinct")
            .with_force_model(extra_force)
            .expect("force participants belong to the system");

        assert_eq!(dynamics.force_models().len(), 2);
        assert_eq!(
            dynamics.force_models()[1].name(),
            "Earth radiation pressure"
        );
    }

    #[test]
    fn duplicate_and_external_bodies_are_rejected() {
        assert!(matches!(
            TwoBodyDynamics::new(Body::EARTH, Body::EARTH),
            Err(DynamicsDescriptionError::DuplicateBody(Body::EARTH))
        ));

        let external_force: ForceModelHandle<Body> = Arc::new(DirectedForce {
            name: "external source",
            sources: vec![Body::SUN].into_boxed_slice(),
            targets: vec![Body::MOON].into_boxed_slice(),
        });
        let result = TwoBodyDynamics::new(Body::EARTH, Body::MOON)
            .expect("fixture bodies are distinct")
            .with_force_model(external_force);

        assert_eq!(
            result.expect_err("Sun is outside the described system"),
            DynamicsDescriptionError::ExternalBody {
                model: "external source".to_owned(),
                body: Body::SUN,
            }
        );
    }
}
