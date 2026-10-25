// SPDX-License-Identifier: AGPL-3.0-only
//! JavaScript's Date range and the sentences that read a moment past the years jiff holds: toISOString, and the usage clock
//! words in the zones `Intl` takes by name or by offset. Every answer is what node 22 printed (TZ=UTC).

use wsp_words::js_date::{iso_string, time_value};
use wsp_words::usage::{clock_word, day_key_of, hour_of, resets_word};

const ISO: &[(f64, Option<&str>)] = &[
    (0.0, Some("1970-01-01T00:00:00.000Z")),
    (-1.0, Some("1969-12-31T23:59:59.999Z")),
    (1.5, Some("1970-01-01T00:00:00.001Z")),
    (-0.5, Some("1970-01-01T00:00:00.000Z")),
    (253402300799999.0, Some("9999-12-31T23:59:59.999Z")),
    (253402300800000.0, Some("+010000-01-01T00:00:00.000Z")),
    (253436860800000.0, Some("+010001-02-04T00:00:00.000Z")),
    (8640000000000000.0, Some("+275760-09-13T00:00:00.000Z")),
    (-8640000000000000.0, Some("-271821-04-20T00:00:00.000Z")),
    (8640000000000001.0, None),
    (-62198755200000.0, Some("-000001-01-01T00:00:00.000Z")),
    (-62167219200000.0, Some("0000-01-01T00:00:00.000Z")),
    (-62135596800000.0, Some("0001-01-01T00:00:00.000Z")),
    (1000000000000000.0, Some("+033658-09-27T01:46:40.000Z")),
    (10000000000000000.0, None),
    (1700000000000.0, Some("2023-11-14T22:13:20.000Z")),
    (1790812799999.0, Some("2026-09-30T23:59:59.999Z")),
    (1790812800000.0, Some("2026-10-01T00:00:00.000Z")),
    (4102444800000.0, Some("2100-01-01T00:00:00.000Z")),
    (32503680000000.0, Some("3000-01-01T00:00:00.000Z")),
    (64060588800000.0, Some("4000-01-01T00:00:00.000Z")),
    (100000000000000.0, Some("5138-11-16T09:46:40.000Z")),
    (10000000000000.0, Some("2286-11-20T17:46:40.000Z")),
];

