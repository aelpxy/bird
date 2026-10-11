// podman prefixes each log entry with an RFC 3339 time with nanoseconds and the host's offset,
// like `2026-10-11T08:59:55.174657000+06:00 `; birdd passes it on as unix milliseconds

pub(crate) fn split_timestamp(entry: &[u8]) -> (Option<i64>, &[u8]) {
    let Some(space) = entry.iter().position(|&byte| byte == b' ') else {
        return (None, entry);
    };
    let (prefix, rest) = entry.split_at(space);
    match std::str::from_utf8(prefix).ok().and_then(unix_millis) {
        Some(at) => (Some(at), rest.get(1..).unwrap_or_default()),
        None => (None, entry),
    }
}

fn unix_millis(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let mut date_parts = date.splitn(3, '-').map(str::parse::<i64>);
    let (year, month, day) = (
        date_parts.next()?.ok()?,
        date_parts.next()?.ok()?,
        date_parts.next()?.ok()?,
    );
    let (clock, offset_secs) = if let Some(clock) = time.strip_suffix('Z') {
        (clock, 0)
    } else {
        let split = time.rfind(['+', '-'])?;
        let (clock, offset) = time.split_at(split);
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let (hours, minutes) = offset.get(1..)?.split_once(':')?;
        (
            clock,
            sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60),
        )
    };
    let (whole, fraction) = clock.split_once('.').unwrap_or((clock, ""));
    let mut clock_parts = whole.splitn(3, ':').map(str::parse::<i64>);
    let (hour, minute, second) = (
        clock_parts.next()?.ok()?,
        clock_parts.next()?.ok()?,
        clock_parts.next()?.ok()?,
    );
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..=60).contains(&second)
        && fraction.bytes().all(|byte| byte.is_ascii_digit());
    if !valid {
        return None;
    }
    let millis: i64 = format!("{fraction:0<3}").get(..3)?.parse().ok()?;
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second
        - offset_secs;
    Some(seconds * 1000 + millis)
}

// days since 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's algorithm)
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_podmans_timestamps() {
        // 2026-10-11T02:59:55.174Z
        let at = 1_791_687_595_174;
        assert_eq!(unix_millis("2026-10-11T08:59:55.174657000+06:00"), Some(at));
        assert_eq!(unix_millis("2026-10-11T02:59:55.174657000Z"), Some(at));
        assert_eq!(unix_millis("2026-10-10T21:59:55.1749-05:00"), Some(at));
        assert_eq!(unix_millis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            unix_millis("2024-02-29T12:00:00.5Z"),
            Some(1_709_208_000_500)
        );
    }

    #[test]
    fn rejects_what_is_not_a_timestamp() {
        for text in [
            "hello",
            "2026-13-01T00:00:00Z",
            "2026-10-11T25:00:00Z",
            "2026-10-11",
            "2026-10-11T08:59:55.1x2Z",
        ] {
            assert_eq!(unix_millis(text), None, "{text}");
        }
    }

    #[test]
    fn splits_the_prefix_off_an_entry() {
        assert_eq!(
            split_timestamp(b"1970-01-01T00:00:01Z hello world\n"),
            (Some(1000), b"hello world\n".as_slice())
        );
        assert_eq!(
            split_timestamp(b"no time here\n"),
            (None, b"no time here\n".as_slice())
        );
        assert_eq!(split_timestamp(b"single"), (None, b"single".as_slice()));
    }
}
