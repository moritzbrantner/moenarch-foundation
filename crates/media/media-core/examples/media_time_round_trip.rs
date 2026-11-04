//! Clean-checkout smoke example for the `media-core` interchange contracts.
//!
//! Exercises exact rational media time (`Timestamp`, `MediaRange`, exact
//! rescaling and cross-timebase comparison), the timed-text DTOs, a
//! media-located annotation, and their JSON and WebVTT round trips.
//!
//! Run with `cargo run -p moenarch-media-core --example media_time_round_trip`;
//! `cargo test` runs the same check as a test.

use media_core::annotations::{AnnotationSelector, MediaAnnotation};
use media_core::{
    format_webvtt, parse_webvtt, MediaRange, MediaSourceRef, Timebase, TimedTextContract,
    TimedTextSegmentContract, TimedTextWordContract, Timestamp,
};

fn exact_time() -> MediaRange {
    let millis = Timebase::try_new(1, 1_000).expect("valid timebase");
    let frames = Timebase::try_new(1, 25).expect("valid timebase");
    let start = Timestamp::try_new(1_200, millis).expect("valid timestamp");
    let end = Timestamp::try_new(85, frames).expect("valid timestamp");

    let range = MediaRange::new(start, end).expect("start precedes end");
    let rescaled = start
        .rescale_exact(frames)
        .expect("1.2 s is an exact frame");
    assert_eq!(rescaled.pts, 30);
    assert!(rescaled.same_instant(start).expect("comparable"));
    assert!(range.contains(rescaled).expect("comparable"));
    assert!(!range.contains(end).expect("comparable"), "half-open end");

    let encoded = serde_json::to_string(&start).expect("timestamp serializes");
    assert_eq!(encoded, r#"{"pts":"1200","timebase":{"num":1,"den":1000}}"#);
    assert_eq!(
        serde_json::from_str::<Timestamp>(&encoded).expect("timestamp deserializes"),
        start
    );
    range
}

fn timed_text(range: MediaRange) -> TimedTextContract {
    let start = range.start.checked_seconds().expect("valid start");
    let end = range.end.checked_seconds().expect("valid end");
    let mut segment = TimedTextSegmentContract::new(0, "hello world")
        .with_time_range(Some(start), Some(end))
        .expect("valid segment range");
    segment.language = Some("en".to_string());
    for (text, word_start, word_end) in [("hello", start, 2.0), ("world", 2.0, end)] {
        let word = TimedTextWordContract::new(text)
            .with_time_range(Some(word_start), Some(word_end))
            .expect("valid word range");
        segment.push_word(word).expect("word lies inside segment");
    }
    let mut contract = TimedTextContract::new(vec![segment]);
    contract.source = Some("file:///clip.mp4".to_string());
    contract.validate_strict().expect("strictly valid contract");

    let encoded = serde_json::to_string(&contract).expect("contract serializes");
    let decoded: TimedTextContract = serde_json::from_str(&encoded).expect("contract deserializes");
    assert_eq!(decoded, contract);

    let webvtt = format_webvtt(&contract).expect("renders WebVTT");
    let reparsed = parse_webvtt(&webvtt).expect("parses WebVTT");
    assert_eq!(reparsed.segments[0].text, "hello world");
    assert_eq!(reparsed.segments[0].start_seconds(), Some(start));
    assert_eq!(reparsed.segments[0].end_seconds(), Some(end));
    assert_eq!(format_webvtt(&reparsed).expect("re-renders"), webvtt);
    contract
}

fn annotation(range: MediaRange) -> MediaAnnotation {
    let annotation = MediaAnnotation::new("greeting-1", "speech")
        .label("greeting")
        .during(range)
        .source(MediaSourceRef::source("clip").source_kind("video"))
        .selector(AnnotationSelector::TextSegment {
            stream_id: "captions".to_string(),
            segment_index: 0,
        })
        .validated()
        .expect("valid annotation");
    let encoded = serde_json::to_string(&annotation).expect("annotation serializes");
    let decoded: MediaAnnotation = serde_json::from_str(&encoded).expect("annotation deserializes");
    assert_eq!(decoded, annotation);
    annotation
}

fn round_trip() -> (TimedTextContract, MediaAnnotation) {
    let range = exact_time();
    (timed_text(range), annotation(range))
}

fn main() {
    let (contract, annotation) = round_trip();
    println!(
        "{}",
        format_webvtt(&contract).expect("validated contract renders")
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&annotation).expect("validated annotation serializes")
    );
}

#[test]
fn media_time_and_timed_text_round_trip() {
    round_trip();
}
