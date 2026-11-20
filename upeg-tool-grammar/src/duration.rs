//! Fixed-unit duration grammar shared by timer literals and schedule conditions.

use std::{fmt, iter::Peekable, str::CharIndices, time::Duration};

const MONTH_SECONDS: u64 = 2_630_016; // 30.44 days, independent of a calendar.
const YEAR_SECONDS: u64 = 31_557_600; // 365.25 days.
type Characters<'a> = Peekable<CharIndices<'a>>;

/// A malformed number/unit or a duration that cannot be represented exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DurationParseError {
    Empty,
    NumberExpected(usize),
    InvalidCharacter(usize),
    UnknownUnit { unit: String, value: u64 },
    NumberOverflow,
}

impl fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("value was empty"),
            Self::NumberExpected(offset) => write!(f, "expected number at {offset}"),
            Self::InvalidCharacter(offset) => write!(f, "invalid character at {offset}"),
            Self::UnknownUnit { unit, value } if unit.is_empty() => {
                write!(f, "time unit needed, for example {value}sec or {value}ms")
            }
            Self::UnknownUnit { unit, .. } => write!(
                f,
                "unknown time unit \"{unit}\", supported units: ns, us/µs, ms, sec, min, hours, days, weeks, months, years (and few variations)"
            ),
            Self::NumberOverflow => f.write_str(
                "number is too large or cannot be represented without a lack of precision (values below 1ns are not supported)",
            ),
        }
    }
}

impl std::error::Error for DurationParseError {}

#[derive(Clone, Copy)]
enum Scale {
    Nanoseconds(u64),
    Seconds(u64),
}

impl Scale {
    fn duration(self, value: u64, divisor: u64) -> Result<Duration, DurationParseError> {
        let factor = match self {
            Self::Nanoseconds(factor) | Self::Seconds(factor) => factor,
        };
        let scaled = value
            .checked_mul(factor)
            .filter(|value| value % divisor == 0)
            .ok_or(DurationParseError::NumberOverflow)?
            / divisor;
        Ok(match self {
            Self::Nanoseconds(_) => Duration::from_nanos(scaled),
            Self::Seconds(_) => Duration::from_secs(scaled),
        })
    }
}

fn unit(suffix: &str) -> Option<(Scale, Option<Scale>)> {
    use Scale::{Nanoseconds as Ns, Seconds as Sec};
    Some(match suffix {
        "ns" | "nsec" | "nanos" => (Ns(1), None),
        "us" | "usec" | "µs" => (Ns(1_000), Some(Ns(1_000))),
        "ms" | "msec" | "millis" => (Ns(1_000_000), Some(Ns(1_000_000))),
        "s" | "sec" | "secs" | "second" | "seconds" => (Sec(1), Some(Ns(1_000_000_000))),
        "m" | "min" | "mins" | "minute" | "minutes" => (Sec(60), Some(Ns(60_000_000_000))),
        "h" | "hr" | "hrs" | "hour" | "hours" => (Sec(3_600), Some(Sec(3_600))),
        "d" | "day" | "days" => (Sec(86_400), Some(Sec(86_400))),
        "w" | "wk" | "wks" | "week" | "weeks" => (Sec(604_800), Some(Sec(604_800))),
        "M" | "month" | "months" => (Sec(MONTH_SECONDS), Some(Sec(MONTH_SECONDS))),
        "y" | "yr" | "yrs" | "year" | "years" => (Sec(YEAR_SECONDS), Some(Sec(YEAR_SECONDS))),
        _ => return None,
    })
}

fn offset(characters: &mut Characters<'_>, end: usize) -> usize {
    characters.peek().map_or(end, |(offset, _)| *offset)
}

fn digits(characters: &mut Characters<'_>) -> Result<Option<(u64, u32)>, DurationParseError> {
    let mut value = 0_u64;
    let mut count = 0_u32;
    while let Some((_, character)) = characters.peek().copied() {
        if character.is_ascii_digit() {
            let digit = u64::from(u32::from(character) - u32::from('0'));
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or(DurationParseError::NumberOverflow)?;
            count = count
                .checked_add(1)
                .ok_or(DurationParseError::NumberOverflow)?;
        } else if !character.is_whitespace() {
            break;
        }
        characters.next();
    }
    Ok((count != 0).then_some((value, count)))
}

