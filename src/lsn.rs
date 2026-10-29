//! PostgreSQL log sequence numbers (`pg_lsn`), printed as `HI/LO` in hex.

/// Parse `0/3000060` into a byte position. `None` for anything else.
pub fn parse_lsn(text: &str) -> Option<u64> {
    let (hi, lo) = text.trim().split_once('/')?;
    if hi.is_empty() || lo.is_empty() || hi.len() > 8 || lo.len() > 8 {
        return None;
    }
    let hi = u32::from_str_radix(hi, 16).ok()?;
    let lo = u32::from_str_radix(lo, 16).ok()?;
    Some((u64::from(hi) << 32) | u64::from(lo))
}

/// Bytes between two positions; zero when `restart` is ahead of `current`.
pub fn lag_bytes(current: u64, restart: u64) -> u64 {
    current.saturating_sub(restart)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pg_lsn_hi_lo_hex() {
        assert_eq!(parse_lsn("0/0"), Some(0));
        assert_eq!(parse_lsn("0/3000060"), Some(0x300_0060));
        assert_eq!(parse_lsn("1/0"), Some(1 << 32));
        assert_eq!(parse_lsn("A/FF"), Some((0xA << 32) | 0xFF));
        assert_eq!(parse_lsn("FFFFFFFF/FFFFFFFF"), Some(u64::MAX));
        assert_eq!(parse_lsn(" 5/00000000 "), Some(5 << 32));
        assert_eq!(parse_lsn("5"), None);
        assert_eq!(parse_lsn("/5"), None);
        assert_eq!(parse_lsn("5/"), None);
        assert_eq!(parse_lsn("g/1"), None);
        assert_eq!(parse_lsn("100000000/1"), None);
    }

    #[test]
    fn lag_never_goes_negative() {
        assert_eq!(lag_bytes(10, 4), 6);
        assert_eq!(lag_bytes(4, 10), 0);
    }
}
