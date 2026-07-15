//! Unit-qualified covariance-entry quantities not provided by the SI catalog.

macro_rules! covariance_quantity {
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

        impl std::ops::Add for $name {
            type Output = Self;

            fn add(self, other: Self) -> Self {
                Self(self.0 + other.0)
            }
        }
    };
}

covariance_quantity!(
    /// Velocity covariance entry in square metres per square second (`m^2/s^2`).
    VelocityVariance,
    from_square_metres_per_square_second,
    as_square_metres_per_square_second
);

covariance_quantity!(
    /// Angular covariance entry in square radians (`rad^2`).
    AngularVariance,
    from_square_radians,
    as_square_radians
);

covariance_quantity!(
    /// Frequency covariance entry in square hertz (`Hz^2`).
    FrequencyVariance,
    from_square_hertz,
    as_square_hertz
);

covariance_quantity!(
    /// Time covariance entry in square seconds (`s^2`).
    TimeVariance,
    from_square_seconds,
    as_square_seconds
);

covariance_quantity!(
    /// Position/velocity covariance entry in square metres per second (`m^2/s`).
    PositionVelocityCovariance,
    from_square_metres_per_second,
    as_square_metres_per_second
);
