#![forbid(unsafe_code)]

//! Strict parsing and lossless formatting of NORAD two- and three-line elements.
//!
//! The parser validates the standard 69-column ASCII records, checksums,
//! fixed-column syntax, numeric ranges, and matching catalog identifiers.
//! The unmodified `sgp4` dependency parses and stores the orbital elements.
//! A narrow adapter adds strict syntax/range validation and lossless text output
//! (the dependency has no TLE text writer). An optional preceding name record
//! is preserved for 3LE input. Serde reads/writes named source records and
//! revalidates them on deserialization, rather than trusting cached elements.
//! Epoch years use the
//! conventional pivot: `57..=99` means 1957–1999 and `00..=56` means
//! 2000–2056.
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
//! assert_eq!(tle.epoch_year(), 1997);
//! assert_eq!(tle.to_string().lines().count(), 2);
//! let named: TwoLineElement = format!("0 NOAA 14\n{tle}").parse()?;
//! let json = serde_json::to_string(&named)?;
//! let restored: TwoLineElement = serde_json::from_str(&json)?;
//! assert_eq!(restored.object_name(), Some("0 NOAA 14"));
//! assert_eq!(restored.to_string(), named.to_string());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{fmt, str::FromStr};

use ::sgp4::chrono::{Datelike, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "sgp4")]
mod sgp4;

#[cfg(feature = "sgp4")]
pub use sgp4::Sgp4ConversionError;

const LINE_LENGTH: usize = 69;

/// The TLE record line associated with a parse failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TleLine {
    /// Identification, epoch, derivatives, and B* fields.
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

/// A named fixed-width TLE field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TleField {
    /// Line number.
    LineNumber,
    /// Required separator space.
    Separator,
    /// NORAD catalog identifier.
    SatelliteCatalogNumber,
    /// Classification character.
    Classification,
    /// International launch designator.
    InternationalDesignator,
    /// Two-digit epoch year.
    EpochYear,
    /// Epoch day and fraction.
    EpochDay,
    /// First mean-motion derivative divided by two.
    MeanMotionFirstDerivative,
    /// Second mean-motion derivative divided by six.
    MeanMotionSecondDerivative,
    /// SGP4 B* coefficient.
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
    /// Revolution number.
    RevolutionNumber,
    /// Checksum digit.
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
            Self::BStar => "B*",
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

/// A recoverable error parsing an untrusted TLE record.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum TleError {
    /// A source line was not exactly 69 ASCII bytes.
    #[error("{line} must contain exactly 69 ASCII bytes, found {actual}")]
    InvalidLineLength {
        /// Affected line.
        line: TleLine,
        /// Observed byte length.
        actual: usize,
    },
    /// A non-ASCII byte appeared in a source line.
    #[error("{line} column {column} is not ASCII")]
    NonAscii {
        /// Affected line.
        line: TleLine,
        /// One-based column.
        column: usize,
    },
    /// A column did not contain the character required by its field.
    #[error("{line} column {column} is invalid for {field}")]
    InvalidCharacter {
        /// Affected line.
        line: TleLine,
        /// Affected field.
        field: TleField,
        /// One-based column.
        column: usize,
    },
    /// A parsed field value was outside its representable TLE range.
    #[error("{line} field {field} is outside the TLE range")]
    ValueOutOfRange {
        /// Affected line.
        line: TleLine,
        /// Affected field.
        field: TleField,
    },
    /// The checksum digit did not match the checksum of columns 1–68.
    #[error("{line} checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch {
        /// Affected line.
        line: TleLine,
        /// Computed checksum.
        expected: u8,
        /// Supplied checksum.
        found: u8,
    },
    /// The records identify different catalog objects.
    #[error("catalog number mismatch: line 1 identifies {line_one}, line 2 identifies {line_two}")]
    CatalogNumberMismatch {
        /// Catalog number on line 1.
        line_one: u32,
        /// Catalog number on line 2.
        line_two: u32,
    },
    /// Combined input did not contain two data lines and an optional name.
    #[error("a TLE must contain two data lines and optionally a preceding name")]
    InvalidLineCount,
    /// A 3LE name is empty or contains non-printable/non-ASCII characters.
    #[error("a 3LE name must contain nonblank printable ASCII text")]
    InvalidObjectName,
    /// The external parser rejected a syntax-validated record.
    #[error("SGP4 TLE parser rejected the record: {0}")]
    Parser(#[from] ::sgp4::TleError),
}

