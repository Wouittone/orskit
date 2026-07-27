#![forbid(unsafe_code)]

//! Strict parsing and canonical formatting of NORAD Two-Line Element sets.
//!
//! A [`TwoLineElement`] accepts exactly two 69-character ASCII lines, validates
//! their fixed columns and standard modulo-10 checksums, and requires the
//! catalog numbers on both lines to agree. Numeric accessors name the units
//! defined by the TLE format. The opt-in `sgp4` feature converts a parsed
//! record into the separate model-specific state consumed by
//! `dynamics-sgp4`; propagation does not depend on this format type.
//!
//! The grammar and ranges follow the public CelesTrak TLE format description
//! and Space-Track's current Alpha-5 documentation. Epoch years use the
//! conventional pivot: `57..=99` mean 1957–1999 and `00..=56` mean 2000–2056.
//! Epoch days start at one and remain within the selected calendar year.
//!
//! # Example
//!
//! ```
//! use tle::TwoLineElement;
//!
//! let tle = TwoLineElement::parse(
//!     "1 23455U 94089A   97320.90946019  .00000140  00000-0  10191-3 0  2621",
//!     "2 23455  99.0090 272.6745 0008546 223.1686 136.8816 14.11711747148495",
//! )?;
//!
//! assert_eq!(tle.satellite_catalog_number(), 23_455);
//! assert_eq!(tle.epoch_year(), 1997);
//! assert_eq!(tle.inclination_deg(), 99.0090);
//! assert_eq!(tle.to_string().lines().count(), 2);
//! # Ok::<(), tle::TleError>(())
//! ```

use std::{fmt, str::FromStr};

use thiserror::Error;

#[cfg(feature = "sgp4")]
mod sgp4;

#[cfg(feature = "sgp4")]
pub use sgp4::Sgp4ConversionError;

const LINE_LENGTH: usize = 69;
const SCALE_4: u64 = 10_000;
const SCALE_7: u64 = 10_000_000;
const SCALE_8: u64 = 100_000_000;

/// One of the two fixed-format TLE lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TleLine {
    /// Element identification, epoch, derivatives, drag, and set number.
    One,
    /// Mean orbital elements and revolution number.
    Two,
}

impl fmt::Display for TleLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::One => "line 1",
            Self::Two => "line 2",
        })
    }
}

/// Named fixed-width field within a TLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TleField {
    /// Line-number marker.
    LineNumber,
    /// Fixed separator column.
    Separator,
    /// Satellite catalog number.
    SatelliteCatalogNumber,
    /// Element-set classification.
    Classification,
    /// International designator.
    InternationalDesignator,
    /// Two-digit epoch year.
    EpochYear,
    /// Day of year and fraction.
    EpochDay,
    /// First derivative of mean motion divided by two.
    MeanMotionFirstDerivative,
    /// Second derivative of mean motion divided by six.
    MeanMotionSecondDerivative,
    /// SGP4 B* drag term.
    BStar,
    /// Ephemeris type.
    EphemerisType,
    /// Element-set number.
    ElementSetNumber,
    /// Inclination.
    Inclination,
    /// Right ascension of the ascending node.
    RightAscensionOfAscendingNode,
    /// Eccentricity.
    Eccentricity,
    /// Argument of perigee.
    ArgumentOfPerigee,
    /// Mean anomaly.
    MeanAnomaly,
    /// Mean motion.
    MeanMotion,
    /// Revolution number at epoch.
    RevolutionNumber,
    /// Checksum.
    Checksum,
}

impl fmt::Display for TleField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LineNumber => "line number",
            Self::Separator => "fixed separator",
            Self::SatelliteCatalogNumber => "satellite catalog number",
            Self::Classification => "classification",
            Self::InternationalDesignator => "international designator",
            Self::EpochYear => "epoch year",
            Self::EpochDay => "epoch day",
            Self::MeanMotionFirstDerivative => "mean-motion first derivative",
            Self::MeanMotionSecondDerivative => "mean-motion second derivative",
            Self::BStar => "B* drag term",
            Self::EphemerisType => "ephemeris type",
            Self::ElementSetNumber => "element-set number",
            Self::Inclination => "inclination",
            Self::RightAscensionOfAscendingNode => "right ascension of the ascending node",
            Self::Eccentricity => "eccentricity",
            Self::ArgumentOfPerigee => "argument of perigee",
            Self::MeanAnomaly => "mean anomaly",
            Self::MeanMotion => "mean motion",
            Self::RevolutionNumber => "revolution number",
            Self::Checksum => "checksum",
        })
    }
}

