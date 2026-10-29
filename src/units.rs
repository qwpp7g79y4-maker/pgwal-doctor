//! `pg_settings.unit` conversions.

/// Convert a `pg_settings` value in `unit` to bytes. `None` for a NULL
/// unit (a plain number, not a size), an unknown unit or a negative value.
pub fn to_bytes(value: i64, unit: Option<&str>) -> Option<u64> {
    let value = u64::try_from(value).ok()?;
    let factor: u64 = match unit? {
        "B" => 1,
        "kB" => 1 << 10,
        "8kB" => 8 << 10,
        "MB" => 1 << 20,
        "GB" => 1 << 30,
        "TB" => 1 << 40,
        _ => return None,
    };
    value.checked_mul(factor)
}

/// Convert a `pg_settings` time value to seconds (`ms` rounds down).
pub fn to_seconds(value: i64, unit: Option<&str>) -> Option<u64> {
    let value = u64::try_from(value).ok()?;
    match unit? {
        "ms" => Some(value / 1000),
        "s" => Some(value),
        "min" => value.checked_mul(60),
        "h" => value.checked_mul(3600),
        "d" => value.checked_mul(86_400),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_mb_and_8kb_blocks_to_bytes() {
        assert_eq!(to_bytes(1024, Some("MB")), Some(1 << 30));
        assert_eq!(to_bytes(80, Some("MB")), Some(80 << 20));
        assert_eq!(to_bytes(2048, Some("8kB")), Some(16 << 20));
        assert_eq!(to_bytes(16_777_216, Some("B")), Some(16 << 20));
        assert_eq!(to_bytes(4, Some("kB")), Some(4096));
        assert_eq!(to_bytes(1, Some("GB")), Some(1 << 30));
        assert_eq!(to_bytes(1, Some("TB")), Some(1 << 40));
    }

    #[test]
    fn null_unit_unknown_unit_and_negative_give_none() {
        assert_eq!(to_bytes(64, None), None);
        assert_eq!(to_bytes(1, Some("furlong")), None);
        assert_eq!(to_bytes(-1, Some("MB")), None);
        assert_eq!(to_bytes(i64::MAX, Some("TB")), None);
    }

    #[test]
    fn converts_time_units_to_seconds() {
        assert_eq!(to_seconds(300, Some("s")), Some(300));
        assert_eq!(to_seconds(5, Some("min")), Some(300));
        assert_eq!(to_seconds(1500, Some("ms")), Some(1));
        assert_eq!(to_seconds(300, None), None);
    }
}