/// (zone, instant, dayKeyOf, hourOf, resetsWord a week on, clockWord against a fixed now)
const ZONES: &[(&str, f64, &str, f64, &str, &str)] = &[
    ("UTC", 1700000000000.0, "2023-11-14", 22.0, "resets on Tue", "22:13"),
    ("UTC", 1720000000000.0, "2024-07-03", 9.0, "resets on Wed", "Wed 09:46"),
    ("UTC", 253402300799999.0, "9999-12-31", 23.0, "resets on Fri", "Fri 23:59"),
    ("UTC", 253402300800000.0, "10000-01-01", 0.0, "resets on Sat", "Sat 00:00"),
    ("UTC", 300000000000000.0, "11476-08-15", 5.0, "resets on Tue", "Tue 05:20"),
    ("UTC", 650000000000000.0, "22567-09-06", 3.0, "resets on Sun", "Sun 03:33"),
    ("UTC", 1000000000000000.0, "33658-09-27", 1.0, "resets on Fri", "Fri 01:46"),
    ("UTC", 4000000000000000.0, "128724-12-15", 7.0, "resets on Mon", "Mon 07:06"),
    ("UTC", -300000000000000.0, "7538-05-18", 18.0, "resets on Fri", "Fri 18:40"),
    ("UTC", -62000000000000.0, "5-04-19", 9.0, "resets on Tue", "Tue 09:46"),
    ("UTC", -8640000000000000.0, "271822-04-20", 0.0, "resets on Tue", "Tue 00:00"),
    ("UTC", 1790812800000.0, "2026-10-01", 0.0, "resets on Thu", "Thu 00:00"),
    ("UTC", 1793000000000.0, "2026-10-26", 7.0, "resets on Mon", "Mon 07:33"),
    ("America/New_York", 1700000000000.0, "2023-11-14", 17.0, "resets on Tue", "17:13"),
    ("America/New_York", 1720000000000.0, "2024-07-03", 5.0, "resets on Wed", "Wed 05:46"),
    ("America/New_York", 253402300799999.0, "9999-12-31", 18.0, "resets on Fri", "Fri 18:59"),
    ("America/New_York", 253402300800000.0, "9999-12-31", 19.0, "resets on Fri", "Fri 19:00"),
    ("America/New_York", 300000000000000.0, "11476-08-15", 1.0, "resets on Tue", "Tue 01:20"),
    ("America/New_York", 650000000000000.0, "22567-09-05", 23.0, "resets on Sat", "Sat 23:33"),
    ("America/New_York", 1000000000000000.0, "33658-09-26", 21.0, "resets on Thu", "Thu 21:46"),
    ("America/New_York", 4000000000000000.0, "128724-12-15", 2.0, "resets on Mon", "Mon 02:06"),
    ("America/New_York", -300000000000000.0, "7538-05-18", 13.0, "resets on Fri", "Fri 13:43"),
    ("America/New_York", -62000000000000.0, "5-04-19", 4.0, "resets on Tue", "Tue 04:50"),
    ("America/New_York", -8640000000000000.0, "271822-04-19", 19.0, "resets on Mon", "Mon 19:03"),
    ("America/New_York", 1790812800000.0, "2026-09-30", 20.0, "resets on Wed", "Wed 20:00"),
    ("America/New_York", 1793000000000.0, "2026-10-26", 3.0, "resets on Mon", "Mon 03:33"),
    ("Asia/Kolkata", 1700000000000.0, "2023-11-15", 3.0, "resets on Wed", "03:43"),
    ("Asia/Kolkata", 1720000000000.0, "2024-07-03", 15.0, "resets on Wed", "Wed 15:16"),
    ("Asia/Kolkata", 253402300799999.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:29"),
    ("Asia/Kolkata", 253402300800000.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:30"),
    ("Asia/Kolkata", 300000000000000.0, "11476-08-15", 10.0, "resets on Tue", "Tue 10:50"),
    ("Asia/Kolkata", 650000000000000.0, "22567-09-06", 9.0, "resets on Sun", "Sun 09:03"),
    ("Asia/Kolkata", 1000000000000000.0, "33658-09-27", 7.0, "resets on Fri", "Fri 07:16"),
    ("Asia/Kolkata", 4000000000000000.0, "128724-12-15", 12.0, "resets on Mon", "Mon 12:36"),
    ("Asia/Kolkata", -300000000000000.0, "7538-05-19", 0.0, "resets on Sat", "Sat 00:33"),
    ("Asia/Kolkata", -62000000000000.0, "5-04-19", 15.0, "resets on Tue", "Tue 15:40"),
    ("Asia/Kolkata", -8640000000000000.0, "271822-04-20", 5.0, "resets on Tue", "Tue 05:53"),
    ("Asia/Kolkata", 1790812800000.0, "2026-10-01", 5.0, "resets on Thu", "Thu 05:30"),
    ("Asia/Kolkata", 1793000000000.0, "2026-10-26", 13.0, "resets on Mon", "Mon 13:03"),
    ("Europe/London", 1700000000000.0, "2023-11-14", 22.0, "resets on Tue", "22:13"),
    ("Europe/London", 1720000000000.0, "2024-07-03", 10.0, "resets on Wed", "Wed 10:46"),
    ("Europe/London", 253402300799999.0, "9999-12-31", 23.0, "resets on Fri", "Fri 23:59"),
    ("Europe/London", 253402300800000.0, "10000-01-01", 0.0, "resets on Sat", "Sat 00:00"),
    ("Europe/London", 300000000000000.0, "11476-08-15", 6.0, "resets on Tue", "Tue 06:20"),
    ("Europe/London", 650000000000000.0, "22567-09-06", 4.0, "resets on Sun", "Sun 04:33"),
    ("Europe/London", 1000000000000000.0, "33658-09-27", 2.0, "resets on Fri", "Fri 02:46"),
    ("Europe/London", 4000000000000000.0, "128724-12-15", 7.0, "resets on Mon", "Mon 07:06"),
    ("Europe/London", -300000000000000.0, "7538-05-18", 18.0, "resets on Fri", "Fri 18:38"),
    ("Europe/London", -62000000000000.0, "5-04-19", 9.0, "resets on Tue", "Tue 09:45"),
    ("Europe/London", -8640000000000000.0, "271822-04-19", 23.0, "resets on Mon", "Mon 23:58"),
    ("Europe/London", 1790812800000.0, "2026-10-01", 1.0, "resets on Thu", "Thu 01:00"),
    ("Europe/London", 1793000000000.0, "2026-10-26", 7.0, "resets on Mon", "Mon 07:33"),
    ("Australia/Lord_Howe", 1700000000000.0, "2023-11-15", 9.0, "resets on Wed", "09:13"),
    ("Australia/Lord_Howe", 1720000000000.0, "2024-07-03", 20.0, "resets on Wed", "Wed 20:16"),
    ("Australia/Lord_Howe", 253402300799999.0, "10000-01-01", 10.0, "resets on Sat", "Sat 10:59"),
    ("Australia/Lord_Howe", 253402300800000.0, "10000-01-01", 11.0, "resets on Sat", "Sat 11:00"),
    ("Australia/Lord_Howe", 300000000000000.0, "11476-08-15", 15.0, "resets on Tue", "Tue 15:50"),
    ("Australia/Lord_Howe", 650000000000000.0, "22567-09-06", 14.0, "resets on Sun", "Sun 14:03"),
    ("Australia/Lord_Howe", 1000000000000000.0, "33658-09-27", 12.0, "resets on Fri", "Fri 12:16"),
    ("Australia/Lord_Howe", 4000000000000000.0, "128724-12-15", 18.0, "resets on Mon", "Mon 18:06"),
    ("Australia/Lord_Howe", -300000000000000.0, "7538-05-19", 5.0, "resets on Sat", "Sat 05:16"),
    ("Australia/Lord_Howe", -62000000000000.0, "5-04-19", 20.0, "resets on Tue", "Tue 20:23"),
    ("Australia/Lord_Howe", -8640000000000000.0, "271822-04-20", 10.0, "resets on Tue", "Tue 10:36"),
    ("Australia/Lord_Howe", 1790812800000.0, "2026-10-01", 10.0, "resets on Thu", "Thu 10:30"),
    ("Australia/Lord_Howe", 1793000000000.0, "2026-10-26", 18.0, "resets on Mon", "Mon 18:33"),
    ("+05:30", 1700000000000.0, "2023-11-15", 3.0, "resets on Wed", "03:43"),
    ("+05:30", 1720000000000.0, "2024-07-03", 15.0, "resets on Wed", "Wed 15:16"),
    ("+05:30", 253402300799999.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:29"),
    ("+05:30", 253402300800000.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:30"),
    ("+05:30", 300000000000000.0, "11476-08-15", 10.0, "resets on Tue", "Tue 10:50"),
    ("+05:30", 650000000000000.0, "22567-09-06", 9.0, "resets on Sun", "Sun 09:03"),
    ("+05:30", 1000000000000000.0, "33658-09-27", 7.0, "resets on Fri", "Fri 07:16"),
    ("+05:30", 4000000000000000.0, "128724-12-15", 12.0, "resets on Mon", "Mon 12:36"),
    ("+05:30", -300000000000000.0, "7538-05-19", 0.0, "resets on Sat", "Sat 00:10"),
    ("+05:30", -62000000000000.0, "5-04-19", 15.0, "resets on Tue", "Tue 15:16"),
    ("+05:30", -8640000000000000.0, "271822-04-20", 5.0, "resets on Tue", "Tue 05:30"),
    ("+05:30", 1790812800000.0, "2026-10-01", 5.0, "resets on Thu", "Thu 05:30"),
    ("+05:30", 1793000000000.0, "2026-10-26", 13.0, "resets on Mon", "Mon 13:03"),
    ("-08:00", 1700000000000.0, "2023-11-14", 14.0, "resets on Tue", "14:13"),
    ("-08:00", 1720000000000.0, "2024-07-03", 1.0, "resets on Wed", "Wed 01:46"),
    ("-08:00", 253402300799999.0, "9999-12-31", 15.0, "resets on Fri", "Fri 15:59"),
    ("-08:00", 253402300800000.0, "9999-12-31", 16.0, "resets on Fri", "Fri 16:00"),
    ("-08:00", 300000000000000.0, "11476-08-14", 21.0, "resets on Mon", "Mon 21:20"),
    ("-08:00", 650000000000000.0, "22567-09-05", 19.0, "resets on Sat", "Sat 19:33"),
    ("-08:00", 1000000000000000.0, "33658-09-26", 17.0, "resets on Thu", "Thu 17:46"),
    ("-08:00", 4000000000000000.0, "128724-12-14", 23.0, "resets on Sun", "Sun 23:06"),
    ("-08:00", -300000000000000.0, "7538-05-18", 10.0, "resets on Fri", "Fri 10:40"),
    ("-08:00", -62000000000000.0, "5-04-19", 1.0, "resets on Tue", "Tue 01:46"),
    ("-08:00", -8640000000000000.0, "271822-04-19", 16.0, "resets on Mon", "Mon 16:00"),
    ("-08:00", 1790812800000.0, "2026-09-30", 16.0, "resets on Wed", "Wed 16:00"),
    ("-08:00", 1793000000000.0, "2026-10-25", 23.0, "resets on Sun", "Sun 23:33"),
    ("+0530", 1700000000000.0, "2023-11-15", 3.0, "resets on Wed", "03:43"),
    ("+0530", 1720000000000.0, "2024-07-03", 15.0, "resets on Wed", "Wed 15:16"),
    ("+0530", 253402300799999.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:29"),
    ("+0530", 253402300800000.0, "10000-01-01", 5.0, "resets on Sat", "Sat 05:30"),
    ("+0530", 300000000000000.0, "11476-08-15", 10.0, "resets on Tue", "Tue 10:50"),
    ("+0530", 650000000000000.0, "22567-09-06", 9.0, "resets on Sun", "Sun 09:03"),
    ("+0530", 1000000000000000.0, "33658-09-27", 7.0, "resets on Fri", "Fri 07:16"),
    ("+0530", 4000000000000000.0, "128724-12-15", 12.0, "resets on Mon", "Mon 12:36"),
    ("+0530", -300000000000000.0, "7538-05-19", 0.0, "resets on Sat", "Sat 00:10"),
    ("+0530", -62000000000000.0, "5-04-19", 15.0, "resets on Tue", "Tue 15:16"),
    ("+0530", -8640000000000000.0, "271822-04-20", 5.0, "resets on Tue", "Tue 05:30"),
    ("+0530", 1790812800000.0, "2026-10-01", 5.0, "resets on Thu", "Thu 05:30"),
    ("+0530", 1793000000000.0, "2026-10-26", 13.0, "resets on Mon", "Mon 13:03"),
    ("-08", 1700000000000.0, "2023-11-14", 14.0, "resets on Tue", "14:13"),
    ("-08", 1720000000000.0, "2024-07-03", 1.0, "resets on Wed", "Wed 01:46"),
    ("-08", 253402300799999.0, "9999-12-31", 15.0, "resets on Fri", "Fri 15:59"),
    ("-08", 253402300800000.0, "9999-12-31", 16.0, "resets on Fri", "Fri 16:00"),
    ("-08", 300000000000000.0, "11476-08-14", 21.0, "resets on Mon", "Mon 21:20"),
    ("-08", 650000000000000.0, "22567-09-05", 19.0, "resets on Sat", "Sat 19:33"),
    ("-08", 1000000000000000.0, "33658-09-26", 17.0, "resets on Thu", "Thu 17:46"),
    ("-08", 4000000000000000.0, "128724-12-14", 23.0, "resets on Sun", "Sun 23:06"),
    ("-08", -300000000000000.0, "7538-05-18", 10.0, "resets on Fri", "Fri 10:40"),
    ("-08", -62000000000000.0, "5-04-19", 1.0, "resets on Tue", "Tue 01:46"),
    ("-08", -8640000000000000.0, "271822-04-19", 16.0, "resets on Mon", "Mon 16:00"),
    ("-08", 1790812800000.0, "2026-09-30", 16.0, "resets on Wed", "Wed 16:00"),
    ("-08", 1793000000000.0, "2026-10-25", 23.0, "resets on Sun", "Sun 23:33"),
    ("Etc/GMT+5", 1700000000000.0, "2023-11-14", 17.0, "resets on Tue", "17:13"),
    ("Etc/GMT+5", 1720000000000.0, "2024-07-03", 4.0, "resets on Wed", "Wed 04:46"),
    ("Etc/GMT+5", 253402300799999.0, "9999-12-31", 18.0, "resets on Fri", "Fri 18:59"),
    ("Etc/GMT+5", 253402300800000.0, "9999-12-31", 19.0, "resets on Fri", "Fri 19:00"),
    ("Etc/GMT+5", 300000000000000.0, "11476-08-15", 0.0, "resets on Tue", "Tue 00:20"),
    ("Etc/GMT+5", 650000000000000.0, "22567-09-05", 22.0, "resets on Sat", "Sat 22:33"),
    ("Etc/GMT+5", 1000000000000000.0, "33658-09-26", 20.0, "resets on Thu", "Thu 20:46"),
    ("Etc/GMT+5", 4000000000000000.0, "128724-12-15", 2.0, "resets on Mon", "Mon 02:06"),
    ("Etc/GMT+5", -300000000000000.0, "7538-05-18", 13.0, "resets on Fri", "Fri 13:40"),
    ("Etc/GMT+5", -62000000000000.0, "5-04-19", 4.0, "resets on Tue", "Tue 04:46"),
    ("Etc/GMT+5", -8640000000000000.0, "271822-04-19", 19.0, "resets on Mon", "Mon 19:00"),
    ("Etc/GMT+5", 1790812800000.0, "2026-09-30", 19.0, "resets on Wed", "Wed 19:00"),
    ("Etc/GMT+5", 1793000000000.0, "2026-10-26", 2.0, "resets on Mon", "Mon 02:33"),
];

#[test]
fn to_iso_string_is_what_a_date_writes_or_none_where_it_throws() {
    let wrong: Vec<String> = ISO
        .iter()
        .filter(|(ms, want)| iso_string(*ms).as_deref() != *want)
        .map(|(ms, want)| format!("{ms}: node {want:?}, here {:?}", iso_string(*ms)))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_time_value_keeps_the_whole_millisecond_toward_zero_and_refuses_what_a_date_does() {
    assert_eq!(time_value(1.9), Some(1));
    assert_eq!(time_value(-1.9), Some(-1));
    assert_eq!(time_value(-0.5), Some(0));
    assert_eq!(time_value(8.64e15), Some(8_640_000_000_000_000));
    assert_eq!(time_value(8.64e15 + 1.0), None);
    assert_eq!(time_value(f64::NAN), None);
    assert_eq!(time_value(f64::INFINITY), None);
}

#[test]
fn the_clock_words_read_every_zone_and_year_as_node_reads_them() {
    let mut wrong = Vec::new();
    for (zone, at, day, hour, resets, clock) in ZONES {
        let got = (
            day_key_of(*at, Some(zone)),
            hour_of(*at, Some(zone)),
            resets_word(*at + 7.0 * 86_400_000.0, *at, Some(zone)),
            clock_word(*at, 1.7e12, Some(zone)),
        );
        let want = (day.to_string(), *hour, resets.to_string(), clock.to_string());
        if got != want {
            wrong.push(format!("{zone} at {at}:\n  node {want:?}\n  here {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), ZONES.len(), wrong.join("\n"));
}