/// Failure while parsing an untrusted TLE.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TleError {
    /// A line was not exactly 69 bytes.
    #[error("{line} must contain exactly 69 ASCII bytes, found {actual}")]
    InvalidLineLength {
        /// Affected source line.
        line: TleLine,
        /// Observed byte length.
        actual: usize,
    },
    /// A non-ASCII byte occurred.
    #[error("{line} column {column} is not ASCII")]
    NonAscii {
        /// Affected source line.
        line: TleLine,
        /// One-based source column.
        column: usize,
    },
    /// A fixed or field-specific column had an invalid character.
    #[error("{line} column {column} is invalid for {field}")]
    InvalidCharacter {
        /// Affected source line.
        line: TleLine,
        /// Affected field.
        field: TleField,
        /// One-based source column.
        column: usize,
    },
    /// A syntactically valid field value was outside its defined range.
    #[error("{line} field {field} is outside the TLE range")]
    ValueOutOfRange {
        /// Affected source line.
        line: TleLine,
        /// Affected field.
        field: TleField,
    },
    /// A line checksum did not match column 69.
    #[error("{line} checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch {
        /// Affected source line.
        line: TleLine,
        /// Checksum calculated over columns 1–68.
        expected: u8,
        /// Digit stored in column 69.
        found: u8,
    },
    /// The two lines identified different catalog objects.
    #[error("catalog number mismatch: line 1 identifies {line_one}, line 2 identifies {line_two}")]
    CatalogNumberMismatch {
        /// Catalog number decoded from line 1.
        line_one: u32,
        /// Catalog number decoded from line 2.
        line_two: u32,
    },
    /// Combined text did not contain exactly two TLE lines.
    #[error("a TLE must contain exactly two data lines")]
    InvalidLineCount,
}

/// Validated mean elements represented by one NORAD Two-Line Element set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwoLineElement {
    satellite_catalog_number: u32,
    classification: char,
    international_designator: Option<String>,
    epoch_year_two_digits: u8,
    epoch_day_scaled: u64,
    mean_motion_first_derivative_scaled: i64,
    mean_motion_second_derivative: ImpliedExponent,
    b_star: ImpliedExponent,
    ephemeris_type: u8,
    element_set_number: u16,
    inclination_scaled: u64,
    right_ascension_scaled: u64,
    eccentricity_scaled: u64,
    argument_of_perigee_scaled: u64,
    mean_anomaly_scaled: u64,
    mean_motion_scaled: u64,
    revolution_number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImpliedExponent {
    mantissa: i32,
    exponent: i8,
}

