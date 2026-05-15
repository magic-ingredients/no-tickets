//! Path-segment percent-encoding for safe URL interpolation.
//!
//! Encodes RFC 3986 pchar-minus-percent: the canonical
//! `domain.entity.action.vN` event-type ids and interaction ids pass
//! through unchanged (all chars in the set are pchar-unreserved or
//! sub-delim), but pathological inputs (`/`, `?`, `#`, etc.) are
//! percent-escaped before they hit the URL — closing the path-
//! traversal and URL-structure-smuggling vectors.

use percent_encoding::{utf8_percent_encode, AsciiSet, PercentEncode, CONTROLS};

/// Path-segment encode set: control bytes plus the small fixed set of
/// punctuation that would otherwise change URL structure if a
/// caller-supplied id contained them. Canonical type ids
/// (`ai.task.completed.v1`) consist of pchar-unreserved characters
/// only, so they pass through verbatim.
///
/// The `%` byte itself is in the set so a pre-encoded id like
/// `foo%20bar` can't double-decode on the server.
pub const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%');

/// Percent-encode a path segment for safe interpolation into a URL.
/// Returns the lazy `PercentEncode` iterator from `percent-encoding`
/// — usable directly in `format!` via its `Display` impl, no heap
/// allocation for the canonical (unchanged) case.
pub fn encode_path_segment(segment: &str) -> PercentEncode<'_> {
    utf8_percent_encode(segment, PATH_SEGMENT)
}

#[cfg(test)]
mod tests {
    use super::encode_path_segment;

    #[test]
    fn canonical_id_passes_through_unchanged() {
        assert_eq!(
            encode_path_segment("ai.task.completed.v1").to_string(),
            "ai.task.completed.v1",
        );
    }

    #[test]
    fn slash_question_hash_get_encoded() {
        assert_eq!(
            encode_path_segment("weird/id?with#chars").to_string(),
            "weird%2Fid%3Fwith%23chars",
        );
    }

    #[test]
    fn percent_itself_is_encoded() {
        // Defence against double-decoding: a pre-encoded `%20` from
        // the caller becomes `%2520` on the wire so the server's
        // decoder doesn't collapse it to a literal space.
        assert_eq!(
            encode_path_segment("foo%20bar").to_string(),
            "foo%2520bar",
        );
    }

    #[test]
    fn empty_segment_yields_empty_string() {
        assert_eq!(encode_path_segment("").to_string(), "");
    }

    #[test]
    fn control_chars_get_encoded() {
        assert_eq!(encode_path_segment("a\nb").to_string(), "a%0Ab");
    }

    /// Adversarial review #5: explicit membership pin for every
    /// character in `PATH_SEGMENT.add(...)` beyond CONTROLS. A
    /// regression that drops one of the `.add(...)` lines (say,
    /// `.add(b'<')`) would leave that character un-encoded — the
    /// existing `slash_question_hash_get_encoded` test only covers
    /// three of the ten, so a `<` regression would slip through.
    /// This test pins each entry individually.
    #[test]
    fn every_extra_punctuation_byte_in_the_set_gets_encoded() {
        let cases = [
            (' ', "%20"),
            ('"', "%22"),
            ('#', "%23"),
            ('<', "%3C"),
            ('>', "%3E"),
            ('?', "%3F"),
            ('`', "%60"),
            ('{', "%7B"),
            ('}', "%7D"),
            ('/', "%2F"),
            ('%', "%25"),
        ];
        for (raw, encoded) in cases {
            let input = format!("a{raw}b");
            let expected = format!("a{encoded}b");
            assert_eq!(
                encode_path_segment(&input).to_string(),
                expected,
                "byte {raw:?} must encode to {encoded:?}",
            );
        }
    }
}
