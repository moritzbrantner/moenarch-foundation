use media_core::{
    format_audacity_labels, format_plain_text, format_srt, format_tsv, format_webvtt,
    parse_plain_text, parse_srt, parse_webvtt, TimedTextContract, TimedTextSegmentContract,
};

const SRT_FIXTURE_BODY: &str = include_str!("fixtures/canonical.srt");
const VTT_FIXTURE_BODY: &str = include_str!("fixtures/canonical.vtt");
const CANONICAL_TEXT: &str = include_str!("fixtures/canonical.txt");
const CANONICAL_TSV: &str = include_str!("fixtures/canonical.tsv");
const CANONICAL_AUD: &str = include_str!("fixtures/canonical.aud");

fn fixture_contract() -> TimedTextContract {
    let mut first = TimedTextSegmentContract::new(17, "  Hello\nworld --> now  ")
        .with_time_range(Some(0.0), Some(1.2346))
        .unwrap();
    first.speaker = Some("SPEAKER_00".to_string());
    let mut second = TimedTextSegmentContract::new(42, "Later cue")
        .with_time_range(Some(3_600.0), Some(3_602.4))
        .unwrap();
    second.speaker = Some("SPEAKER_01".to_string());
    TimedTextContract::new(vec![first, second])
}

#[test]
fn renderers_match_canonical_format_fixtures_exactly() {
    let contract = fixture_contract();
    let canonical_srt = format!("{SRT_FIXTURE_BODY}\n");
    let canonical_vtt = format!("{VTT_FIXTURE_BODY}\n");

    assert_eq!(format_srt(&contract).unwrap(), canonical_srt);
    assert_eq!(format_webvtt(&contract).unwrap(), canonical_vtt);
    assert_eq!(format_plain_text(&contract), CANONICAL_TEXT);
    assert_eq!(format_tsv(&contract).unwrap(), CANONICAL_TSV);
    assert_eq!(format_audacity_labels(&contract).unwrap(), CANONICAL_AUD);
}

#[test]
fn subtitle_round_trips_preserve_the_canonical_projection() {
    let canonical_srt = format!("{SRT_FIXTURE_BODY}\n");
    let canonical_vtt = format!("{VTT_FIXTURE_BODY}\n");
    let srt = format_srt(&parse_srt(&canonical_srt).unwrap()).unwrap();
    let webvtt = format_webvtt(&parse_webvtt(&canonical_vtt).unwrap()).unwrap();

    assert_eq!(srt, canonical_srt);
    assert_eq!(webvtt, canonical_vtt);
}

#[test]
fn parsers_accept_bom_crlf_ids_settings_and_multiline_text() {
    let srt = "\u{feff}27\r\n00:00:01,000 --> 00:00:02,250\r\nfirst line\r\nsecond line\r\n";
    let parsed_srt = parse_srt(srt).unwrap();
    assert_eq!(parsed_srt.segments[0].index, 27);
    assert_eq!(parsed_srt.segments[0].text, "first line\nsecond line");
    assert_eq!(parsed_srt.text.as_deref(), Some("first line\nsecond line"));

    let vtt = "\u{feff}WEBVTT transcript\r\n\r\nintro\r\n00:01.500 --> 00:02.750 position:10% line:90%\r\nfirst line\r\nsecond line\r\n";
    let parsed_vtt = parse_webvtt(vtt).unwrap();
    assert_eq!(parsed_vtt.segments[0].index, 0);
    assert_eq!(parsed_vtt.segments[0].start_seconds(), Some(1.5));
    assert_eq!(parsed_vtt.segments[0].end_seconds(), Some(2.75));
    assert_eq!(parsed_vtt.segments[0].text, "first line\nsecond line");
}

#[test]
fn plain_text_normalizes_lines_and_has_a_deterministic_projection() {
    let parsed = parse_plain_text("\u{feff} first \r\n\r\n second\rthird \n");

    assert_eq!(parsed.segments.len(), 3);
    assert_eq!(parsed.segments[0].index, 0);
    assert_eq!(parsed.segments[1].index, 2);
    assert_eq!(parsed.segments[2].index, 3);
    assert!(parsed
        .segments
        .iter()
        .all(|segment| segment.start_seconds().is_none()));
    assert_eq!(format_plain_text(&parsed), "first\nsecond\nthird\n");
    assert_eq!(parse_plain_text("").text.as_deref(), Some(""));
    assert_eq!(format_plain_text(&TimedTextContract::default()), "");
}

#[test]
fn plain_tsv_and_audacity_renderers_do_not_add_speaker_syntax() {
    let mut contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(
        0,
        "  one\ttwo\nthree  ",
    )
    .with_time_range(Some(1.0), Some(2.0))
    .unwrap()]);
    contract.segments[0].speaker = Some("speaker-a".to_string());

    assert_eq!(format_plain_text(&contract), "one\ttwo\nthree\n");
    assert_eq!(
        format_tsv(&contract).unwrap(),
        "start\tend\ttext\n1000\t2000\tone two three\n"
    );
    assert_eq!(
        format_audacity_labels(&contract).unwrap(),
        "1\t2\tone two three\n"
    );
}

#[test]
fn timed_rendering_requires_complete_non_negative_ranges() {
    let missing_end = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "cue")
        .with_time_range(Some(1.0), None)
        .unwrap()]);
    let negative = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "cue")
        .with_time_range(Some(-1.0), Some(0.0))
        .unwrap()]);

    assert!(format_srt(&missing_end)
        .unwrap_err()
        .to_string()
        .contains("requires both start and end"));
    assert!(format_webvtt(&negative)
        .unwrap_err()
        .to_string()
        .contains("non-negative"));
    assert!(format_tsv(&missing_end).is_err());
    assert!(format_audacity_labels(&missing_end).is_err());
    assert!(format_tsv(&negative).is_err());
    assert!(format_audacity_labels(&negative).is_err());
}

#[test]
fn subtitle_parsers_reject_malformed_ranges() {
    let invalid_documents = [
        "1\nmissing timing\n",
        "1\n00:00:01,000 -->\ncue\n",
        "1\n-00:00:01,000 --> 00:00:02,000\ncue\n",
        "1\n00:00:02,000 --> 00:00:01,000\ncue\n",
        "1\n00:60:00,000 --> 01:00:01,000\ncue\n",
        "1\n00:00:60,000 --> 00:01:01,000\ncue\n",
        "1\n00:00:00,000 --> 00:00:01,000 settings\ncue\n",
        "1\n00:00:00,000 --> 00:00:01,000\n",
    ];

    for document in invalid_documents {
        assert!(
            parse_srt(document).is_err(),
            "accepted invalid SRT: {document:?}"
        );
    }
    assert!(parse_webvtt("WEBVTT\n\n00:00.000 --> -00:01.000\ncue\n").is_err());
}

#[test]
fn webvtt_hour_field_follows_rounded_timestamp() {
    let contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "boundary")
        .with_time_range(Some(3_599.999_6), Some(3_600.000_4))
        .unwrap()]);

    assert_eq!(
        format_webvtt(&contract).unwrap(),
        "WEBVTT\n\n01:00:00.000 --> 01:00:00.000\nboundary\n\n"
    );
}