/// Parse concatenated or whitespace-separated fixed-unit durations.
///
/// Aliases are case-sensitive; `M` is a 30.44-day month and `m` is a minute.
/// Whitespace may separate digits as well as units. Fractions must be exact:
/// subsecond/second/minute fractions resolve to nanoseconds, while fractions
/// of hours and larger units resolve to whole seconds. Overflow never saturates.
pub fn parse_duration(input: &str) -> Result<Duration, DurationParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(DurationParseError::Empty);
    }
    if input == "0" {
        return Ok(Duration::ZERO);
    }
    let mut characters = input.char_indices().peekable();
    let mut total = Duration::ZERO;
    while characters.peek().is_some() {
        let start = offset(&mut characters, input.len());
        let (integer, _) =
            digits(&mut characters)?.ok_or(DurationParseError::NumberExpected(start))?;
        let fraction = if characters
            .peek()
            .is_some_and(|(_, character)| *character == '.')
        {
            let decimal_offset = offset(&mut characters, input.len());
            characters.next();
            let fraction_start = offset(&mut characters, input.len());
            let parsed = digits(&mut characters)?;
            let fraction_end = offset(&mut characters, input.len());
            let missing_digit_offset = if fraction_start == fraction_end {
                decimal_offset
            } else {
                fraction_end
            };
            let (value, count) =
                parsed.ok_or(DurationParseError::InvalidCharacter(missing_digit_offset))?;
            let divisor = 10_u64
                .checked_pow(count)
                .ok_or(DurationParseError::NumberOverflow)?;
            Some((value, divisor))
        } else {
            None
        };
        let unit_start = offset(&mut characters, input.len());
        let mut unit_end = unit_start;
        while let Some((index, character)) = characters.peek().copied() {
            if character.is_ascii_alphabetic() || character == 'µ' {
                unit_end = index + character.len_utf8();
                characters.next();
            } else if character.is_ascii_digit() {
                break;
            } else if character.is_whitespace() {
                characters.next();
                break;
            } else {
                return Err(DurationParseError::InvalidCharacter(index));
            }
        }
        let suffix = &input[unit_start..unit_end];
        let (integer_scale, fraction_scale) =
            unit(suffix).ok_or_else(|| DurationParseError::UnknownUnit {
                unit: suffix.to_owned(),
                value: integer,
            })?;
        let integer = integer_scale.duration(integer, 1)?;
        total = total
            .checked_add(integer)
            .ok_or(DurationParseError::NumberOverflow)?;
        if let Some((value, divisor)) = fraction {
            let fractional = fraction_scale
                .ok_or(DurationParseError::NumberOverflow)?
                .duration(value, divisor)?;
            total = total
                .checked_add(fractional)
                .ok_or(DurationParseError::NumberOverflow)?;
        }
    }
    Ok(total)
}

/// A duration displayed with the same fixed units accepted by the parser.
#[derive(Clone, Copy, Debug)]
pub struct FormattedDuration(Duration);

/// Format without allocation, using compound units such as `1s 500ms`.
pub fn format_duration(duration: Duration) -> FormattedDuration {
    FormattedDuration(duration)
}

fn part(
    f: &mut fmt::Formatter<'_>,
    started: &mut bool,
    value: u64,
    unit: &str,
    plural: bool,
) -> fmt::Result {
    if value != 0 {
        if *started {
            f.write_str(" ")?;
        }
        write!(f, "{value}{unit}")?;
        if plural && value != 1 {
            f.write_str("s")?;
        }
        *started = true;
    }
    Ok(())
}

impl fmt::Display for FormattedDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str("0s");
        }
        let mut started = false;
        let mut seconds = self.0.as_secs();
        for (label, divisor, plural) in [
            ("year", YEAR_SECONDS, true),
            ("month", MONTH_SECONDS, true),
            ("day", 86_400, true),
            ("h", 3_600, false),
            ("m", 60, false),
            ("s", 1, false),
        ] {
            part(f, &mut started, seconds / divisor, label, plural)?;
            seconds %= divisor;
        }
        let mut nanoseconds = u64::from(self.0.subsec_nanos());
        for (label, divisor) in [("ms", 1_000_000), ("us", 1_000), ("ns", 1)] {
            part(f, &mut started, nanoseconds / divisor, label, false)?;
            nanoseconds %= divisor;
        }
        Ok(())
    }
}
