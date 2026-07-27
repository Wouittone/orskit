# Current Rust crate architecture

This diagram describes the checked-in Cargo workspace, not the complete target
architecture. It is generated from normal path dependencies returned by
`cargo metadata --format-version 1 --no-deps --locked`; development-only
dependencies are omitted and optional dependencies use dashed arrows.

```mermaid
flowchart TB
    subgraph LAYER_PUBLIC_FACADE["Public facade"]
        ORSKIT["orskit"]
    end
    subgraph LAYER_WORKFLOWS_AND_I_O["Workflows and I/O"]
        CCSDS["ccsds"]
        TLE["tle"]
        ORSKIT_EXPORT["orskit-export"]
        ORBIT_DETERMINATION["orbit-determination"]
        MEASUREMENTS["measurements"]
    end
    subgraph LAYER_DYNAMICS["Dynamics"]
        DYNAMICS["dynamics"]
        DYNAMICS_CORE["dynamics-core"]
        DYNAMICS_NUMERICAL["dynamics-numerical"]
        DYNAMICS_TWO_BODIES["dynamics-two-bodies"]
    end
    subgraph LAYER_PHYSICAL_MODEL["Physical model"]
        CORE["core (lib: orskit_core)"]
        ORBITS["orbits"]
        GRAVITY["gravity"]
        EPHEMERIS["ephemeris"]
        FRAMES["frames"]
        BODIES["bodies"]
    end
    subgraph LAYER_FOUNDATIONS["Foundations"]
        UTILS["utils"]
        UNITS["units"]
        ORSKIT_DATA["orskit-data"]
    end
    BODIES --> UNITS
    CCSDS --> CORE
    CCSDS --> FRAMES
    CCSDS --> ORBITS
    CCSDS --> UNITS
    CORE --> FRAMES
    CORE --> UNITS
    DYNAMICS -.->|optional| CORE
    DYNAMICS --> DYNAMICS_CORE
    DYNAMICS -.->|optional| DYNAMICS_NUMERICAL
    DYNAMICS -.->|optional| DYNAMICS_TWO_BODIES
    DYNAMICS -.->|optional| FRAMES
    DYNAMICS -.->|optional| ORBITS
    DYNAMICS -.->|optional| UNITS
    DYNAMICS_CORE --> BODIES
    DYNAMICS_CORE --> CORE
    DYNAMICS_CORE --> UNITS
    DYNAMICS_NUMERICAL --> CORE
    DYNAMICS_NUMERICAL --> DYNAMICS_CORE
    DYNAMICS_NUMERICAL --> FRAMES
    DYNAMICS_NUMERICAL --> ORBITS
    DYNAMICS_NUMERICAL --> UNITS
    DYNAMICS_TWO_BODIES --> CORE
    DYNAMICS_TWO_BODIES --> DYNAMICS_CORE
    DYNAMICS_TWO_BODIES --> FRAMES
    DYNAMICS_TWO_BODIES --> GRAVITY
    DYNAMICS_TWO_BODIES --> ORBITS
    DYNAMICS_TWO_BODIES --> UNITS
    EPHEMERIS --> BODIES
    EPHEMERIS --> FRAMES
    EPHEMERIS --> ORSKIT_DATA
    EPHEMERIS --> UNITS
    FRAMES --> BODIES
    FRAMES --> ORSKIT_DATA
    FRAMES --> UNITS
    GRAVITY --> FRAMES
    GRAVITY --> UNITS
    MEASUREMENTS --> FRAMES
    MEASUREMENTS --> UNITS
    MEASUREMENTS -.->|optional| UTILS
    ORBIT_DETERMINATION --> CORE
    ORBIT_DETERMINATION --> DYNAMICS
    ORBIT_DETERMINATION --> FRAMES
    ORBIT_DETERMINATION --> ORBITS
    ORBIT_DETERMINATION --> UNITS
    ORBITS --> CORE
    ORBITS --> FRAMES
    ORBITS --> GRAVITY
    ORBITS --> UNITS
    ORSKIT -.->|optional| BODIES
    ORSKIT -.->|optional| CCSDS
    ORSKIT --> CORE
    ORSKIT -.->|optional| DYNAMICS
    ORSKIT -.->|optional| EPHEMERIS
    ORSKIT --> FRAMES
    ORSKIT -.->|optional| GRAVITY
    ORSKIT -.->|optional| MEASUREMENTS
    ORSKIT -.->|optional| ORBIT_DETERMINATION
    ORSKIT -.->|optional| ORBITS
    ORSKIT --> ORSKIT_DATA
    ORSKIT -.->|optional| ORSKIT_EXPORT
    ORSKIT -.->|optional| TLE
    ORSKIT --> UNITS
    ORSKIT_EXPORT --> CORE
    ORSKIT_EXPORT -.->|optional| DYNAMICS_TWO_BODIES
    ORSKIT_EXPORT --> FRAMES
    ORSKIT_EXPORT --> GRAVITY
    ORSKIT_EXPORT -.->|optional| ORBITS
    ORSKIT_EXPORT -.->|optional| UNITS
    TLE -.->|optional| CORE
    TLE -.->|optional| DYNAMICS
    TLE -.->|optional| UNITS
    UTILS --> UNITS
```

Dependencies point from a consumer to the crate it uses. The layer groupings
are maintained in `scripts/check_crate_diagram.ps1`; the script fails when a
workspace package has not been assigned to a layer. Run `just diagram` after
a manifest change and `just diagram-check` in review. Both commands use the
locked dependency graph.

The graph is descriptive. The normative dependency direction and the meaning
of each domain boundary remain in [the target architecture](../.agent/ARCHITECTURE.md).
In particular, the feature-gated `dynamics` and `orskit` facades point inward
to implementations because they curate exports; this does not permit a
physical-model crate to depend upward on either facade.
