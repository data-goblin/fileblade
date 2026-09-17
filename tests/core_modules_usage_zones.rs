#[path = "common/usage_fixtures.rs"]
mod usage_fixtures;

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use serde_json::json;
use usage_fixtures::*;

fn local_date(moment: DateTime<Utc>, offset: FixedOffset) -> String {
    moment
        .with_timezone(&offset)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn local_dates_and_day_windows_follow_the_zone_the_helper_runs_in() {
    local_dates_follow_the_zone();
    the_day_filter_uses_local_midnight_across_daylight_saving();
}

fn local_dates_follow_the_zone() {
    use_zone("UTC0");
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![stub("skill-alpha", "alpha", "user")];
    fixture.transcript("s1.jsonl", &[skill_call(day, "toolu_t1", "alpha")]);
    let utc = fixture.history(&items);
    assert_eq!(utc["days"], json!([[fixture.date(day), 1, 1, 0, 0, 0]]));

    use_zone("<+14>-14");
    let east = fixture.history(&items);
    let offset = FixedOffset::east_opt(14 * 3600).expect("offset");
    let eastern = local_date(day, offset);
    assert_ne!(fixture.date(day), eastern);
    assert_eq!(east["days"], json!([[eastern, 1, 1, 0, 0, 0]]));
    assert_eq!(east["coverageStart"], json!(eastern));
    assert_eq!(east["until"], json!(local_date(Utc::now(), offset)));
    use_zone("UTC0");
}

fn the_day_filter_uses_local_midnight_across_daylight_saving() {
    use_zone("Europe/Brussels");
    let fixture = Fixture::new();
    let zone = FixedOffset::east_opt(3600).expect("winter offset");
    let start = zone
        .with_ymd_and_hms(2026, 3, 29, 0, 0, 0)
        .single()
        .expect("local midnight")
        .with_timezone(&Utc);
    let summer = FixedOffset::east_opt(2 * 3600).expect("summer offset");
    let end = summer
        .with_ymd_and_hms(2026, 3, 30, 0, 0, 0)
        .single()
        .expect("next local midnight")
        .with_timezone(&Utc);
    fixture.transcript(
        "dst.jsonl",
        &[
            skill_call(start - Duration::milliseconds(1), "before", "alpha"),
            skill_call(start, "start", "alpha"),
            skill_call(end - Duration::milliseconds(1), "last", "alpha"),
            skill_call(end, "after", "alpha"),
        ],
    );
    let items = vec![stub("skill-alpha", "alpha", "user")];
    fixture.counts(&items);
    let day = fileblade::core_modules::usage::query::skill_day(
        &fixture.environment(),
        &items,
        "2026-03-29",
    );
    let named: Vec<(String, i64)> = day["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap_or("").to_string(),
                row["uses"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    assert_eq!(named, vec![("alpha".to_string(), 2)]);
    use_zone("UTC0");
}
