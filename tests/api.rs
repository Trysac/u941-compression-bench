//! Tests of the public API, from outside the crate.

use proptest::prelude::*;

// Its own import group: rustfmt sorts imports within a group, and the crate name varies per repository.
use compression_bench::{Error, normalize};

#[test]
fn trims_and_lowercases() {
    let cases = [
        ("  Hello World ", "hello world"),
        ("ABC", "abc"),
        ("\tx\n", "x"),
    ];
    for (input, want) in cases {
        assert_eq!(normalize(input), Ok(want.to_string()), "input {input:?}");
    }
}

#[test]
fn rejects_blank_input() {
    for input in ["", "   ", "\t\n"] {
        assert_eq!(normalize(input), Err(Error::Blank), "input {input:?}");
    }
    assert_eq!(Error::Blank.to_string(), "input is blank");
}

proptest! {
    #[test]
    fn normalizing_twice_changes_nothing(input in "\\PC*") {
        if let Ok(once) = normalize(&input) {
            prop_assert_eq!(normalize(&once), Ok(once.clone()));
        }
    }

    #[test]
    fn output_has_no_surrounding_whitespace(input in "\\PC*") {
        match normalize(&input) {
            Ok(out) => prop_assert_eq!(out.trim(), out.as_str()),
            Err(err) => prop_assert_eq!(err, Error::Blank),
        }
    }
}