impl TwoLineElement {
    /// Parses and validates two fixed-width TLE lines.
    ///
    /// Each input must contain exactly 69 ASCII bytes and no line terminator.
    /// Checksums, fixed columns, field syntax, field ranges, and matching
    /// catalog numbers are validated before a value is returned.
    pub fn parse(line_one: &str, line_two: &str) -> Result<Self, TleError> {
        let one = validate_line(line_one, TleLine::One)?;
        let two = validate_line(line_two, TleLine::Two)?;
        validate_checksum(one, TleLine::One)?;
        validate_checksum(two, TleLine::Two)?;
        validate_fixed_columns(one, TleLine::One)?;
        validate_fixed_columns(two, TleLine::Two)?;

        let satellite_catalog_number = parse_catalog_number(&one[2..7], TleLine::One, 3)?;
        let line_two_catalog = parse_catalog_number(&two[2..7], TleLine::Two, 3)?;
        if satellite_catalog_number != line_two_catalog {
            return Err(TleError::CatalogNumberMismatch {
                line_one: satellite_catalog_number,
                line_two: line_two_catalog,
            });
        }

        let classification = one[7];
        if !matches!(classification, b'U' | b'S') {
            return Err(invalid_character(TleLine::One, TleField::Classification, 8));
        }
        let international_designator = parse_international_designator(&one[9..17])?;
        let epoch_year_two_digits =
            parse_digits(&one[18..20], TleLine::One, TleField::EpochYear, 19)? as u8;
        let epoch_day_scaled =
            parse_fixed_decimal(&one[20..32], 3, 8, TleLine::One, TleField::EpochDay, 21)?;
        let days_in_epoch_year = u64::from(days_in_year(expand_year(epoch_year_two_digits)));
        if epoch_day_scaled < SCALE_8 || epoch_day_scaled >= (days_in_epoch_year + 1) * SCALE_8 {
            return Err(out_of_range(TleLine::One, TleField::EpochDay));
        }

        let mean_motion_first_derivative_scaled = parse_signed_fraction(
            &one[33..43],
            TleLine::One,
            TleField::MeanMotionFirstDerivative,
            34,
        )?;
        let mean_motion_second_derivative = parse_implied_exponent(
            &one[44..52],
            TleLine::One,
            TleField::MeanMotionSecondDerivative,
            45,
        )?;
        let b_star = parse_implied_exponent(&one[53..61], TleLine::One, TleField::BStar, 54)?;
        let ephemeris_type =
            parse_digits(&one[62..63], TleLine::One, TleField::EphemerisType, 63)? as u8;
        let element_set_number =
            parse_space_padded_integer(&one[64..68], TleLine::One, TleField::ElementSetNumber, 65)?
                as u16;

        let inclination_scaled =
            parse_fixed_decimal(&two[8..16], 3, 4, TleLine::Two, TleField::Inclination, 9)?;
        if inclination_scaled > 180 * SCALE_4 {
            return Err(out_of_range(TleLine::Two, TleField::Inclination));
        }
        let right_ascension_scaled = parse_fixed_decimal(
            &two[17..25],
            3,
            4,
            TleLine::Two,
            TleField::RightAscensionOfAscendingNode,
            18,
        )?;
        validate_full_angle(
            right_ascension_scaled,
            TleField::RightAscensionOfAscendingNode,
        )?;
        let eccentricity_scaled =
            parse_digits(&two[26..33], TleLine::Two, TleField::Eccentricity, 27)?;
        let argument_of_perigee_scaled = parse_fixed_decimal(
            &two[34..42],
            3,
            4,
            TleLine::Two,
            TleField::ArgumentOfPerigee,
            35,
        )?;
        validate_full_angle(argument_of_perigee_scaled, TleField::ArgumentOfPerigee)?;
        let mean_anomaly_scaled =
            parse_fixed_decimal(&two[43..51], 3, 4, TleLine::Two, TleField::MeanAnomaly, 44)?;
        validate_full_angle(mean_anomaly_scaled, TleField::MeanAnomaly)?;
        let mean_motion_scaled =
            parse_fixed_decimal(&two[52..63], 2, 8, TleLine::Two, TleField::MeanMotion, 53)?;
        if mean_motion_scaled == 0 {
            return Err(out_of_range(TleLine::Two, TleField::MeanMotion));
        }
        let revolution_number =
            parse_space_padded_integer(&two[63..68], TleLine::Two, TleField::RevolutionNumber, 64)?
                as u32;

        Ok(Self {
            satellite_catalog_number,
            classification: char::from(classification),
            international_designator,
            epoch_year_two_digits,
            epoch_day_scaled,
            mean_motion_first_derivative_scaled,
            mean_motion_second_derivative,
            b_star,
            ephemeris_type,
            element_set_number,
            inclination_scaled,
            right_ascension_scaled,
            eccentricity_scaled,
            argument_of_perigee_scaled,
            mean_anomaly_scaled,
            mean_motion_scaled,
            revolution_number,
        })
    }

    /// Returns the decoded numeric satellite catalog number.
    #[must_use]
    pub const fn satellite_catalog_number(&self) -> u32 {
        self.satellite_catalog_number
    }

    /// Returns the element-set classification character.
    #[must_use]
    pub const fn classification(&self) -> char {
        self.classification
    }

    /// Returns the trimmed international designator, when present.
    #[must_use]
    pub fn international_designator(&self) -> Option<&str> {
        self.international_designator.as_deref()
    }

    /// Returns the expanded UTC epoch year using the conventional 1957 pivot.
    #[must_use]
    pub const fn epoch_year(&self) -> u16 {
        expand_year(self.epoch_year_two_digits)
    }

