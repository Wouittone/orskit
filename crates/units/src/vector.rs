use uom::si::{
    acceleration::meter_per_second_squared, angular_velocity::radian_per_second, length::meter,
    velocity::meter_per_second,
};

use crate::{Acceleration, AngularVelocity, Length, Velocity};

macro_rules! typed_cartesian_vector {
    (
        $(#[$meta:meta])*
        $name:ident,
        $quantity:ty,
        $from_si:ident,
        $to_si:ident,
        $unit:ty
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $name {
            x: $quantity,
            y: $quantity,
            z: $quantity,
        }

        impl $name {
            /// Constructs a Cartesian value from three typed components.
            #[must_use]
            pub const fn new(x: $quantity, y: $quantity, z: $quantity) -> Self {
                Self { x, y, z }
            }

            /// Returns the x component.
            #[must_use]
            pub const fn x(self) -> $quantity {
                self.x
            }

            /// Returns the y component.
            #[must_use]
            pub const fn y(self) -> $quantity {
                self.y
            }

            /// Returns the z component.
            #[must_use]
            pub const fn z(self) -> $quantity {
                self.z
            }

            /// Returns all three typed components in x/y/z order.
            #[must_use]
            pub const fn components(self) -> [$quantity; 3] {
                [self.x, self.y, self.z]
            }

            /// Returns whether every component has a finite scalar value.
            #[must_use]
            pub fn is_finite(self) -> bool {
                self.$to_si().into_iter().all(f64::is_finite)
            }

            /// Constructs a value at an explicit raw-SI interoperability boundary.
            #[must_use]
            pub fn $from_si(x: f64, y: f64, z: f64) -> Self {
                Self::new(
                    <$quantity>::new::<$unit>(x),
                    <$quantity>::new::<$unit>(y),
                    <$quantity>::new::<$unit>(z),
                )
            }

            /// Extracts raw SI components for numerical kernels and FFI only.
            #[must_use]
            pub fn $to_si(self) -> [f64; 3] {
                [
                    self.x.get::<$unit>(),
                    self.y.get::<$unit>(),
                    self.z.get::<$unit>(),
                ]
            }
        }

        impl std::ops::Add for $name {
            type Output = Self;

            fn add(self, right: Self) -> Self::Output {
                Self::new(self.x + right.x, self.y + right.y, self.z + right.z)
            }
        }

        impl std::ops::Sub for $name {
            type Output = Self;

            fn sub(self, right: Self) -> Self::Output {
                Self::new(self.x - right.x, self.y - right.y, self.z - right.z)
            }
        }
    };
}

typed_cartesian_vector!(
    /// Position vector. Every component is a [`Length`].
    Position,
    Length,
    from_metres,
    to_metres,
    meter
);

impl Position {
    /// Euclidean distance from the frame origin.
    #[must_use]
    pub fn norm(self) -> Length {
        let [x, y, z] = self.to_metres();
        Length::new::<meter>((x.mul_add(x, y.mul_add(y, z * z))).sqrt())
    }
}

typed_cartesian_vector!(
    /// Velocity vector. Every component is a [`Velocity`].
    VelocityVector,
    Velocity,
    from_metres_per_second,
    to_metres_per_second,
    meter_per_second
);

impl VelocityVector {
    /// Magnitude of the velocity vector (speed).
    #[must_use]
    pub fn speed(self) -> Velocity {
        let [x, y, z] = self.to_metres_per_second();
        Velocity::new::<meter_per_second>((x.mul_add(x, y.mul_add(y, z * z))).sqrt())
    }
}

typed_cartesian_vector!(
    /// Acceleration vector. Every component is an [`Acceleration`].
    AccelerationVector,
    Acceleration,
    from_metres_per_second_squared,
    to_metres_per_second_squared,
    meter_per_second_squared
);

impl AccelerationVector {
    /// Magnitude of the acceleration vector.
    #[must_use]
    pub fn magnitude(self) -> Acceleration {
        let [x, y, z] = self.to_metres_per_second_squared();
        Acceleration::new::<meter_per_second_squared>((x.mul_add(x, y.mul_add(y, z * z))).sqrt())
    }
}

typed_cartesian_vector!(
    /// Angular-velocity vector. Every component is an [`AngularVelocity`].
    AngularVelocityVector,
    AngularVelocity,
    from_radians_per_second,
    to_radians_per_second,
    radian_per_second
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_norm_preserves_length_dimension() {
        let position = Position::from_metres(3.0, 4.0, 0.0);
        assert_eq!(position.norm(), Length::new::<meter>(5.0));
    }

    #[test]
    fn velocity_magnitude_is_speed() {
        let velocity = VelocityVector::from_metres_per_second(0.0, 3.0, 4.0);
        assert_eq!(velocity.speed(), Velocity::new::<meter_per_second>(5.0));
    }

    #[test]
    fn position_vectors_support_standard_arithmetic() {
        let left = Position::from_metres(3.0, 4.0, 5.0);
        let right = Position::from_metres(1.0, 2.0, 3.0);
        assert_eq!(left + right, Position::from_metres(4.0, 6.0, 8.0));
        assert_eq!(left - right, Position::from_metres(2.0, 2.0, 2.0));
    }
}
