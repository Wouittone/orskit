use hifitime::{Epoch, TimeScale, Unit};

#[test]
fn iers_2015_and_2016_leap_boundaries_map_to_tai() {
    // IERS Bulletin C 49 introduced the final UTC second of 2015-06-30.
    let before_2015 = Epoch::from_gregorian_utc_hms(2015, 6, 30, 23, 59, 59);
    let after_2015 = Epoch::from_gregorian_utc_at_midnight(2015, 7, 1);
    assert_eq!(before_2015.to_gregorian_tai(), (2015, 7, 1, 0, 0, 34, 0));
    assert_eq!(after_2015.to_gregorian_tai(), (2015, 7, 1, 0, 0, 36, 0));

    // IERS Bulletin C 52 introduced the final UTC second of 2016-12-31.
    let before_2017 = Epoch::from_gregorian_utc_hms(2016, 12, 31, 23, 59, 59);
    let after_2017 = Epoch::from_gregorian_utc_at_midnight(2017, 1, 1);
    assert_eq!(before_2017.to_gregorian_tai(), (2017, 1, 1, 0, 0, 35, 0));
    assert_eq!(after_2017.to_gregorian_tai(), (2017, 1, 1, 0, 0, 37, 0));
}

#[test]
fn elapsed_time_counts_the_inserted_utc_second() {
    let before = Epoch::from_gregorian_utc_hms(2016, 12, 31, 23, 59, 59);
    let after = Epoch::from_gregorian_utc_at_midnight(2017, 1, 1);

    // UTC is civil rather than uniform. Convert both endpoints to TAI before
    // interpreting their difference as elapsed physical time.
    assert_eq!(
        after.to_time_scale(TimeScale::TAI) - before.to_time_scale(TimeScale::TAI),
        2 * Unit::Second
    );
}

#[test]
fn hifitime_4_3_does_not_retain_utc_60_as_a_distinct_instant() {
    let before = Epoch::from_gregorian_utc_hms(2016, 12, 31, 23, 59, 59);
    let labeled_leap = Epoch::from_gregorian_utc_hms(2016, 12, 31, 23, 59, 60);

    // Hifitime 4.3 aliases the civil :60 label to :59. Keep this explicit
    // until the public time dependency can represent the inserted UTC second.
    assert_eq!(
        labeled_leap.to_time_scale(TimeScale::TAI),
        before.to_time_scale(TimeScale::TAI)
    );
}

#[test]
fn one_instant_round_trips_through_selected_scales() {
    let instant = Epoch::from_gregorian_utc(2017, 1, 1, 0, 0, 0, 123_456_789);

    for scale in [
        TimeScale::TAI,
        TimeScale::TT,
        TimeScale::TDB,
        TimeScale::ET,
        TimeScale::GPST,
        TimeScale::GST,
        TimeScale::BDT,
    ] {
        assert_eq!(
            instant.to_time_scale(scale).to_time_scale(TimeScale::UTC),
            instant
        );
    }
}
