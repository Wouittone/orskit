# Implementing a custom propagation pair

`dynamics-core` exposes two related extension points:

- `Propagator<State>` owns a physical problem and the method used to advance
  one epoch-qualified `Orbit<State>` to an absolute target epoch.
- `PropagationState<Problem>` validates a caller-facing state against that
  problem, resolves it into the representation advanced by the method, and
  restores the caller-facing representation afterward.

The [compiled custom example](../../crates/dynamics/core/examples/custom_propagator.rs)
shows the complete wiring with constant linear motion. It is intentionally a
software example, not an orbital model:

```powershell
cargo run -p dynamics-core --example custom_propagator --locked
```

## Define the state and problem

The caller-facing type implements `orskit_core::SpacecraftState`, including an
explicit `ReferenceFrame`. The problem contains immutable model configuration;
in a real propagator that may include force models, selected data providers,
tolerances, or solver settings. The propagator owns the problem so a caller
cannot accidentally swap physics between calls.

Choose `PropagationState::Resolved` as the representation the algorithm truly
advances. It may be the same type as `State`, as for current Cartesian
two-body propagation, or an internal representation. `validate` should reject
incompatible frames, origins, data coverage, unsupported regimes, and
non-finite values before numerical work. `resolve` and `restore` return a typed
error and must not relabel coordinates or select hidden scientific data.

## Implement propagation

The implementation consumes the initial orbit, derives a duration from the
initial and absolute target epochs, resolves the state with its owned problem,
advances it, restores it, and constructs the result with the target epoch. It
must define behavior for backward propagation and a target equal to the initial
epoch. Do not silently assume UTC is a uniform integration coordinate.

The `Propagator` error type must implement `Error + Send + Sync + 'static`.
Report invalid input, unavailable external data, convergence, event, and
accuracy-budget failures explicitly. A propagator should not panic for a
recoverable physical or numerical input.

## Scientific implementation checklist

Before replacing the linear demonstration with a numerical or analytical
model, document and test:

- state ordering, units, frames, origins, epochs, and time scale;
- equations, conventions, external data, valid regime, and singularities;
- absolute/relative tolerances, iteration limits, dense-output and event
  semantics where applicable;
- nominal, backward, zero-duration, boundary, and failure cases;
- invariants and independently sourced reference vectors with physically
  justified tolerances; and
- whether attitude, mass, variational state, or other coupled properties are
  actually advanced.

Update provenance, the relevant parity row, and an ADR when the model or public
contract warrants it. Implementations belong in a focused dynamics crate; the
`dynamics` facade should only expose them behind a purposeful feature.
