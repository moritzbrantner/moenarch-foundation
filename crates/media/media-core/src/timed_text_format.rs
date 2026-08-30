//! Deterministic, domain-neutral timed-text parsing and rendering.

use crate::{DetectError, Result, TimedTextContract, TimedTextSegmentContract};

/// Parses SubRip subtitle text into the neutral timed-text contract.
///
/// Cue identifiers are optional; numeric identifiers are preserved in the
/// segment index. Cue text keeps its internal line breaks and content, while
/// surrounding whitespace is removed and line endings are normalized. Every
/// cue must use `HH:MM:SS,mmm` timestamps and have a finite, non-negative,
/// non-backward time range.
pub fn parse_srt(input: &str) -> Result<TimedTextContract> {
    parse_subtitles(input, SubtitleSyntax::Srt)
}

/// Parses WebVTT subtitle text into the neutral timed-text contract.
///
/// A UTF-8 byte-order mark may precede the mandatory `WEBVTT` signature.
/// Optional cue identifiers, cue settings, and `NOTE`, `STYLE`, and `REGION`
/// metadata blocks are accepted and discarded by the canonical projection.
/// Cue starts must be nondecreasing, and every cue must span at least one
/// millisecond.
pub fn parse_webvtt(input: &str) -> Result<TimedTextContract> {
    parse_subtitles(input, SubtitleSyntax::WebVtt)
}

/// Parses non-empty plain-text lines as untimed segments.
///
/// Line endings are normalized and each line is trimmed. Empty lines do not
/// create segments.
pub fn parse_plain_text(input: &str) -> TimedTextContract {
    let normalized = normalize_line_endings(input);
    let segments = normalized
        .strip_prefix('\u{feff}')
        .unwrap_or(&normalized)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let text = line.trim();
            (!text.is_empty()).then(|| TimedTextSegmentContract::new(index as u64, text))
        })
        .collect::<Vec<_>>();
    contract_with_aggregate_text(segments)
}

/// Renders canonical SubRip bytes from timed segments.
///
/// Every segment must contain a finite, non-negative start and end time. The
/// output uses one-based cue numbers and `HH:MM:SS,mmm` timestamps.
pub fn format_srt(contract: &TimedTextContract) -> Result<String> {
    format_subtitles(contract, SubtitleSyntax::Srt)
}

/// Renders canonical WebVTT bytes from timed segments.
///
/// Every segment must contain a finite, non-negative start and end time. The
/// output starts with `WEBVTT` and omits the hour field below one hour.
pub fn format_webvtt(contract: &TimedTextContract) -> Result<String> {
    format_subtitles(contract, SubtitleSyntax::WebVtt)
}

/// Renders trimmed, non-empty segment text with one segment per line.
///
/// Non-empty output always ends with a newline.
pub fn format_plain_text(contract: &TimedTextContract) -> String {
    let text = contract
        .segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    trailing_newline(text)
}

/// Renders a tab-separated timed-text table.
///
/// Every segment must contain a finite, non-negative, non-backward time range.
/// Times are rounded to integer milliseconds. Tabs and line endings in cue text
/// become spaces so each segment occupies one row.
pub fn format_tsv(contract: &TimedTextContract) -> Result<String> {
    contract.validate()?;
    let mut output = String::from("start\tend\ttext\n");
    for (index, segment) in contract.segments.iter().enumerate() {
        let (start, end) = complete_non_negative_range(segment, index + 1)?;
        let start = rounded_milliseconds(start)?;
        let end = rounded_milliseconds(end)?;
        let text = sanitize_single_line(&segment.text);
        output.push_str(&format!("{start}\t{end}\t{text}\n"));
    }
    Ok(output)
}

