//! WAL segment file names.
//!
//! A segment name is 24 hex digits: timeline, log and segment, 8 digits each.
//! That layout is an **assumption** (see README); this module is the only
//! place that encodes it.

/// True for a plain WAL segment name. History, partial, backup and
/// temporary files are not segments.
pub fn is_segment_name(name: &str) -> bool {
    name.len() == 24 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_24_hex_segment_and_rejects_history_partial_backup() {
        assert!(is_segment_name("000000010000000000000001"));
        assert!(is_segment_name("00000001000000AB000000FF"));
        assert!(!is_segment_name("00000002.history"));
        assert!(!is_segment_name("000000010000000000000001.partial"));
        assert!(!is_segment_name("000000010000000000000002.00000028.backup"));
        assert!(!is_segment_name("000000010000000000000001.tmp"));
        assert!(!is_segment_name("00000001000000000000000G"));
        assert!(!is_segment_name(""));
    }
}
