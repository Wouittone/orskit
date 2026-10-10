use dynamics::{sgp4::Sgp4Propagator, Propagator};
use hifitime::{Duration, Epoch};
use tle::TwoLineElement;

const POSITION_TOLERANCE_METRES: f64 = 1.0;
const VELOCITY_TOLERANCE_METRES_PER_SECOND: f64 = 0.001;

#[test]
fn vallado_near_earth_verification_record_propagates_to_teme() {
    let tle = TwoLineElement::parse(
        "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753",
        "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667",
    )
    .expect("published Vallado verification record");
    let propagator = Sgp4Propagator::try_from(&tle).expect("valid SGP4 record");
    let target = propagator.epoch() + Duration::from_seconds(360.0 * 60.0);
    let result = propagator
        .propagate(propagator.initial_orbit(), target)
        .expect("near-Earth prediction");

    assert_eq!(result.epoch(), target);
    assert_eq!(result.as_ref().frame(), frames::ReferenceFrame::TEME);
    assert_vector_close(
        result.as_ref().position().to_metres(),
        [-7_154.031_202_02, -3_783.176_825_04, -3_536.194_122_94].map(|value| value * 1_000.0),
        POSITION_TOLERANCE_METRES,
    );
    assert_vector_close(
        result.as_ref().velocity().to_metres_per_second(),
        [4.741_887_409, -4.151_817_765, -2.093_935_425].map(|value| value * 1_000.0),
        VELOCITY_TOLERANCE_METRES_PER_SECOND,
    );
}

#[test]
fn vallado_deep_space_verification_record_propagates_to_teme() {
    let tle = TwoLineElement::parse(
        "1 04632U 70093B   04031.91070959 -.00000084  00000-0  10000-3 0  9955",
        "2 04632  11.4628 273.1101 1450506 207.6000 143.9350  1.20231981 44145",
    )
    .expect("published Vallado verification record");
    let propagator = Sgp4Propagator::try_from(&tle).expect("valid SGP4 record");
    let target = propagator.epoch() - Duration::from_seconds(5_184.0 * 60.0);
    let result = propagator
        .propagate(propagator.initial_orbit(), target)
        .expect("deep-space prediction");

    assert_eq!(result.epoch(), target);
    assert_eq!(result.as_ref().frame(), frames::ReferenceFrame::TEME);
    assert_vector_close(
        result.as_ref().position().to_metres(),
        [-29_020.025_871_28, 13_819.844_190_63, -5_713.336_791_83].map(|value| value * 1_000.0),
        POSITION_TOLERANCE_METRES,
    );
    assert_vector_close(
        result.as_ref().velocity().to_metres_per_second(),
        [-1.768_068_390, -3.235_371_192, -0.395_206_135].map(|value| value * 1_000.0),
        VELOCITY_TOLERANCE_METRES_PER_SECOND,
    );
}

#[test]
fn epoch_conversion_retains_utc_fractional_seconds() {
    let tle = TwoLineElement::parse(
        "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753",
        "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667",
    )
    .expect("published Vallado verification record");
    let propagator = Sgp4Propagator::try_from(&tle).expect("valid SGP4 record");
    assert_eq!(
        propagator.epoch(),
        Epoch::from_gregorian_utc(2000, 6, 27, 18, 50, 19, 733_568_000)
    );
}

#[test]
fn named_serde_records_preserve_model_epoch_and_teme_state() {
    let named = "0 VANGUARD 1\n\
                 1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753\n\
                 2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667";
    let tle: TwoLineElement = named.parse().expect("named Vallado record");
    let restored: TwoLineElement =
        serde_json::from_str(&serde_json::to_string(&tle).unwrap()).unwrap();
    let first = Sgp4Propagator::try_from(&tle).unwrap();
    let second = Sgp4Propagator::try_from(&restored).unwrap();
    assert_eq!(first.epoch(), second.epoch());
    assert_eq!(first.initial_orbit(), second.initial_orbit());
}

#[test]
fn calendar_adapter_uses_exact_tle_ticks_across_leap_days_and_year_end() {
    let original = "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753";
    let line_two = "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667";
    for (epoch_field, expected) in [
        (
            "00060.00000000",
            Epoch::from_gregorian_utc_at_midnight(2000, 2, 29),
        ),
        (
            "56366.99999999",
            Epoch::from_gregorian_utc(2056, 12, 31, 23, 59, 59, 999_136_000),
        ),
        (
            "57001.00000001",
            Epoch::from_gregorian_utc(1957, 1, 1, 0, 0, 0, 864_000),
        ),
        (
            "16366.00000000",
            Epoch::from_gregorian_utc_at_midnight(2016, 12, 31),
        ),
        (
            "17001.00000000",
            Epoch::from_gregorian_utc_at_midnight(2017, 1, 1),
        ),
    ] {
        let mut line = original.to_owned();
        line.replace_range(18..32, epoch_field);
        let checksum: u32 = line.as_bytes()[..68]
            .iter()
            .map(|byte| match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'-' => 1,
                _ => 0,
            })
            .sum();
        line.replace_range(68..69, &(checksum % 10).to_string());
        let tle = TwoLineElement::parse(&line, line_two).expect("valid calendar record");
        assert_eq!(Sgp4Propagator::try_from(&tle).unwrap().epoch(), expected);
    }
}

fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!((actual - expected).abs() <= tolerance);
    }
}