    /// Returns the UTC epoch day of year, including its fractional day.
    #[must_use]
    pub fn epoch_day_utc(&self) -> f64 {
        self.epoch_day_scaled as f64 / SCALE_8 as f64
    }

    /// Returns the first mean-motion derivative divided by two, in rev/day².
    #[must_use]
    pub fn mean_motion_first_derivative_rev_per_day2(&self) -> f64 {
        self.mean_motion_first_derivative_scaled as f64 / SCALE_8 as f64
    }

    /// Returns the second mean-motion derivative divided by six, in rev/day³.
    #[must_use]
    pub fn mean_motion_second_derivative_rev_per_day3(&self) -> f64 {
        self.mean_motion_second_derivative.value()
    }

    /// Returns the TLE B* drag term in inverse Earth radii.
    #[must_use]
    pub fn b_star_inverse_earth_radii(&self) -> f64 {
        self.b_star.value()
    }

    /// Returns the ephemeris-type digit.
    #[must_use]
    pub const fn ephemeris_type(&self) -> u8 {
        self.ephemeris_type
    }

    /// Returns the element-set number.
    #[must_use]
    pub const fn element_set_number(&self) -> u16 {
        self.element_set_number
    }

    /// Returns inclination in degrees.
    #[must_use]
    pub fn inclination_deg(&self) -> f64 {
        self.inclination_scaled as f64 / SCALE_4 as f64
    }

    /// Returns right ascension of the ascending node in degrees.
    #[must_use]
    pub fn right_ascension_of_ascending_node_deg(&self) -> f64 {
        self.right_ascension_scaled as f64 / SCALE_4 as f64
    }

    /// Returns dimensionless eccentricity.
    #[must_use]
    pub fn eccentricity(&self) -> f64 {
        self.eccentricity_scaled as f64 / SCALE_7 as f64
    }

    /// Returns argument of perigee in degrees.
    #[must_use]
    pub fn argument_of_perigee_deg(&self) -> f64 {
        self.argument_of_perigee_scaled as f64 / SCALE_4 as f64
    }

    /// Returns mean anomaly in degrees.
    #[must_use]
    pub fn mean_anomaly_deg(&self) -> f64 {
        self.mean_anomaly_scaled as f64 / SCALE_4 as f64
    }

    /// Returns mean motion in revolutions per day.
    #[must_use]
    pub fn mean_motion_rev_per_day(&self) -> f64 {
        self.mean_motion_scaled as f64 / SCALE_8 as f64
    }

    /// Returns the revolution number at epoch.
    #[must_use]
    pub const fn revolution_number_at_epoch(&self) -> u32 {
        self.revolution_number
    }

    fn formatted_lines(&self) -> (String, String) {
        let catalog = format_catalog_number(self.satellite_catalog_number);
        let international_designator = self.international_designator.as_deref().unwrap_or("");
        let line_one_without_checksum = format!(
            "1 {catalog}{} {international_designator:<8} {:02}{} {} {} {} {} {:>4}",
            self.classification,
            self.epoch_year_two_digits,
            format_scaled(self.epoch_day_scaled, 3, 8, true),
            format_signed_fraction(self.mean_motion_first_derivative_scaled),
            format_implied_exponent(self.mean_motion_second_derivative),
            format_implied_exponent(self.b_star),
            self.ephemeris_type,
            self.element_set_number,
        );
        let line_two_without_checksum = format!(
            "2 {catalog} {} {} {:07} {} {} {}{:>5}",
            format_scaled(self.inclination_scaled, 3, 4, false),
            format_scaled(self.right_ascension_scaled, 3, 4, false),
            self.eccentricity_scaled,
            format_scaled(self.argument_of_perigee_scaled, 3, 4, false),
            format_scaled(self.mean_anomaly_scaled, 3, 4, false),
            format_scaled(self.mean_motion_scaled, 2, 8, false),
            self.revolution_number,
        );
        debug_assert_eq!(line_one_without_checksum.len(), 68);
        debug_assert_eq!(line_two_without_checksum.len(), 68);
        let line_one_checksum = checksum(line_one_without_checksum.as_bytes());
        let line_two_checksum = checksum(line_two_without_checksum.as_bytes());
        (
            format!("{line_one_without_checksum}{line_one_checksum}"),
            format!("{line_two_without_checksum}{line_two_checksum}"),
        )
    }
}