// The pinned parser's errors contain only categorical and integer location data.
impl Eq for TleError {}

/// Validated fixed-column TLE data.
///
/// The original 69-character lines are retained. [`Display`](fmt::Display)
/// therefore preserves all accepted column content rather than normalizing
/// representational choices.
/// Serde represents this type as `object_name` (optional), `line_one`, and
/// `line_two`. Deserialization uses the same strict parser as text input.
/// [`AsRef`] borrows the dependency's elements; their serde representation is
/// OMM data, not TLE text, and does not preserve original TLE spelling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "TleRecord", into = "TleRecord")]
pub struct TwoLineElement {
    line_one: String,
    line_two: String,
    elements: ::sgp4::Elements,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TleRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    object_name: Option<String>,
    line_one: String,
    line_two: String,
}

impl TwoLineElement {
    /// Parses two 69-byte ASCII records, validating checksums, columns, ranges,
    /// and catalog-number agreement.
    pub fn parse(line_one: &str, line_two: &str) -> Result<Self, TleError> {
        Self::parse_named(None, line_one, line_two)
    }

    fn parse_named(
        object_name: Option<String>,
        line_one: &str,
        line_two: &str,
    ) -> Result<Self, TleError> {
        if object_name.as_ref().is_some_and(|name| {
            name.trim().is_empty() || !name.bytes().all(|byte| (b' '..=b'~').contains(&byte))
        }) {
            return Err(TleError::InvalidObjectName);
        }
        let one = validate_line(line_one, TleLine::One)?;
        let two = validate_line(line_two, TleLine::Two)?;
        validate_checksum(one, TleLine::One)?;
        validate_checksum(two, TleLine::Two)?;
        validate_fixed_columns(one, TleLine::One)?;
        validate_fixed_columns(two, TleLine::Two)?;

        let catalog_number = parse_catalog(&one[2..7], TleLine::One, 3)?;
        let second_catalog = parse_catalog(&two[2..7], TleLine::Two, 3)?;
        if catalog_number != second_catalog {
            return Err(TleError::CatalogNumberMismatch {
                line_one: catalog_number,
                line_two: second_catalog,
            });
        }
        if !matches!(one[7], b'U' | b'S' | b'C') {
            return Err(invalid(TleLine::One, TleField::Classification, 8));
        }

        validate_designator(&one[9..17])?;
        let epoch_year_two_digits =
            parse_digits(&one[18..20], TleLine::One, TleField::EpochYear, 19)? as u8;
        let epoch_day_scaled =
            parse_decimal(&one[20..32], 3, 8, TleLine::One, TleField::EpochDay, 21)?;
        let year = expand_year(epoch_year_two_digits);
        let date = NaiveDate::from_yo_opt(i32::from(year), (epoch_day_scaled / 1E8 as u64) as u32)
            .ok_or_else(|| out_of_range(TleLine::One, TleField::EpochDay))?;
        validate_signed_fraction(
            &one[33..43],
            TleLine::One,
            TleField::MeanMotionFirstDerivative,
            34,
        )?;
        validate_implied_exponent(
            &one[44..52],
            TleLine::One,
            TleField::MeanMotionSecondDerivative,
            45,
        )?;
        validate_implied_exponent(&one[53..61], TleLine::One, TleField::BStar, 54)?;
        parse_digits(&one[62..63], TleLine::One, TleField::EphemerisType, 63)?;
        parse_padded_integer(&one[64..68], TleLine::One, TleField::ElementSetNumber, 65)?;

        let inclination_scaled =
            parse_decimal(&two[8..16], 3, 4, TleLine::Two, TleField::Inclination, 9)?;
        if inclination_scaled > (180.0 * 1E4) as u64 {
            return Err(out_of_range(TleLine::Two, TleField::Inclination));
        }
        let raan_scaled = parse_decimal(
            &two[17..25],
            3,
            4,
            TleLine::Two,
            TleField::RightAscensionOfAscendingNode,
            18,
        )?;
        validate_angle(raan_scaled, TleField::RightAscensionOfAscendingNode)?;
        parse_digits(&two[26..33], TleLine::Two, TleField::Eccentricity, 27)?;
        let argument_of_perigee_scaled = parse_decimal(
            &two[34..42],
            3,
            4,
            TleLine::Two,
            TleField::ArgumentOfPerigee,
            35,
        )?;
        validate_angle(argument_of_perigee_scaled, TleField::ArgumentOfPerigee)?;
        let mean_anomaly_scaled =
            parse_decimal(&two[43..51], 3, 4, TleLine::Two, TleField::MeanAnomaly, 44)?;
        validate_angle(mean_anomaly_scaled, TleField::MeanAnomaly)?;
        let mean_motion_scaled =
            parse_decimal(&two[52..63], 2, 8, TleLine::Two, TleField::MeanMotion, 53)?;
        if mean_motion_scaled == 0 {
            return Err(out_of_range(TleLine::Two, TleField::MeanMotion));
        }
        parse_padded_integer(&two[63..68], TleLine::Two, TleField::RevolutionNumber, 64)?;

        // Preserve accepted blank zero fields in output; normalize only the
        // dependency's parsing input, including its checksum.
        let mut parser_line = line_one.to_owned();
        for range in [44..52, 53..61] {
            if one[range.clone()].iter().all(|byte| *byte == b' ') {
                parser_line.replace_range(range, " 00000-0");
            }
        }
        let check = checksum(&parser_line.as_bytes()[..68]);
        parser_line.replace_range(68..69, &check.to_string());
        let mut elements = ::sgp4::Elements::from_tle(object_name, parser_line.as_bytes(), two)?;
        // TLE fractions are exact multiples of 864 microseconds. Correct the
        // dependency's floating-point day conversion without changing the epoch.
        let nanoseconds = (epoch_day_scaled % 1E8 as u64) * 864_000;
        elements.datetime = date
            .and_hms_nano_opt(
                (nanoseconds / 1_000_000_000 / 3_600) as u32,
                (nanoseconds / 1_000_000_000 % 3_600 / 60) as u32,
                (nanoseconds / 1_000_000_000 % 60) as u32,
                (nanoseconds % 1_000_000_000) as u32,
            )
            .ok_or_else(|| out_of_range(TleLine::One, TleField::EpochDay))?;

        Ok(Self {
            line_one: line_one.to_owned(),
            line_two: line_two.to_owned(),
            elements,
        })
    }

