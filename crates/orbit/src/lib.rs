//! Orbital propagation and dynamics
//!
//! This crate provides orbital propagation capabilities using ODE solvers,
//! including gravity modeling and perturbation handling.

pub mod propagator;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
