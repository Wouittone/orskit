//! Typed inverse-time quantities used by state sensitivities.

macro_rules! inverse_time_quantity {
    (
        $(#[$meta:meta])*
        $name:ident,
        $from_si:ident,
        $to_si:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
        pub struct $name(f64);

        impl $name {
            /// Creates a value at the explicit SI interoperability boundary.
            #[must_use]
            pub const fn $from_si(value: f64) -> Self {
                Self(value)
            }

            /// Returns the value in its explicit SI unit.
            #[must_use]
            pub const fn $to_si(self) -> f64 {
                self.0
            }
        }
    };
}

inverse_time_quantity!(
    /// Inverse-time sensitivity entry in reciprocal seconds (`s^-1`).
    InverseTime,
    from_per_second,
    as_per_second
);

inverse_time_quantity!(
    /// Inverse-square-time sensitivity entry in reciprocal square seconds (`s^-2`).
    InverseTimeSquared,
    from_per_square_second,
    as_per_square_second
);
