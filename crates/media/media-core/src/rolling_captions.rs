//! Source-mapped projection of rolling (roll-up) captions into spoken words.
//!
//! Roll-up captions, such as YouTube automatic captions, redisplay the
//! previous line with each new cue and add short "refresh" cues that only
//! repeat text already shown. Reading their cues as speech duplicates every
//! line. [`project_rolling_captions`] keeps the raw cues untouched and returns
//! each spoken word once, with its timing and the cue that introduced it.
//!
//! A cue's leading lines are treated as a redisplay only when they match the
//! words already emitted, so genuinely short or repeated speech that starts a
//! fresh line is preserved. Word timing comes from WebVTT inline timestamp
//! tags (`<00:00:01.230>`) when the cue carries them; otherwise the cue's new
//! words share its interval evenly.

use serde::Serialize;

use crate::{DetectError, Result, TimedTextContract};

/// Identity of the projection algorithm, for consumers recording provenance.
/// Bumped whenever the projection of the same cues can change.
pub const ROLLING_CAPTION_PROJECTION: &str = "media-core/rolling-captions@1";

/// Where a projected word's start time came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RollingWordTiming {
    /// An inline timestamp tag immediately before the word.
    Inline,
    /// The start of the cue that introduced the word.
    CueStart,
    /// Evenly interpolated inside the introducing cue.
    Interpolated,
}

/// One spoken word, introduced by exactly one source cue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollingCaptionWord {
    /// Word text with cue markup removed and basic entities decoded.
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// Position of the introducing cue in the input segment list.
    pub cue_position: usize,
    /// `index` of the introducing cue segment.
    pub cue_index: u64,
    pub timing: RollingWordTiming,
}

/// The spoken words of a rolling caption track, mapped to their source cues.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollingCaptionProjection {
    pub words: Vec<RollingCaptionWord>,
    /// Input positions of cues that introduced no word (pure redisplays or
    /// empty cues).
    pub redisplay_cues: Vec<usize>,
}

/// Projects rolling caption cues into each spoken word once.
///
/// Every segment must carry a time range. The input is not modified.
pub fn project_rolling_captions(contract: &TimedTextContract) -> Result<RollingCaptionProjection> {
    let mut projection = RollingCaptionProjection::default();
    let mut previous_cue_words = 0usize;
    for (position, segment) in contract.segments.iter().enumerate() {
        let (Some(cue_start), Some(cue_end)) = (segment.start_seconds(), segment.end_seconds())
        else {
            return Err(DetectError::InvalidArgument(format!(
                "rolling caption cue {} has no time range",
                segment.index
            )));
        };
        let mut tokens = Vec::new();
        let mut line_ends = Vec::new();
        for line in segment.text.split('\n') {
            tokens.extend(line_tokens(line));
            line_ends.push(tokens.len());
        }
        let overlap =
            redisplayed_prefix(&projection.words, &tokens, &line_ends, previous_cue_words);
        previous_cue_words = tokens.len();
        let new_tokens = &tokens[overlap..];
        if new_tokens.is_empty() {
            projection.redisplay_cues.push(position);
            continue;
        }
        let starts = word_starts(new_tokens, cue_start, cue_end);
        let floor = projection
            .words
            .last()
            .map_or(f64::NEG_INFINITY, |word| word.start_seconds);
        let mut running = floor;
        let clamped: Vec<(f64, RollingWordTiming)> = starts
            .into_iter()
            .map(|(start, timing)| {
                running = running.max(start).min(cue_end.max(running));
                (running, timing)
            })
            .collect();
        for (offset, token) in new_tokens.iter().enumerate() {
            let (start, timing) = clamped[offset];
            let end = clamped
                .get(offset + 1)
                .map_or(cue_end, |(next, _)| *next)
                .max(start);
            projection.words.push(RollingCaptionWord {
                text: token.text.clone(),
                start_seconds: start,
                end_seconds: end,
                cue_position: position,
                cue_index: segment.index,
                timing,
            });
        }
    }
    Ok(projection)
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    text: String,
    start: Option<f64>,
}

/// The longest leading run of `tokens` that redisplays the emitted tail. A
/// redisplay never repeats more than the previous cue showed. When the cue
/// keeps its line breaks, the run must also end on a line boundary or cover
/// the whole previous cue, so speech that starts a fresh line is never taken
/// for a redisplay. Without line breaks (a parser that joined them), any
/// overlap up to the previous cue's length counts.
fn redisplayed_prefix(
    emitted: &[RollingCaptionWord],
    tokens: &[Token],
    line_ends: &[usize],
    previous_cue_words: usize,
) -> usize {
    let limit = previous_cue_words.min(tokens.len());
    let candidates: Vec<usize> = if line_ends.len() > 1 {
        line_ends
            .iter()
            .copied()
            .filter(|&count| count <= limit)
            .chain([limit])
            .collect()
    } else {
        (1..=limit).collect()
    };
    candidates
        .into_iter()
        .filter(|&count| count > 0 && count <= emitted.len())
        .filter(|&count| {
            emitted[emitted.len() - count..]
                .iter()
                .zip(&tokens[..count])
                .all(|(word, token)| word.text == token.text)
        })
        .max()
        .unwrap_or(0)
}

