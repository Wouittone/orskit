#![forbid(unsafe_code)]

//! CCSDS navigation-message ingestion at the orskit I/O boundary.
//!
//! The first supported vertical slice is the CCSDS 502.0-B-3 Orbit
//! Ephemeris Message (OEM) in Key-Value Notation (KVN) and XML. The blocking
//! readers emit events while reading, so callers can process ephemeris points
//! without retaining a complete message. KVN additionally offers Tokio and
//! ordered Rayon ingestion.
//!
//! This implementation crate depends on the core contract library as
//! `orskit_core`; it cannot depend on the `orskit` facade because the facade
//! optionally re-exports `ccsds`. Applications use the public
//! `orskit::core::Epoch` path instead.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//! use ccsds::{OemEvent, OemKvnReader};
//!
//! let reader = OemKvnReader::new(BufReader::new(File::open("orbit.oem")?));
//! for event in reader {
//!     if let OemEvent::Coordinates(coordinates) = event? {
//!         println!(
//!             "{}: {:?}",
//!             coordinates.epoch(),
//!             coordinates.coordinates().position()
//!         );
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod oem;

pub use oem::{
    parse_oem_kvn, parse_oem_kvn_with_limits, parse_oem_xml, parse_oem_xml_with_limits,
    CartesianCovarianceEntry, DeclaredCovarianceAxes, Oem, OemCartesianCovariance, OemComment,
    OemCovarianceAxes, OemCovarianceFrame, OemDecoderLimits, OemDecoderLimitsError, OemError,
    OemEvent, OemHeader, OemKvnReader, OemLimitKind, OemMetadata, OemRecordRef, OemSample,
    OemSection, OemSegment, OemSegmentContext, OemSegmentId, OemTimeSystem, OemXmlReader,
    ReferenceCovarianceAxes, RtnCovarianceAxes,
};

#[cfg(feature = "async")]
pub use oem::AsyncOemKvnReader;

#[cfg(feature = "parallel")]
pub use oem::{parse_oem_kvn_parallel, parse_oem_kvn_parallel_with_limits};
