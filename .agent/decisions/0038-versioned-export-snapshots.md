# ADR-0038: exchange versioned owned snapshots at an explicit boundary

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity rows: stable public Rust facade; orbit states; two-body propagation

## Context

Orbit element states and propagators retain application-owned
`Arc<dyn CentralGravityProvider>` values. A trait object has no general,
reconstructible serialized form, and its concrete implementation may contain
data or behavior that is neither stable nor serializable. Domain types also
use typed physical quantities while interchange formats ultimately contain raw
scalars. A durable boundary must preserve units, frames, epochs/time scales,
provider provenance, state representation, and schema evolution without
making a particular wire format part of the scientific core.

## Decision

1. Serde traits are not implemented directly on domain states, providers, or
   propagators. The opt-in `orskit-export` crate creates owned snapshot values
   containing only explicit interchange data.
2. Every physical raw scalar has a unit-qualified field name. Orbit snapshots
   retain Hifitime's epoch text including its time scale. Reference frames,
   gravity providers, and provider origins use caller-registered stable IDs;
   human-facing `Display` output is never a wire identity.
3. Snapshot families carry a stable schema name and integer schema version.
   Representation discriminators are explicit.
4. The caller registers each opaque central-gravity provider under a stable,
   non-blank ID. Matching uses shared-allocation identity, consistent with
   element-state provider compatibility; an unregistered or ambiguous mapping
   is a typed error. The snapshot also records the provider's origin and
   gravitational parameter.
5. `serialization` exposes the format-neutral context and extension contract.
   Orbit and two-body snapshots follow the facade's separately selected
   `cartesian` and `two-bodies` capabilities; serialization never enables a
   physical implementation implicitly. `serialization-json` additionally
   exposes JSON encoding. None is enabled by default.
6. Import accepts owned snapshots only through an explicit `ImportContext`.
   It validates schema names and versions, epoch syntax, representation
   discriminators, caller-approved frame/provider IDs, provider origin and
   parameter metadata, and then invokes the normal live domain constructors.
   Opaque providers are never reconstructed implicitly.

## Alternatives considered

- Derive Serde traits throughout domain crates: rejected because it couples
  scientific types to interchange policy and cannot reconstruct opaque
  provider implementations.
- Serialize only provider origin and parameter: rejected because numerically
  equal values do not establish the caller's selected dataset/model identity.
- Require one built-in serializable gravity enum: rejected because it closes
  an intentionally application-extensible provider contract.
- Make JSON the domain API: rejected because schema snapshots can support
  other Serde formats without a JSON dependency.

## Consequences

Snapshot exchange is explicit and may allocate owned strings. Applications must
manage stable frame, provider, and provider-origin IDs. Snapshot types are
suitable for interchange, logging, and application-controlled persistence but
are not live domain objects. An importing application remains responsible for
selecting and registering the exact live providers it trusts.

## Validation

Tests cover export and validated import for all built-in orbit representations,
exact provider identity, ambiguous registration failures, schema/epoch
rejection, version/identity/discriminator/domain-value rejection, provider
metadata mismatch, unit-qualified JSON round trips, and complete analytical
propagator settings.
Workspace formatting, Clippy, tests, doctests, and facade feature combinations
validate integration.

## Provenance

This is original orskit architecture. Serde 1.0.228 and serde_json 1.0.150 are
used as unmodified MIT OR Apache-2.0 dependencies. No external scientific
implementation, schema, source, or test vector informed the design.