fn word_starts(tokens: &[Token], cue_start: f64, cue_end: f64) -> Vec<(f64, RollingWordTiming)> {
    let mut starts: Vec<Option<(f64, RollingWordTiming)>> = tokens
        .iter()
        .map(|token| token.start.map(|start| (start, RollingWordTiming::Inline)))
        .collect();
    if starts[0].is_none() {
        starts[0] = Some((cue_start, RollingWordTiming::CueStart));
    }
    // Fill untimed runs evenly between the surrounding known starts.
    let mut index = 0;
    while index < starts.len() {
        if starts[index].is_some() {
            index += 1;
            continue;
        }
        let run_start = index;
        while index < starts.len() && starts[index].is_none() {
            index += 1;
        }
        let before = starts[run_start - 1].map_or(cue_start, |(start, _)| start);
        let after = starts
            .get(index)
            .and_then(|start| start.map(|(value, _)| value))
            .unwrap_or(cue_end);
        let step = (after - before) / (index - run_start + 1) as f64;
        for (offset, slot) in starts[run_start..index].iter_mut().enumerate() {
            *slot = Some((
                before + step * (offset + 1) as f64,
                RollingWordTiming::Interpolated,
            ));
        }
    }
    starts.into_iter().flatten().collect()
}

/// Splits one cue line into words, reading inline timestamp tags as the start
/// of the following word and dropping every other tag.
fn line_tokens(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut pending_start = None;
    let mut current = String::new();
    let mut current_start = None;
    let mut chars = line.chars();
    let flush = |current: &mut String, current_start: &mut Option<f64>, tokens: &mut Vec<Token>| {
        let text = decode_entities(current);
        let text = text.trim();
        if !text.is_empty() {
            tokens.push(Token {
                text: text.to_string(),
                start: current_start.take(),
            });
        }
        current.clear();
        *current_start = None;
    };
    while let Some(ch) = chars.next() {
        match ch {
            '<' => {
                let mut tag = String::new();
                for tag_ch in chars.by_ref() {
                    if tag_ch == '>' {
                        break;
                    }
                    tag.push(tag_ch);
                }
                if let Some(seconds) = inline_timestamp(tag.trim()) {
                    flush(&mut current, &mut current_start, &mut tokens);
                    pending_start = Some(seconds);
                }
            }
            ch if ch.is_whitespace() => flush(&mut current, &mut current_start, &mut tokens),
            ch => {
                if current.is_empty() {
                    current_start = pending_start.take();
                }
                current.push(ch);
            }
        }
    }
    flush(&mut current, &mut current_start, &mut tokens);
    // `&nbsp;` decodes to a space inside one token; split such tokens again.
    tokens
        .into_iter()
        .flat_map(|token| {
            let mut first = true;
            token
                .text
                .split_whitespace()
                .map(|part| Token {
                    text: part.to_string(),
                    start: if std::mem::take(&mut first) {
                        token.start
                    } else {
                        None
                    },
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Parses a WebVTT inline timestamp (`[HH:]MM:SS.mmm`).
fn inline_timestamp(tag: &str) -> Option<f64> {
    let parts = tag.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => ("0", *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    let (whole, millis) = seconds.split_once('.')?;
    let digits = |value: &str, width: Option<usize>| {
        (!value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && width.is_none_or(|width| value.len() == width))
        .then(|| value.parse::<u64>().ok())
        .flatten()
    };
    let hours = digits(hours, None)?;
    let minutes = digits(minutes, Some(2))?;
    let whole = digits(whole, Some(2))?;
    let millis = digits(millis, Some(3))?;
    (minutes < 60 && whole < 60)
        .then(|| (hours * 3600 + minutes * 60 + whole) as f64 + millis as f64 / 1000.0)
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lrm;", "")
        .replace("&rlm;", "")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_webvtt, TimedTextSegmentContract};

    fn cues(rows: &[(f64, f64, &str)]) -> TimedTextContract {
        TimedTextContract::new(
            rows.iter()
                .enumerate()
                .map(|(index, (start, end, text))| {
                    TimedTextSegmentContract::new(index as u64, *text)
                        .with_time_range(Some(*start), Some(*end))
                        .unwrap()
                })
                .collect(),
        )
    }

    fn texts(projection: &RollingCaptionProjection) -> Vec<&str> {
        projection
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect()
    }

    // YouTube automatic captions, minus their whitespace-only cues.
    const YOUTUBE: &[(f64, f64, &str)] = &[
        (
            3.32,
            5.884,
            "Good<00:00:03.360><c> morning,</c><00:00:03.640><c> everyone.</c>",
        ),
        (5.884, 5.894, "Good morning, everyone."),
        (5.894, 9.35, "Good morning, everyone.\n&gt;&gt; [applause]"),
        (
            9.36,
            11.55,
            "&gt;&gt; Thank<00:00:09.520><c> you</c><00:00:09.640><c> all.</c>",
        ),
        (11.55, 11.56, "&gt;&gt; Thank you all."),
        (
            11.56,
            13.27,
            "&gt;&gt; Thank you all.\nback<00:00:11.800><c> at</c><00:00:11.920><c> Blender</c>",
        ),
        (13.27, 13.28, "back at Blender"),
    ];
    const SPOKEN: [&str; 12] = [
        "Good",
        "morning,",
        "everyone.",
        ">>",
        "[applause]",
        ">>",
        "Thank",
        "you",
        "all.",
        "back",
        "at",
        "Blender",
    ];

    #[test]
    fn rolling_cues_emit_each_word_once_with_inline_timing() {
        let projection = project_rolling_captions(&cues(YOUTUBE)).unwrap();
        assert_eq!(texts(&projection), SPOKEN);
        assert_eq!(projection.redisplay_cues, [1, 4, 6]);
        let morning = &projection.words[1];
        assert_eq!(morning.timing, RollingWordTiming::Inline);
        assert_eq!((morning.start_seconds, morning.end_seconds), (3.36, 3.64));
        let back = &projection.words[9];
        assert_eq!(back.timing, RollingWordTiming::CueStart);
        assert_eq!((back.start_seconds, back.cue_position), (11.56, 5));
        assert_eq!(projection.words.last().unwrap().end_seconds, 13.27);
    }

    #[test]
    fn cues_with_joined_lines_project_the_same_words() {
        let joined = YOUTUBE
            .iter()
            .map(|(start, end, text)| (*start, *end, text.replace('\n', " ")))
            .collect::<Vec<_>>();
        let rows = joined
            .iter()
            .map(|(start, end, text)| (*start, *end, text.as_str()))
            .collect::<Vec<_>>();
        let projection = project_rolling_captions(&cues(&rows)).unwrap();
        assert_eq!(texts(&projection), SPOKEN);
    }

    #[test]
    fn untagged_incremental_cues_merge_by_overlap_and_interpolate() {
        let vtt = "WEBVTT\n\n\
00:00:01.000 --> 00:00:02.000\nwhat are the\n\n\
00:00:02.000 --> 00:00:04.000\nwhat are the requirements for\n\n\
00:00:04.000 --> 00:00:04.010\nrequirements for\n";
        let projection = project_rolling_captions(&parse_webvtt(vtt).unwrap()).unwrap();
        assert_eq!(
            texts(&projection),
            ["what", "are", "the", "requirements", "for"]
        );
        let requirements = &projection.words[3];
        assert_eq!(requirements.timing, RollingWordTiming::CueStart);
        assert_eq!(requirements.start_seconds, 2.0);
        assert_eq!(projection.words[4].timing, RollingWordTiming::Interpolated);
        assert_eq!(projection.words[4].start_seconds, 3.0);
        assert_eq!(projection.redisplay_cues, [2]);
    }

    #[test]
    fn genuinely_repeated_short_speech_on_a_new_line_is_kept() {
        // "No." said twice: the second cue redisplays line one and adds a new
        // "No." line, which is new speech, not a redisplay.
        let projection = project_rolling_captions(&cues(&[
            (1.0, 1.5, "No."),
            (1.5, 2.0, "No.\nNo."),
            (2.0, 2.5, "Yes."),
        ]))
        .unwrap();
        assert_eq!(texts(&projection), ["No.", "No.", "Yes."]);
        assert!(projection.redisplay_cues.is_empty());
    }

    #[test]
    fn input_contract_is_unchanged() {
        let contract = cues(&[(1.0, 2.0, "hello"), (2.0, 2.01, "hello")]);
        let before = contract.clone();
        let projection = project_rolling_captions(&contract).unwrap();
        assert_eq!(contract, before);
        assert_eq!(texts(&projection), ["hello"]);
        assert_eq!(projection.redisplay_cues, [1]);
    }

    #[test]
    fn untimed_cues_are_rejected() {
        let contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "hi")]);
        assert!(project_rolling_captions(&contract).is_err());
    }

    #[test]
    fn inline_timestamps_parse_with_optional_hours() {
        assert_eq!(inline_timestamp("00:01.500"), Some(1.5));
        assert_eq!(inline_timestamp("01:00:01.250"), Some(3601.25));
        assert_eq!(inline_timestamp("c"), None);
        assert_eq!(inline_timestamp("00:1.500"), None);
    }
}