/// Renders Audacity label-track rows without product-specific speaker syntax.
///
/// Every segment must contain a finite, non-negative, non-backward time range.
/// Tabs and line endings in label text become spaces. Speaker fields are
/// intentionally not projected because speaker decoration is product policy
/// rather than part of the Audacity label format.
pub fn format_audacity_labels(contract: &TimedTextContract) -> Result<String> {
    contract.validate()?;
    let mut output = String::new();
    for (index, segment) in contract.segments.iter().enumerate() {
        let (start, end) = complete_non_negative_range(segment, index + 1)?;
        let text = sanitize_single_line(&segment.text);
        output.push_str(&format!("{start}\t{end}\t{text}\n"));
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum SubtitleSyntax {
    Srt,
    WebVtt,
}

impl SubtitleSyntax {
    fn decimal_marker(self) -> char {
        match self {
            Self::Srt => ',',
            Self::WebVtt => '.',
        }
    }

    fn allows_minute_only_timestamp(self) -> bool {
        matches!(self, Self::WebVtt)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Srt => "SRT",
            Self::WebVtt => "WebVTT",
        }
    }

    fn accepts_hour_width(self, width: usize) -> bool {
        match self {
            Self::Srt => width == 2,
            Self::WebVtt => width >= 2,
        }
    }
}

fn parse_subtitles(input: &str, syntax: SubtitleSyntax) -> Result<TimedTextContract> {
    let normalized = normalize_line_endings(input);
    let without_bom = normalized.strip_prefix('\u{feff}').unwrap_or(&normalized);
    let body = subtitle_body(without_bom, syntax)?;
    let blocks = subtitle_blocks(body);
    let mut segments = Vec::with_capacity(blocks.len());
    let mut saw_cue = false;
    let mut previous_webvtt_start = None;

    for block in &blocks {
        if matches!(syntax, SubtitleSyntax::WebVtt) && is_webvtt_metadata_block(block, saw_cue)? {
            continue;
        }
        saw_cue = true;
        let cue_number = segments.len() + 1;
        let timing_index = block
            .iter()
            .position(|line| line.contains("-->"))
            .ok_or_else(|| {
                invalid_error(format!("subtitle cue {cue_number} is missing a time range"))
            })?;
        if timing_index > 1 {
            return invalid(format!(
                "subtitle cue {} has more than one line before its time range",
                cue_number
            ));
        }

        let (start, end) = parse_timing_line(block[timing_index], syntax, cue_number)?;
        if matches!(syntax, SubtitleSyntax::WebVtt) {
            let start_millis = rounded_milliseconds(start)?;
            if previous_webvtt_start.is_some_and(|previous| start_millis < previous) {
                return invalid(format!(
                    "WebVTT cue {cue_number} starts before the preceding cue"
                ));
            }
            previous_webvtt_start = Some(start_millis);
        }
        let text = block[(timing_index + 1)..].join("\n");
        let text = text.trim();
        if text.is_empty() {
            return invalid(format!("subtitle cue {cue_number} has no text"));
        }
        if matches!(syntax, SubtitleSyntax::WebVtt) && text.contains("-->") {
            return invalid(format!(
                "WebVTT cue {cue_number} payload must not contain -->"
            ));
        }

        let index = if matches!(syntax, SubtitleSyntax::Srt) && timing_index == 1 {
            block[0]
                .trim()
                .parse::<u64>()
                .unwrap_or((cue_number - 1) as u64)
        } else {
            (cue_number - 1) as u64
        };
        let segment =
            TimedTextSegmentContract::new(index, text).with_time_range(Some(start), Some(end))?;
        segments.push(segment);
    }

    let contract = contract_with_aggregate_text(segments);
    contract.validate()?;
    Ok(contract)
}

fn subtitle_body(input: &str, syntax: SubtitleSyntax) -> Result<&str> {
    let (first_line, body) = input.split_once('\n').unwrap_or((input, ""));
    match syntax {
        SubtitleSyntax::Srt if is_webvtt_header(first_line) => {
            invalid("an SRT document must not contain a WEBVTT signature")
        }
        SubtitleSyntax::Srt => Ok(input),
        SubtitleSyntax::WebVtt if is_webvtt_header(first_line) => Ok(body),
        SubtitleSyntax::WebVtt => invalid("a WebVTT document must begin with a WEBVTT signature"),
    }
}

fn is_webvtt_header(line: &str) -> bool {
    line == "WEBVTT"
        || line
            .strip_prefix("WEBVTT")
            .is_some_and(|suffix| suffix.starts_with([' ', '\t']) && !suffix.contains("-->"))
}

fn is_webvtt_metadata_block(block: &[&str], saw_cue: bool) -> Result<bool> {
    let Some(first) = block.first().map(|line| line.trim_end()) else {
        return Ok(false);
    };
    let is_note = first == "NOTE"
        || first
            .strip_prefix("NOTE")
            .is_some_and(|suffix| suffix.starts_with([' ', '\t']));
    if is_note {
        if block.iter().any(|line| line.contains("-->")) {
            return invalid("WebVTT NOTE block is malformed");
        }
        return Ok(true);
    }
    let is_header_block = matches!(first, "STYLE" | "REGION");
    if !is_header_block {
        return Ok(false);
    }
    if saw_cue {
        return invalid(format!("WebVTT {first} blocks must precede all cues"));
    }
    if block.len() < 2 || block[1..].iter().any(|line| line.contains("-->")) {
        return invalid(format!("WebVTT {first} block is malformed"));
    }
    Ok(true)
}

fn subtitle_blocks(input: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn parse_timing_line(line: &str, syntax: SubtitleSyntax, cue_number: usize) -> Result<(f64, f64)> {
    let (start_text, end_and_settings) = line
        .split_once("-->")
        .ok_or_else(|| invalid_error(format!("subtitle cue {cue_number} has a malformed range")))?;
    if end_and_settings.contains("-->") {
        return invalid(format!(
            "subtitle cue {cue_number} has a malformed time range"
        ));
    }
    let start_text = start_text.trim();
    if start_text.split_whitespace().count() != 1 {
        return invalid(format!(
            "subtitle cue {cue_number} has a malformed start timestamp"
        ));
    }
    let mut end_tokens = end_and_settings.split_whitespace();
    let end_text = end_tokens.next().ok_or_else(|| {
        invalid_error(format!(
            "subtitle cue {cue_number} is missing an end timestamp"
        ))
    })?;
    if matches!(syntax, SubtitleSyntax::Srt) && end_tokens.next().is_some() {
        return invalid(format!(
            "subtitle cue {cue_number} has unexpected SRT cue settings"
        ));
    }

    let start = parse_timestamp(start_text, syntax, cue_number)?;
    let end = parse_timestamp(end_text, syntax, cue_number)?;
    if matches!(syntax, SubtitleSyntax::WebVtt) && end <= start {
        return invalid(format!("WebVTT cue {cue_number} must end after it starts"));
    }
    if end < start {
        return invalid(format!("subtitle cue {cue_number} ends before it starts"));
    }
    Ok((start, end))
}

fn parse_timestamp(timestamp: &str, syntax: SubtitleSyntax, cue_number: usize) -> Result<f64> {
    if timestamp.starts_with('-') {
        return invalid(format!(
            "subtitle cue {cue_number} has a negative timestamp"
        ));
    }
    let parts = timestamp.split(':').collect::<Vec<_>>();
    let (hours, minutes_text, seconds_text) = match parts.as_slice() {
        [minutes, seconds] if syntax.allows_minute_only_timestamp() => (0_u64, *minutes, *seconds),
        [hours, minutes, seconds] if syntax.accepts_hour_width(hours.len()) => {
            (parse_digits(hours, cue_number)?, *minutes, *seconds)
        }
        _ => {
            return invalid(format!(
                "subtitle cue {cue_number} has a malformed {} timestamp",
                syntax.label()
            ))
        }
    };
    let minutes = parse_fixed_digits(minutes_text, 2, cue_number)?;
    if minutes >= 60 {
        return invalid(format!(
            "subtitle cue {cue_number} has a timestamp minute outside 0..60"
        ));
    }

    let Some((seconds_text, fraction_text)) = seconds_text.split_once(syntax.decimal_marker())
    else {
        return invalid(format!(
            "subtitle cue {cue_number} has a malformed {} timestamp",
            syntax.label()
        ));
    };
    let seconds = parse_fixed_digits(seconds_text, 2, cue_number)?;
    let milliseconds = parse_fixed_digits(fraction_text, 3, cue_number)?;
    if seconds >= 60 {
        return invalid(format!(
            "subtitle cue {cue_number} has a timestamp second outside 0..60"
        ));
    }
    let total_seconds = hours
        .checked_mul(3_600)
        .and_then(|value| value.checked_add(minutes * 60))
        .and_then(|value| value.checked_add(seconds))
        .ok_or_else(|| {
            invalid_error(format!("subtitle cue {cue_number} timestamp is too large"))
        })?;
    let total_milliseconds = total_seconds
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(milliseconds))
        .ok_or_else(|| {
            invalid_error(format!("subtitle cue {cue_number} timestamp is too large"))
        })?;
    Ok(total_milliseconds as f64 / 1_000.0)
}

fn parse_fixed_digits(value: &str, width: usize, cue_number: usize) -> Result<u64> {
    if value.len() != width {
        return invalid(format!(
            "subtitle cue {cue_number} timestamp fields must use {width} digits"
        ));
    }
    parse_digits(value, cue_number)
}

fn parse_digits(value: &str, cue_number: usize) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid(format!(
            "subtitle cue {cue_number} has a malformed timestamp"
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| invalid_error(format!("subtitle cue {cue_number} timestamp is too large")))
}

fn format_subtitles(contract: &TimedTextContract, syntax: SubtitleSyntax) -> Result<String> {
    contract.validate()?;
    let mut output = match syntax {
        SubtitleSyntax::Srt => String::new(),
        SubtitleSyntax::WebVtt => String::from("WEBVTT\n\n"),
    };
    let mut previous_webvtt_start = None;
    for (index, segment) in contract.segments.iter().enumerate() {
        let (start, end) = complete_non_negative_range(segment, index + 1)?;
        let start_millis = rounded_milliseconds(start)?;
        let end_millis = rounded_milliseconds(end)?;
        if matches!(syntax, SubtitleSyntax::Srt)
            && (start_millis >= 360_000_000 || end_millis >= 360_000_000)
        {
            return invalid(format!(
                "SRT cue {} timestamps must fit a two-digit hour field",
                index + 1
            ));
        }
        if matches!(syntax, SubtitleSyntax::WebVtt) {
            if end_millis <= start_millis {
                return invalid(format!(
                    "WebVTT cue {} must span at least one millisecond after rounding",
                    index + 1
                ));
            }
            if previous_webvtt_start.is_some_and(|previous| start_millis < previous) {
                return invalid(format!(
                    "WebVTT cue {} starts before the preceding cue",
                    index + 1
                ));
            }
            previous_webvtt_start = Some(start_millis);
        }
        let normalized_text = normalize_line_endings(&segment.text);
        let text = normalized_text.trim();
        if text.is_empty() {
            return invalid(format!("subtitle cue {} has no text", index + 1));
        }
        if matches!(syntax, SubtitleSyntax::WebVtt) && text.contains("-->") {
            return invalid(format!(
                "WebVTT cue {} payload must not contain -->",
                index + 1
            ));
        }
        if matches!(syntax, SubtitleSyntax::Srt) {
            output.push_str(&(index + 1).to_string());
            output.push('\n');
        }
        output.push_str(&format_timestamp(start_millis, syntax));
        output.push_str(" --> ");
        output.push_str(&format_timestamp(end_millis, syntax));
        output.push('\n');
        output.push_str(text);
        output.push_str("\n\n");
    }
    Ok(output)
}

fn complete_non_negative_range(
    segment: &TimedTextSegmentContract,
    cue_number: usize,
) -> Result<(f64, f64)> {
    let (Some(start), Some(end)) = (segment.start_seconds(), segment.end_seconds()) else {
        return invalid(format!(
            "subtitle cue {cue_number} requires both start and end timestamps"
        ));
    };
    if !start.is_finite() || !end.is_finite() {
        return invalid(format!(
            "subtitle cue {cue_number} timestamps must be finite"
        ));
    }
    if start < 0.0 || end < 0.0 {
        return invalid(format!(
            "subtitle cue {cue_number} timestamps must be non-negative"
        ));
    }
    if end < start {
        return invalid(format!("subtitle cue {cue_number} ends before it starts"));
    }
    Ok((start, end))
}

fn format_timestamp(milliseconds: u64, syntax: SubtitleSyntax) -> String {
    let hours = milliseconds / 3_600_000;
    let minutes = (milliseconds / 60_000) % 60;
    let seconds = (milliseconds / 1_000) % 60;
    let millis = milliseconds % 1_000;
    match syntax {
        SubtitleSyntax::Srt => format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}"),
        SubtitleSyntax::WebVtt if hours == 0 => {
            format!("{minutes:02}:{seconds:02}.{millis:03}")
        }
        SubtitleSyntax::WebVtt => {
            format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
        }
    }
}

fn rounded_milliseconds(seconds: f64) -> Result<u64> {
    let milliseconds = (seconds * 1_000.0).round();
    if milliseconds >= u64::MAX as f64 {
        return invalid("subtitle timestamp is too large to render");
    }
    Ok(milliseconds as u64)
}

fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn sanitize_single_line(text: &str) -> String {
    text.trim()
        .chars()
        .map(|character| match character {
            '\t' | '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

fn contract_with_aggregate_text(segments: Vec<TimedTextSegmentContract>) -> TimedTextContract {
    let text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut contract = TimedTextContract::new(segments);
    contract.text = Some(text);
    contract
}

fn trailing_newline(text: String) -> String {
    if text.is_empty() {
        text
    } else {
        format!("{text}\n")
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}