    /// Returns the decoded NORAD catalog number, including Alpha-5 identifiers.
    #[must_use]
    pub const fn satellite_catalog_number(&self) -> u32 {
        self.elements.norad_id as u32
    }

    /// Returns the classification character.
    #[must_use]
    pub const fn classification(&self) -> char {
        match self.elements.classification {
            ::sgp4::Classification::Unclassified => 'U',
            ::sgp4::Classification::Classified => 'C',
            ::sgp4::Classification::Secret => 'S',
        }
    }

    /// Returns the trimmed international launch designator, if present.
    #[must_use]
    pub fn international_designator(&self) -> Option<&str> {
        let designator = self.line_one[9..17].trim();
        (!designator.is_empty()).then_some(designator)
    }

    /// Returns the preceding 3LE name exactly as supplied, including any `0 ` prefix.
    #[must_use]
    pub fn object_name(&self) -> Option<&str> {
        self.elements.object_name.as_deref()
    }

    /// Returns the expanded epoch year using the standard 1957 pivot.
    #[must_use]
    pub fn epoch_year(&self) -> u16 {
        self.elements.datetime.year() as u16
    }

    /// Returns the UTC epoch day of year, including its fractional part.
    #[must_use]
    pub fn epoch_day_utc(&self) -> f64 {
        let datetime = self.elements.datetime;
        f64::from(datetime.ordinal())
            + (f64::from(datetime.num_seconds_from_midnight())
                + f64::from(datetime.nanosecond()) / 1E9)
                / 86_400.0
    }

