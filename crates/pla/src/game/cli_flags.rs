//! Shared command-line flag helpers: a malformed value is reported, never silently replaced.
//!
//! Play-test 2026-10-14: `--event-time-scale 20:` (a typo) silently kept the default 1, so the
//! distortion took 217 s instead of ~11 s. Every viewer/plugin flag parser goes through
//! [`value_of`] + [`parse_or_warn`] (numbers) or [`report_bad_value`] (enums): the problem is
//! printed to stderr at once and kept in a list the viewer shows once as a start-up toast.

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Mutex;

static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Value of `--flag VALUE` or `--flag=VALUE` (the last occurrence wins).
pub fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut found = None;
    for (i, a) in args.iter().enumerate() {
        if a == flag {
            if let Some(v) = args.get(i + 1) {
                found = Some(v.as_str());
            }
        } else if let Some(v) = a.strip_prefix(flag).and_then(|r| r.strip_prefix('=')) {
            found = Some(v);
        }
    }
    found
}

/// The warning text for a flag value that could not be used.
pub fn bad_value_message(flag: &str, raw: &str, expected: &str, fallback: &str) -> String {
    format!("warning: {flag}: could not parse '{raw}' as {expected}; using {fallback}")
}

/// Print the warning to stderr and remember it for the start-up toast (each text once).
pub fn report_bad_value(flag: &str, raw: &str, expected: &str, fallback: impl Display) {
    let msg = bad_value_message(flag, raw, expected, &fallback.to_string());
    let mut list = WARNINGS.lock().unwrap_or_else(|e| e.into_inner());
    if !list.contains(&msg) {
        eprintln!("{msg}");
        list.push(msg);
    }
}

/// `Some(value)` when `flag` is present and its value parses and passes `valid`; a present but
/// malformed or invalid value is reported (see [`report_bad_value`]) and gives `None`, so the
/// caller's default applies. `expected` describes the accepted values ("a number > 0").
pub fn parse_or_warn<T: FromStr>(
    args: &[String],
    flag: &str,
    expected: &str,
    fallback: impl Display,
    valid: impl Fn(&T) -> bool,
) -> Option<T> {
    let raw = value_of(args, flag)?;
    match raw.trim().parse::<T>() {
        Ok(v) if valid(&v) => Some(v),
        _ => {
            report_bad_value(flag, raw, expected, fallback);
            None
        }
    }
}

/// Every warning reported so far in this process (for the start-up toast).
pub fn warnings() -> Vec<String> {
    WARNINGS.lock().map(|l| l.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn values_with_space_or_equals() {
        assert_eq!(value_of(&a("v --area 1"), "--area"), Some("1"));
        assert_eq!(value_of(&a("v --area=2"), "--area"), Some("2"));
        assert_eq!(value_of(&a("v --areas 2"), "--area"), None);
        assert_eq!(value_of(&a("v --area"), "--area"), None);
    }

    #[test]
    fn a_typo_is_reported_and_the_default_applies() {
        let args = a("viewer --event-time-scale 20:");
        let v: Option<f32> = parse_or_warn(&args, "--event-time-scale", "a number > 0", 1, |v: &f32| *v > 0.0);
        assert_eq!(v, None);
        let w = warnings();
        assert!(
            w.contains(&"warning: --event-time-scale: could not parse '20:' as a number > 0; using 1".to_string()),
            "{w:?}"
        );
        let ok: Option<f32> = parse_or_warn(&a("v --event-time-scale 20"), "--event-time-scale", "a number", 1, |_: &f32| true);
        assert_eq!(ok, Some(20.0));
        let absent: Option<f32> = parse_or_warn(&a("v"), "--event-time-scale", "a number", 1, |_: &f32| true);
        assert_eq!(absent, None);
    }
}
