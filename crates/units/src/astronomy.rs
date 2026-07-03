use thiserror::Error;

/// Error returned when constructing a custom physical quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum QuantityError {
    /// The supplied SI value was NaN or infinite.
    #[error("{quantity} must be finite")]
    NonFinite {
        /// Name of the rejected quantity.
        quantity: &'static str,
    },
    /// The supplied SI value was not strictly positive.
    #[error("{quantity} must be strictly positive")]
    NotPositive {
        /// Name of the rejected quantity.
        quantity: &'static str,
    },
}

/// Standard gravitational parameter, with dimension `L^3 T^-2`.
///
/// The canonical stored unit is cubic metres per square second (`m^3/s^2`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GravitationalParameter(f64);

impl GravitationalParameter {
    /// Constructs a positive finite gravitational parameter from `m^3/s^2`.
    pub fn from_cubic_metres_per_second_squared(value: f64) -> Result<Self, QuantityError> {
        validate_positive_finite(value, "gravitational parameter")?;
        Ok(Self(value))
    }

    /// Returns this parameter in `m^3/s^2`.
    #[must_use]
    pub const fn as_cubic_metres_per_second_squared(self) -> f64 {
        self.0
    }
}

/// Newtonian gravitational constant, with dimension `L^3 M^-1 T^-2`.
///
/// The canonical stored unit is cubic metres per kilogram per square second
/// (`m^3/(kg*s^2)`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GravitationalConstant(f64);

impl GravitationalConstant {
    /// Constructs a positive finite constant from `m^3/(kg*s^2)`.
    pub fn from_cubic_metres_per_kilogram_second_squared(
        value: f64,
    ) -> Result<Self, QuantityError> {
        validate_positive_finite(value, "gravitational constant")?;
        Ok(Self(value))
    }

    /// Returns this constant in `m^3/(kg*s^2)`.
    #[must_use]
    pub const fn as_cubic_metres_per_kilogram_second_squared(self) -> f64 {
        self.0
    }
}

fn validate_positive_finite(value: f64, quantity: &'static str) -> Result<(), QuantityError> {
    if !value.is_finite() {
        return Err(QuantityError::NonFinite { quantity });
    }
    if value <= 0.0 {
        return Err(QuantityError::NotPositive { quantity });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_quantities_reject_non_physical_values() {
        assert!(matches!(
            GravitationalParameter::from_cubic_metres_per_second_squared(f64::NAN),
            Err(QuantityError::NonFinite { .. })
        ));
        assert!(matches!(
            GravitationalConstant::from_cubic_metres_per_kilogram_second_squared(0.0),
            Err(QuantityError::NotPositive { .. })
        ));
    }
}