    /// Returns the first derivative of mean motion divided by two, in rev/day².
    #[must_use]
    pub fn mean_motion_first_derivative_rev_per_day2(&self) -> f64 {
        self.elements.mean_motion_dot
    }

    /// Returns the second derivative of mean motion divided by six, in rev/day³.
    #[must_use]
    pub fn mean_motion_second_derivative_rev_per_day3(&self) -> f64 {
        self.elements.mean_motion_ddot
    }

    /// Returns the SGP4 B* coefficient in inverse Earth radii.
    #[must_use]
    pub fn b_star_inverse_earth_radii(&self) -> f64 {
        self.elements.drag_term
    }

    /// Returns the ephemeris type.
    #[must_use]
    pub const fn ephemeris_type(&self) -> u8 {
        self.elements.ephemeris_type
    }

    /// Returns the element-set number.
    #[must_use]
    pub const fn element_set_number(&self) -> u16 {
        self.elements.element_set_number as u16
    }

    /// Returns inclination in degrees.
    #[must_use]
    pub fn inclination_deg(&self) -> f64 {
        self.elements.inclination
    }

    /// Returns right ascension of the ascending node in degrees.
    #[must_use]
    pub fn right_ascension_of_ascending_node_deg(&self) -> f64 {
        self.elements.right_ascension
    }

    /// Returns dimensionless eccentricity.
    #[must_use]
    pub fn eccentricity(&self) -> f64 {
        self.elements.eccentricity
    }

    /// Returns argument of perigee in degrees.
    #[must_use]
    pub fn argument_of_perigee_deg(&self) -> f64 {
        self.elements.argument_of_perigee
    }

    /// Returns mean anomaly in degrees.
    #[must_use]
    pub fn mean_anomaly_deg(&self) -> f64 {
        self.elements.mean_anomaly
    }

    /// Returns mean motion in revolutions per day.
    #[must_use]
    pub fn mean_motion_rev_per_day(&self) -> f64 {
        self.elements.mean_motion
    }

    /// Returns the revolution number at epoch.
    #[must_use]
    pub const fn revolution_number_at_epoch(&self) -> u32 {
        self.elements.revolution_number as u32
    }
}

impl FromStr for TwoLineElement {
    type Err = TleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut lines = value.lines();
        let first = lines.next().ok_or(TleError::InvalidLineCount)?;
        let second = lines.next().ok_or(TleError::InvalidLineCount)?;
        let third = lines.next();
        if lines.next().is_some() {
            return Err(TleError::InvalidLineCount);
        }
        match third {
            None => Self::parse(first, second),
            Some(_) if first.starts_with("1 ") && second.starts_with("2 ") => {
                Err(TleError::InvalidLineCount)
            }
            Some(third) => Self::parse_named(Some(first.to_owned()), second, third),
        }
    }
}

impl fmt::Display for TwoLineElement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.object_name() {
            writeln!(formatter, "{name}")?;
        }
        write!(formatter, "{}\n{}", self.line_one, self.line_two)
    }
}

impl AsRef<::sgp4::Elements> for TwoLineElement {
    fn as_ref(&self) -> &::sgp4::Elements {
        &self.elements
    }
}