impl FromStr for TwoLineElement {
    type Err = TleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut lines = value.lines();
        let line_one = lines.next().ok_or(TleError::InvalidLineCount)?;
        let line_two = lines.next().ok_or(TleError::InvalidLineCount)?;
        if lines.next().is_some() {
            return Err(TleError::InvalidLineCount);
        }
        Self::parse(line_one, line_two)
    }
}

impl fmt::Display for TwoLineElement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line_one, line_two) = self.formatted_lines();
        write!(formatter, "{line_one}\n{line_two}")
    }
}

impl ImpliedExponent {
    fn value(self) -> f64 {
        f64::from(self.mantissa) / 100_000.0 * 10_f64.powi(i32::from(self.exponent))
    }
}

const fn expand_year(year: u8) -> u16 {
    if year >= 57 {
        1900 + year as u16
    } else {
        2000 + year as u16
    }
}

const fn days_in_year(year: u16) -> u16 {
    if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
        366
    } else {
        365
    }
}

fn validate_line(value: &str, line: TleLine) -> Result<&[u8], TleError> {
    if value.len() != LINE_LENGTH {
        return Err(TleError::InvalidLineLength {
            line,
            actual: value.len(),
        });
    }
    if let Some(column) = value.bytes().position(|byte| !byte.is_ascii()) {
        return Err(TleError::NonAscii {
            line,
            column: column + 1,
        });
    }
    Ok(value.as_bytes())
}

fn validate_checksum(bytes: &[u8], line: TleLine) -> Result<(), TleError> {
    let found = bytes[68];
    if !found.is_ascii_digit() {
        return Err(invalid_character(line, TleField::Checksum, 69));
    }
    let found = found - b'0';
    let expected = checksum(&bytes[..68]);
    if found != expected {
        return Err(TleError::ChecksumMismatch {
            line,
            expected,
            found,
        });
    }
    Ok(())
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0_u8, |sum, byte| {
        let value = match byte {
            b'0'..=b'9' => byte - b'0',
            b'-' => 1,
            _ => 0,
        };
        (sum + value) % 10
    })
}

fn validate_fixed_columns(bytes: &[u8], line: TleLine) -> Result<(), TleError> {
    let (line_number, spaces): (u8, &[usize]) = match line {
        TleLine::One => (b'1', &[1, 8, 17, 32, 43, 52, 61, 63]),
        TleLine::Two => (b'2', &[1, 7, 16, 25, 33, 42, 51]),
    };
    if bytes[0] != line_number {
        return Err(invalid_character(line, TleField::LineNumber, 1));
    }
    for &index in spaces {
        if bytes[index] != b' ' {
            return Err(invalid_character(line, TleField::Separator, index + 1));
        }
    }
    Ok(())
}

fn parse_catalog_number(bytes: &[u8], line: TleLine, column: usize) -> Result<u32, TleError> {
    if bytes[0].is_ascii_uppercase() {
        let prefix = alpha5_prefix(bytes[0])
            .ok_or_else(|| invalid_character(line, TleField::SatelliteCatalogNumber, column))?;
        let suffix = parse_digits(
            &bytes[1..],
            line,
            TleField::SatelliteCatalogNumber,
            column + 1,
        )? as u32;
        return Ok(u32::from(prefix) * 10_000 + suffix);
    }
    parse_space_padded_integer(bytes, line, TleField::SatelliteCatalogNumber, column)
        .map(|value| value as u32)
}

const fn alpha5_prefix(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'H' => Some(byte - b'A' + 10),
        b'J'..=b'N' => Some(byte - b'J' + 18),
        b'P'..=b'Z' => Some(byte - b'P' + 23),
        _ => None,
    }
}

fn format_catalog_number(value: u32) -> String {
    if value < 100_000 {
        return format!("{value:05}");
    }
    let prefix = (value / 10_000) as u8;
    let letter = match prefix {
        10..=17 => b'A' + prefix - 10,
        18..=22 => b'J' + prefix - 18,
        23..=33 => b'P' + prefix - 23,
        _ => unreachable!("validated Alpha-5 catalog number"),
    };
    format!("{}{:04}", char::from(letter), value % 10_000)
}

