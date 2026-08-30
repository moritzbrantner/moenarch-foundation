use media_core::{
    format_audacity_labels, format_plain_text, format_srt, format_tsv, format_webvtt,
    parse_plain_text, parse_srt, parse_webvtt, TimedTextContract, TimedTextSegmentContract,
};

const CANONICAL_SRT: &str = include_str!("fixtures/canonical.srt");
const CANONICAL_VTT: &str = include_str!("fixtures/canonical.vtt");
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
    let mut webvtt_contract = contract.clone();
    webvtt_contract.segments[0].text = "  Hello\nworld -> now  ".to_string();

    assert_eq!(format_srt(&contract).unwrap(), CANONICAL_SRT);
    assert_eq!(format_webvtt(&webvtt_contract).unwrap(), CANONICAL_VTT);
    assert_eq!(format_plain_text(&contract), CANONICAL_TEXT);
    assert_eq!(format_tsv(&contract).unwrap(), CANONICAL_TSV);
    assert_eq!(format_audacity_labels(&contract).unwrap(), CANONICAL_AUD);
}

#[test]
fn subtitle_round_trips_preserve_the_canonical_projection() {
    let srt = format_srt(&parse_srt(CANONICAL_SRT).unwrap()).unwrap();
    let webvtt = format_webvtt(&parse_webvtt(CANONICAL_VTT).unwrap()).unwrap();

    assert_eq!(srt, CANONICAL_SRT);
    assert_eq!(webvtt, CANONICAL_VTT);
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
fn webvtt_metadata_is_accepted_before_cues_and_discarded() {
    let input = "WEBVTT\n\nNOTE source metadata\nnot a cue\n\nSTYLE\n::cue { color: lime; }\n\nREGION\nid:main\nwidth:40%\n\nidentifier\n00:01.000 --> 00:02.000 align:start\ncaption\n";
    let parsed = parse_webvtt(input).unwrap();

    assert_eq!(parsed.segments.len(), 1);
    assert_eq!(parsed.segments[0].index, 0);
    assert_eq!(parsed.segments[0].text, "caption");
    assert_eq!(
        format_webvtt(&parsed).unwrap(),
        "WEBVTT\n\n00:01.000 --> 00:02.000\ncaption\n\n"
    );

    let style_after_cue =
        "WEBVTT\n\n00:00.000 --> 00:01.000\ncaption\n\nSTYLE\n::cue { color: lime; }\n";
    assert!(parse_webvtt(style_after_cue).is_err());
    assert!(parse_webvtt("WEBVTT\n\nNOTE invalid --> comment\n").is_err());
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
fn subtitle_parsers_enforce_syntax_specific_timestamp_grammar() {
    for invalid_srt in [
        "1\n00:00:00.000 --> 00:00:01.000\ncue\n",
        "1\n00:00,000 --> 00:01,000\ncue\n",
        "1\n00:00:00,00 --> 00:00:01,000\ncue\n",
        "1\n0:00:00,000 --> 00:00:01,000\ncue\n",
        "1\n100:00:00,000 --> 100:00:01,000\ncue\n",
    ] {
        assert!(parse_srt(invalid_srt).is_err());
    }
    for invalid_webvtt in [
        "00:00.000 --> 00:01.000\ncue\n",
        "WEBVTT\n\n00:00,000 --> 00:01,000\ncue\n",
        "WEBVTT\n\n00:00.00 --> 00:01.000\ncue\n",
        "WEBVTT\n\n00:00.000 --> 00:00.000\ncue\n",
    ] {
        assert!(parse_webvtt(invalid_webvtt).is_err());
    }
    let long_webvtt =
        parse_webvtt("WEBVTT\n\n100:00:00.000 --> 100:00:01.000\nlong recording\n").unwrap();
    assert_eq!(long_webvtt.segments[0].start_seconds(), Some(360_000.0));

    let too_long_for_srt =
        TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "long recording")
            .with_time_range(Some(360_000.0), Some(360_001.0))
            .unwrap()]);
    assert!(format_srt(&too_long_for_srt).is_err());
}

#[test]
fn srt_preserves_arrows_while_webvtt_rejects_them_in_payloads() {
    let contract = fixture_contract();

    assert!(format_srt(&contract).unwrap().contains("world --> now"));
    assert!(format_webvtt(&contract).is_err());
    assert!(parse_srt(CANONICAL_SRT).unwrap().segments[0]
        .text
        .contains("world --> now"));
    assert!(parse_webvtt("WEBVTT\n\n00:00.000 --> 00:01.000\nnot --> allowed\n").is_err());
}

#[test]
fn subtitle_renderers_normalize_constructed_contract_line_endings() {
    let contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(
        0,
        "first\r\nsecond\rthird",
    )
    .with_time_range(Some(0.0), Some(1.0))
    .unwrap()]);

    assert_eq!(
        format_srt(&contract).unwrap(),
        "1\n00:00:00,000 --> 00:00:01,000\nfirst\nsecond\nthird\n\n"
    );
    assert_eq!(
        format_webvtt(&contract).unwrap(),
        "WEBVTT\n\n00:00.000 --> 00:01.000\nfirst\nsecond\nthird\n\n"
    );
}

#[test]
fn webvtt_validates_rounded_ranges_and_start_order() {
    let contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "boundary")
        .with_time_range(Some(3_599.999_6), Some(3_600.001))
        .unwrap()]);

    assert_eq!(
        format_webvtt(&contract).unwrap(),
        "WEBVTT\n\n01:00:00.000 --> 01:00:00.001\nboundary\n\n"
    );

    let rounded_to_zero = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "short")
        .with_time_range(Some(1.000_1), Some(1.000_4))
        .unwrap()]);
    assert!(format_webvtt(&rounded_to_zero).is_err());

    let out_of_order = TimedTextContract::new(vec![
        TimedTextSegmentContract::new(0, "later")
            .with_time_range(Some(2.0), Some(3.0))
            .unwrap(),
        TimedTextSegmentContract::new(1, "earlier")
            .with_time_range(Some(1.0), Some(1.5))
            .unwrap(),
    ]);
    assert!(format_webvtt(&out_of_order).is_err());
    assert!(parse_webvtt(
        "WEBVTT\n\n00:02.000 --> 00:03.000\nlater\n\n00:01.000 --> 00:01.500\nearlier\n"
    )
    .is_err());
}

#[test]
fn millisecond_rendering_rejects_values_that_round_to_two_to_the_64th() {
    let boundary_seconds = u64::MAX as f64 / 1_000.0;
    let contract = TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "too large")
        .with_time_range(Some(0.0), Some(boundary_seconds))
        .unwrap()]);

    assert!(format_srt(&contract).is_err());
    assert!(format_webvtt(&contract).is_err());
    assert!(format_tsv(&contract).is_err());
}