impl PartialEq for TwoLineElement {
    fn eq(&self, other: &Self) -> bool {
        self.line_one == other.line_one
            && self.line_two == other.line_two
            && self.object_name() == other.object_name()
    }
}

impl Eq for TwoLineElement {}

impl From<TwoLineElement> for TleRecord {
    fn from(value: TwoLineElement) -> Self {
        Self {
            object_name: value.elements.object_name,
            line_one: value.line_one,
            line_two: value.line_two,
        }
    }
}

impl TryFrom<TleRecord> for TwoLineElement {
    type Error = TleError;

    fn try_from(value: TleRecord) -> Result<Self, Self::Error> {
        Self::parse_named(value.object_name, &value.line_one, &value.line_two)
    }
}

const fn expand_year(year: u8) -> u16 {
    if year >= 57 {
        1900 + year as u16
    } else {
        2000 + year as u16
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
    if !bytes[68].is_ascii_digit() {
        return Err(invalid(line, TleField::Checksum, 69));
    }
    let found = bytes[68] - b'0';
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
    bytes.iter().fold(0, |sum, byte| {
        (sum + match byte {
            b'0'..=b'9' => byte - b'0',
            b'-' => 1,
            _ => 0,
        }) % 10
    })
}

fn validate_fixed_columns(bytes: &[u8], line: TleLine) -> Result<(), TleError> {
    let (line_number, spaces): (u8, &[usize]) = match line {
        TleLine::One => (b'1', &[1, 8, 17, 32, 43, 52, 61, 63]),
        TleLine::Two => (b'2', &[1, 7, 16, 25, 33, 42, 51]),
    };
    if bytes[0] != line_number {
        return Err(invalid(line, TleField::LineNumber, 1));
    }
    for &index in spaces {
        if bytes[index] != b' ' {
            return Err(invalid(line, TleField::Separator, index + 1));
        }
    }
    Ok(())
}

fn parse_catalog(bytes: &[u8], line: TleLine, column: usize) -> Result<u32, TleError> {
    if bytes[0].is_ascii_uppercase() {
        let prefix = match bytes[0] {
            b'A'..=b'H' => bytes[0] - b'A' + 10,
            b'J'..=b'N' => bytes[0] - b'J' + 18,
            b'P'..=b'Z' => bytes[0] - b'P' + 23,
            _ => return Err(invalid(line, TleField::SatelliteCatalogNumber, column)),
        };
        return Ok(u32::from(prefix) * 10_000
            + parse_digits(
                &bytes[1..],
                line,
                TleField::SatelliteCatalogNumber,
                column + 1,
            )? as u32);
    }
    parse_padded_integer(bytes, line, TleField::SatelliteCatalogNumber, column)
        .map(|value| value as u32)
}

fn validate_designator(bytes: &[u8]) -> Result<(), TleError> {
    if bytes.iter().all(|byte| *byte == b' ') {
        return Ok(());
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
    let start = piece
        .iter()
        .position(|byte| *byte != b' ')
        .ok_or_else(|| invalid(TleLine::One, TleField::InternationalDesignator, 15))?;
    let end = piece
        .iter()
        .rposition(|byte| *byte != b' ')
        .unwrap_or(start);
    if let Some(offset) = piece[start..=end]
        .iter()
        .position(|byte| !byte.is_ascii_uppercase())
    {
        return Err(invalid(
            TleLine::One,
            TleField::InternationalDesignator,
            15 + start + offset,
        ));
    }
    Ok(())
}

fn validate_signed_fraction(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<(), TleError> {
    if !matches!(bytes[0], b' ' | b'+' | b'-') {
        return Err(invalid(line, field, column));
    }
    if bytes[1] != b'.' {
        return Err(invalid(line, field, column + 1));
    }
    parse_digits(&bytes[2..], line, field, column + 2)?;
    Ok(())
}

fn validate_implied_exponent(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<(), TleError> {
    if bytes.iter().all(|byte| *byte == b' ') {
        return Ok(());
    }
    if !matches!(bytes[0], b' ' | b'+' | b'-') {
        return Err(invalid(line, field, column));
    }
    parse_digits(&bytes[1..6], line, field, column + 1)?;
    if !matches!(bytes[6], b'+' | b'-') {
        return Err(invalid(line, field, column + 6));
    }
    parse_digits(&bytes[7..8], line, field, column + 7)?;
    Ok(())
}

fn parse_decimal(
    bytes: &[u8],
    integer_width: usize,
    fractional_width: usize,
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<u64, TleError> {
    if bytes[integer_width] != b'.' {
        return Err(invalid(line, field, column + integer_width));
    }
    let integer = parse_padded_integer(&bytes[..integer_width], line, field, column)?;
    let fraction = parse_digits(
        &bytes[integer_width + 1..],
        line,
        field,
        column + integer_width + 1,
    )?;
    Ok(integer * 10_u64.pow(fractional_width as u32) + fraction)
}

fn parse_padded_integer(
    bytes: &[u8],
    line: TleLine,
    field: TleField,
    column: usize,
) -> Result<u64, TleError> {
    let first = bytes
        .iter()
        .position(u8::is_ascii_digit)
        .ok_or_else(|| invalid(line, field, column))?;
    if let Some(offset) = bytes[..first].iter().position(|byte| *byte != b' ') {
        return Err(invalid(line, field, column + offset));
    }
    parse_digits(&bytes[first..], line, field, column + first)
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
            return Err(invalid(line, field, column + offset));
        }
        value = value * 10 + u64::from(byte - b'0');
    }
    Ok(value)
}

fn validate_angle(value: u64, field: TleField) -> Result<(), TleError> {
    if value > (360.0 * 1E4) as u64 {
        Err(out_of_range(TleLine::Two, field))
    } else {
        Ok(())
    }
}

fn invalid(line: TleLine, field: TleField, column: usize) -> TleError {
    TleError::InvalidCharacter {
        line,
        field,
        column,
    }
}

fn out_of_range(line: TleLine, field: TleField) -> TleError {
    TleError::ValueOutOfRange { line, field }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_1: &str = "1 23455U 94089A   97320.90946019  .00000140  00000-0  10191-3 0  2621";
    const LINE_2: &str = "2 23455  99.0090 272.6745 0008546 223.1686 136.8816 14.11711747148495";

    #[test]
    fn parses_values_and_round_trips_original_columns_losslessly() {
        let tle = TwoLineElement::parse(LINE_1, LINE_2).expect("valid records");
        assert_eq!(tle.satellite_catalog_number(), 23_455);
        assert_eq!(tle.epoch_year(), 1997);
        assert_eq!(tle.epoch_day_utc(), 320.90946019);
        assert_eq!(tle.mean_motion_rev_per_day(), 14.11711747);
        assert_eq!(tle.international_designator(), Some("94089A"));
        assert_eq!(tle.to_string(), format!("{LINE_1}\n{LINE_2}"));
        assert_eq!(tle.to_string().parse::<TwoLineElement>(), Ok(tle));
    }

    #[test]
    fn validates_checksum_field_range_and_line_count() {
        let mut bad_checksum = LINE_1.to_owned();
        bad_checksum.replace_range(68..69, "0");
        assert!(matches!(
            TwoLineElement::parse(&bad_checksum, LINE_2),
            Err(TleError::ChecksumMismatch {
                line: TleLine::One,
                ..
            })
        ));

        let mut bad_inclination = LINE_2.to_owned();
        bad_inclination.replace_range(8..16, "181.0000");
        let checksum = checksum(&bad_inclination.as_bytes()[..68]);
        bad_inclination.replace_range(68..69, &checksum.to_string());
        assert!(matches!(
            TwoLineElement::parse(LINE_1, &bad_inclination),
            Err(TleError::ValueOutOfRange {
                field: TleField::Inclination,
                ..
            })
        ));
        assert_eq!(
            format!("{LINE_1}\n{LINE_2}\nname").parse::<TwoLineElement>(),
            Err(TleError::InvalidLineCount)
        );
    }

    #[test]
    fn epoch_year_pivot_and_leap_day_range_are_checked() {
        let mut line = LINE_1.to_owned();
        line.replace_range(18..20, "56");
        line.replace_range(20..32, "366.00000000");
        let checksum_digit = checksum(&line.as_bytes()[..68]);
        line.replace_range(68..69, &checksum_digit.to_string());
        assert_eq!(
            TwoLineElement::parse(&line, LINE_2).unwrap().epoch_year(),
            2056
        );

        line.replace_range(18..20, "57");
        let checksum_digit = checksum(&line.as_bytes()[..68]);
        line.replace_range(68..69, &checksum_digit.to_string());
        assert!(matches!(
            TwoLineElement::parse(&line, LINE_2),
            Err(TleError::ValueOutOfRange {
                field: TleField::EpochDay,
                ..
            })
        ));
    }

    #[test]
    fn alpha5_catalog_number_is_decoded_without_normalizing_the_records() {
        let line_one = replace_field(LINE_1, 2..7, "P4018");
        let line_two = replace_field(LINE_2, 2..7, "P4018");
        let parsed = TwoLineElement::parse(&line_one, &line_two).expect("valid Alpha-5 records");
        assert_eq!(parsed.satellite_catalog_number(), 234_018);
        assert_eq!(parsed.to_string(), format!("{line_one}\n{line_two}"));
        assert_eq!(parsed.as_ref().norad_id, 234_018);
    }

    #[test]
    fn named_three_line_records_and_serde_round_trip() {
        for name in ["NOAA 14", "0 NOAA 14", "1 NOAA 14"] {
            let source = format!("{name}\n{LINE_1}\n{LINE_2}");
            let tle: TwoLineElement = source.parse().expect("valid 3LE");
            assert_eq!(tle.object_name(), Some(name));
            assert_eq!(tle.as_ref().object_name.as_deref(), Some(name));
            assert_eq!(tle.to_string(), source);
            assert_eq!(
                source.replace('\n', "\r\n").parse::<TwoLineElement>(),
                Ok(tle.clone())
            );
            let json = serde_json::to_string(&tle).expect("serialize named records");
            let restored: TwoLineElement = serde_json::from_str(&json).expect("deserialize 3LE");
            assert_eq!(restored, tle);
            assert_eq!(restored.as_ref(), tle.as_ref());
            assert_eq!(restored.to_string(), source);
        }
    }

    #[test]
    fn serde_records_and_dependency_omm_are_distinct_formats() {
        let tle = TwoLineElement::parse(LINE_1, LINE_2).expect("valid TLE");
        let json = serde_json::to_value(&tle).expect("serialize TLE records");
        assert_eq!(json["line_one"], LINE_1);
        assert!(json.get("object_name").is_none());
        let restored: TwoLineElement = serde_json::from_value(json).expect("deserialize TLE");
        assert_eq!(restored, tle);
        let omm = serde_json::to_value(tle.as_ref()).expect("serialize external elements");
        assert_eq!(omm["NORAD_CAT_ID"], 23_455);
        assert_eq!(omm["OBJECT_ID"], "1994-089A");
        assert!(serde_json::from_value::<TwoLineElement>(omm.clone()).is_err());
        let restored_elements: ::sgp4::Elements =
            serde_json::from_value(omm).expect("deserialize external OMM");
        assert_eq!(&restored_elements, tle.as_ref());
        assert_eq!(tle.international_designator(), Some("94089A"));
    }

    #[test]
    fn serde_cannot_bypass_validation_or_inject_cached_elements() {
        let tle = TwoLineElement::parse(LINE_1, LINE_2).expect("valid TLE");
        let json = serde_json::to_value(&tle).expect("serialize records");
        for (key, value) in [
            ("line_one", serde_json::json!(format!("{}0", &LINE_1[..68]))),
            (
                "line_two",
                serde_json::json!(replace_field(LINE_2, 8..16, "181.0000")),
            ),
            ("object_name", serde_json::json!("bad\nname")),
            ("elements", serde_json::json!({})),
        ] {
            let mut altered = json.clone();
            altered[key] = value;
            assert!(serde_json::from_value::<TwoLineElement>(altered).is_err());
        }
    }

    #[test]
    fn source_preserving_writer_keeps_blank_zeros_and_signed_zero() {
        let line = replace_field(LINE_1, 33..43, "-.00000000");
        let line = replace_field(&line, 44..52, "        ");
        let line = replace_field(&line, 53..61, "        ");
        let tle = TwoLineElement::parse(&line, LINE_2).expect("accepted blank zero fields");
        assert_eq!(tle.mean_motion_second_derivative_rev_per_day3(), 0.0);
        assert_eq!(tle.b_star_inverse_earth_radii(), 0.0);
        assert!(tle
            .mean_motion_first_derivative_rev_per_day2()
            .is_sign_negative());
        assert_eq!(tle.to_string(), format!("{line}\n{LINE_2}"));
        let restored: TwoLineElement =
            serde_json::from_str(&serde_json::to_string(&tle).unwrap()).unwrap();
        assert_eq!(restored.to_string(), tle.to_string());
        assert_eq!(restored.to_string().parse::<TwoLineElement>(), Ok(tle));
    }

    #[test]
    fn malformed_names_line_counts_catalogs_and_columns_remain_errors() {
        for name in ["", "  ", "NOAA\t14", "NOAA é"] {
            assert_eq!(
                format!("{name}\n{LINE_1}\n{LINE_2}").parse::<TwoLineElement>(),
                Err(TleError::InvalidObjectName)
            );
        }
        assert_eq!(
            format!("name\n{LINE_1}\n{LINE_2}\nextra").parse::<TwoLineElement>(),
            Err(TleError::InvalidLineCount)
        );
        assert!(matches!(
            TwoLineElement::parse(LINE_1, &replace_field(LINE_2, 2..7, "23456")),
            Err(TleError::CatalogNumberMismatch {
                line_one: 23_455,
                line_two: 23_456
            })
        ));
        assert!(matches!(
            TwoLineElement::parse(&replace_field(LINE_1, 32..33, "\t"), LINE_2),
            Err(TleError::InvalidCharacter {
                line: TleLine::One,
                field: TleField::Separator,
                column: 33
            })
        ));
        for digit in [" ", "-", "A"] {
            let mut line = LINE_1.to_owned();
            line.replace_range(68..69, digit);
            assert!(matches!(
                TwoLineElement::parse(&line, LINE_2),
                Err(TleError::InvalidCharacter {
                    field: TleField::Checksum,
                    column: 69,
                    ..
                })
            ));
        }
    }

    #[cfg(feature = "sgp4")]
    #[test]
    fn conversion_rejects_nonzero_ephemeris_type() {
        let line_one = replace_field(LINE_1, 62..63, "1");
        let tle = TwoLineElement::parse(&line_one, LINE_2).expect("valid TLE syntax");
        assert!(matches!(
            dynamics::sgp4::Sgp4Propagator::try_from(&tle),
            Err(Sgp4ConversionError::UnsupportedEphemerisType { found: 1 })
        ));
    }

    fn replace_field(source: &str, range: std::ops::Range<usize>, replacement: &str) -> String {
        let mut result = source.to_owned();
        result.replace_range(range, replacement);
        let check = checksum(&result.as_bytes()[..68]);
        result.replace_range(68..69, &check.to_string());
        result
    }
}