fn parse_international_designator(bytes: &[u8]) -> Result<Option<String>, TleError> {
    if bytes.iter().all(|byte| *byte == b' ') {
        return Ok(None);
    }
    parse_digits(
        &bytes[..2],
        TleLine::One,
        TleField::InternationalDesignator,
        10,
    )?;
    parse_digits(
        &bytes[2..5],
        TleLine::One,
        TleField::InternationalDesignator,
        12,
    )?;
    let piece = &bytes[5..];
    let first = piece
        .iter()
        .position(|byte| *byte != b' ')
        .ok_or_else(|| invalid_character(TleLine::One, TleField::InternationalDesignator, 15))?;
    let last = piece
        .iter()
        .rposition(|byte| *byte != b' ')
        .unwrap_or(first);
    if let Some(offset) = piece[first..=last]
        .iter()
        .position(|byte| !byte.is_ascii_uppercase())
    {
        return Err(invalid_character(
            TleLine::One,
            TleField::InternationalDesignator,
            15 + first + offset,
        ));
    }
    let text = bytes
        .iter()
        .map(|byte| char::from(*byte))
        .collect::<String>();
    Ok(Some(text.trim().to_owned()))
}

fn parse_signed_fraction(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<i64, TleError> {
    if !matches!(bytes[0], b' ' | b'+' | b'-') {
        return Err(invalid_character(line, field, column));
    }
    if bytes[1] != b'.' {
        return Err(invalid_character(line, field, column + 1));
    }
    let magnitude = parse_digits(&bytes[2..], line, field, column + 2)? as i64;
    Ok(if bytes[0] == b'-' {
        -magnitude
    } else {
        magnitude
    })
}

fn parse_implied_exponent(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<ImpliedExponent, TleError> {
    if bytes.iter().all(|byte| *byte == b' ') {
        return Ok(ImpliedExponent {
            mantissa: 0,
            exponent: 0,
        });
    }
    if !matches!(bytes[0], b' ' | b'+' | b'-') {
        return Err(invalid_character(line, field, column));
    }
    let magnitude = parse_digits(&bytes[1..6], line, field, column + 1)? as i32;
    if !matches!(bytes[6], b'+' | b'-') {
        return Err(invalid_character(line, field, column + 6));
    }
    let exponent = parse_digits(&bytes[7..8], line, field, column + 7)? as i8;
    Ok(ImpliedExponent {
        mantissa: if bytes[0] == b'-' {
            -magnitude
        } else {
            magnitude
        },
        exponent: if bytes[6] == b'-' {
            -exponent
        } else {
            exponent
        },
    })
}

fn parse_fixed_decimal(
    bytes: &[u8],
    integer_width: usize,
    fractional_width: usize,
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<u64, TleError> {
    debug_assert_eq!(bytes.len(), integer_width + fractional_width + 1);
    if bytes[integer_width] != b'.' {
        return Err(invalid_character(line, field, column + integer_width));
    }
    let integer = parse_space_padded_integer(&bytes[..integer_width], line, field, column)?;
    let fraction = parse_digits(
        &bytes[integer_width + 1..],
        line,
        field,
        column + integer_width + 1,
    )?;
    Ok(integer * 10_u64.pow(fractional_width as u32) + fraction)
}

fn parse_space_padded_integer(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<u64, TleError> {
    let first_digit = bytes
        .iter()
        .position(u8::is_ascii_digit)
        .ok_or_else(|| invalid_character(line, field, column))?;
    if let Some(offset) = bytes[..first_digit].iter().position(|byte| *byte != b' ') {
        return Err(invalid_character(line, field, column + offset));
    }
    parse_digits(&bytes[first_digit..], line, field, column + first_digit)
}

fn parse_digits(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<u64, TleError> {
    let mut value = 0_u64;
    for (offset, byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(invalid_character(line, field, column + offset));
        }
        value = value * 10 + u64::from(byte - b'0');
    }
    Ok(value)
}

fn validate_full_angle(value: u64, field: TleField) -> Result<(), TleError> {
    if value > 360 * SCALE_4 {
        Err(out_of_range(TleLine::Two, field))
    } else {
        Ok(())
    }
}

fn invalid_character(line: TleLine, field: TleField, column: usize) -> TleError {
    TleError::InvalidCharacter {
        line,
        field,
        column,
    }
}

fn out_of_range(line: TleLine, field: TleField) -> TleError {
    TleError::ValueOutOfRange { line, field }
}

fn format_scaled(
    value: u64,
    integer_width: usize,
    fractional_width: usize,
    zero_pad: bool,
) -> String {
    let scale = 10_u64.pow(fractional_width as u32);
    let integer = value / scale;
    let fraction = value % scale;
    let integer = if zero_pad {
        format!("{integer:0integer_width$}")
    } else {
        format!("{integer:>integer_width$}")
    };
    format!("{integer}.{fraction:0fractional_width$}")
}

fn format_signed_fraction(value: i64) -> String {
    let sign = if value < 0 { '-' } else { ' ' };
    format!("{sign}.{:08}", value.unsigned_abs())
}

fn format_implied_exponent(value: ImpliedExponent) -> String {
    let mantissa_sign = if value.mantissa < 0 { '-' } else { ' ' };
    let exponent_sign = if value.exponent < 0 { '-' } else { '+' };
    format!(
        "{mantissa_sign}{:05}{exponent_sign}{}",
        value.mantissa.unsigned_abs(),
        value.exponent.unsigned_abs()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOAA_LINE_1: &str =
        "1 23455U 94089A   97320.90946019  .00000140  00000-0  10191-3 0  2621";
    const NOAA_LINE_2: &str =
        "2 23455  99.0090 272.6745 0008546 223.1686 136.8816 14.11711747148495";
    const NOAA_CANONICAL_LINE_1: &str =
        "1 23455U 94089A   97320.90946019  .00000140  00000+0  10191-3 0  2620";

    #[test]
    fn celestrak_noaa_14_vector_parses_and_formats_canonically() {
        let tle = TwoLineElement::parse(NOAA_LINE_1, NOAA_LINE_2).expect("published TLE");

        assert_eq!(tle.satellite_catalog_number(), 23_455);
        assert_eq!(tle.classification(), 'U');
        assert_eq!(tle.international_designator(), Some("94089A"));
        assert_eq!(tle.epoch_year(), 1997);
        assert_eq!(tle.epoch_day_utc(), 320.90946019);
        assert_eq!(tle.mean_motion_first_derivative_rev_per_day2(), 0.00000140);
        assert_eq!(tle.mean_motion_second_derivative_rev_per_day3(), 0.0);
        assert_eq!(tle.b_star_inverse_earth_radii(), 0.00010191);
        assert_eq!(tle.inclination_deg(), 99.0090);
        assert_eq!(tle.right_ascension_of_ascending_node_deg(), 272.6745);
        assert_eq!(tle.eccentricity(), 0.0008546);
        assert_eq!(tle.argument_of_perigee_deg(), 223.1686);
        assert_eq!(tle.mean_anomaly_deg(), 136.8816);
        assert_eq!(tle.mean_motion_rev_per_day(), 14.11711747);
        assert_eq!(tle.revolution_number_at_epoch(), 14_849);
        assert_eq!(
            tle.to_string(),
            format!("{NOAA_CANONICAL_LINE_1}\n{NOAA_LINE_2}")
        );

        let reparsed: TwoLineElement = tle.to_string().parse().expect("canonical output");
        assert_eq!(reparsed, tle);
    }

    #[test]
    fn standard_checksum_counts_digits_and_minus_signs() {
        assert_eq!(checksum(&NOAA_LINE_1.as_bytes()[..68]), 1);
        assert_eq!(checksum(&NOAA_LINE_2.as_bytes()[..68]), 5);
    }

    #[test]
    fn space_track_alpha5_examples_decode_and_encode() {
        for (encoded, decoded) in [
            (b"A0000".as_slice(), 100_000),
            (b"E8493".as_slice(), 148_493),
            (b"J2931".as_slice(), 182_931),
            (b"P4018".as_slice(), 234_018),
            (b"W1928".as_slice(), 301_928),
            (b"Z9999".as_slice(), 339_999),
        ] {
            assert_eq!(parse_catalog_number(encoded, TleLine::One, 3), Ok(decoded));
            assert_eq!(
                format_catalog_number(decoded),
                String::from_utf8_lossy(encoded)
            );
        }
    }

    #[test]
    fn rejects_bad_checksum_with_line_context() {
        let mut damaged = NOAA_LINE_1.to_owned();
        damaged.replace_range(68..69, "0");
        assert_eq!(
            TwoLineElement::parse(&damaged, NOAA_LINE_2),
            Err(TleError::ChecksumMismatch {
                line: TleLine::One,
                expected: 1,
                found: 0,
            })
        );
    }

    #[test]
    fn rejects_wrong_length_and_extra_lines_without_unbounded_collection() {
        assert_eq!(
            TwoLineElement::parse(&NOAA_LINE_1[..68], NOAA_LINE_2),
            Err(TleError::InvalidLineLength {
                line: TleLine::One,
                actual: 68,
            })
        );
        let three_lines = format!("{NOAA_LINE_1}\n{NOAA_LINE_2}\nname");
        assert_eq!(
            three_lines.parse::<TwoLineElement>(),
            Err(TleError::InvalidLineCount)
        );
    }

    #[test]
    fn rejects_mismatched_catalog_numbers_after_valid_checksums() {
        let changed = replace_field_and_checksum(NOAA_LINE_2, 2..7, "23456");
        assert_eq!(
            TwoLineElement::parse(NOAA_LINE_1, &changed),
            Err(TleError::CatalogNumberMismatch {
                line_one: 23_455,
                line_two: 23_456,
            })
        );
    }

    #[test]
    fn rejects_invalid_fixed_column_and_range_with_field_context() {
        let fixed_column = replace_field_and_checksum(NOAA_LINE_2, 7..8, "0");
        assert_eq!(
            TwoLineElement::parse(NOAA_LINE_1, &fixed_column),
            Err(TleError::InvalidCharacter {
                line: TleLine::Two,
                field: TleField::Separator,
                column: 8,
            })
        );

        let inclination = replace_field_and_checksum(NOAA_LINE_2, 8..16, "181.0000");
        assert_eq!(
            TwoLineElement::parse(NOAA_LINE_1, &inclination),
            Err(TleError::ValueOutOfRange {
                line: TleLine::Two,
                field: TleField::Inclination,
            })
        );

        let classification = replace_field_and_checksum(NOAA_LINE_1, 7..8, "A");
        assert_eq!(
            TwoLineElement::parse(&classification, NOAA_LINE_2),
            Err(TleError::InvalidCharacter {
                line: TleLine::One,
                field: TleField::Classification,
                column: 8,
            })
        );
    }

    #[test]
    fn accepts_fractional_last_day_and_reports_non_ascii_column() {
        let last_day = replace_field_and_checksum(NOAA_LINE_1, 20..32, "365.50000000");
        let parsed = TwoLineElement::parse(&last_day, NOAA_LINE_2).expect("last day of 1997");
        assert_eq!(parsed.epoch_day_utc(), 365.5);

        let mut non_ascii = NOAA_LINE_1.to_owned();
        non_ascii.replace_range(9..11, "é");
        assert_eq!(
            TwoLineElement::parse(&non_ascii, NOAA_LINE_2),
            Err(TleError::NonAscii {
                line: TleLine::One,
                column: 10,
            })
        );
    }

    #[test]
    fn epoch_day_starts_at_one() {
        let day_zero = replace_field_and_checksum(NOAA_LINE_1, 20..32, "000.50000000");
        assert_eq!(
            TwoLineElement::parse(&day_zero, NOAA_LINE_2),
            Err(TleError::ValueOutOfRange {
                line: TleLine::One,
                field: TleField::EpochDay,
            })
        );

        let first_day = replace_field_and_checksum(NOAA_LINE_1, 20..32, "001.00000000");
        let parsed = TwoLineElement::parse(&first_day, NOAA_LINE_2).expect("first UTC day");
        assert_eq!(parsed.epoch_day_utc(), 1.0);
    }

    fn replace_field_and_checksum(
        source: &str,
        range: std::ops::Range<usize>,
        replacement: &str,
    ) -> String {
        let mut result = source.to_owned();
        result.replace_range(range, replacement);
        let calculated = checksum(&result.as_bytes()[..68]);
        result.replace_range(68..69, &calculated.to_string());
        result
    }
}
