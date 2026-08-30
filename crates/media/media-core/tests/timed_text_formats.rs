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
    let input = "WEBVTT\n\nNOTE source metadata\nnot a cue\n\nSTYLE\n::cue { color: lime; }\n\nREGION\nid:main\nwidth:40%\nlines:3\nregionanchor:0%,100%\nviewportanchor:10%,90%\nscroll:up\n\nidentifier\n00:01.000 --> 00:02.000 vertical:rl line:10%,center position:20%,line-left size:40% align:start region:main\ncaption\n";
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
fn webvtt_rejects_invalid_cue_settings_and_region_fields() {
    for setting in [
        "unknown:value",
        "align:start align:end",
        "vertical:up",
        "line:invalid",
        "line:+1",
        "line:10%,bogus",
        "position:101%",
        "size:-1%",
        "size:.5%",
        "size:5.%",
        "align:middle",
        "bare-setting",
    ] {
        let input = format!("WEBVTT\n\n00:00.000 --> 00:01.000 {setting}\ncaption\n");
        assert!(parse_webvtt(&input).is_err(), "accepted setting: {setting}");
    }

    for fields in [
        "width:40%",
        "id:main\nid:other",
        "id:main\nunknown:value",
        "id:main\nwidth:101%",
        "id:main\nlines:0",
        "id:main\nregionanchor:10%",
        "id:main\nscroll:down",
        "id main",
    ] {
        let input = format!("WEBVTT\n\nREGION\n{fields}\n");
        assert!(parse_webvtt(&input).is_err(), "accepted REGION: {fields}");
    }

    for fields in [
        "id:same-line width:40% lines:3",
        "id:tabbed\twidth:40%\tregionanchor:0%,100%",
    ] {
        let input = format!(
            "WEBVTT\n\nREGION\n{fields}\n\n00:00.000 --> 00:01.000 region:same-line\ncaption\n"
        );
        assert!(parse_webvtt(&input).is_ok(), "rejected REGION: {fields}");
    }

    assert!(parse_webvtt("WEBVTT\n\nREGION\nid:main width:40% malformed\n").is_err());
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
fn srt_identifiers_are_either_valid_u64_values_or_absent() {
    for identifier in ["cue-one", "18446744073709551616"] {
        let input = format!("{identifier}\n00:00:00,000 --> 00:00:01,000\ncaption\n");
        assert!(parse_srt(&input).is_err());
    }

    let without_identifiers = parse_srt(
        "00:00:00,000 --> 00:00:01,000\nfirst\n\n00:00:01,000 --> 00:00:02,000\nsecond\n",
    )
    .unwrap();
    assert_eq!(without_identifiers.segments[0].index, 0);
    assert_eq!(without_identifiers.segments[1].index, 1);
}

#[test]
fn parsed_timestamps_must_round_trip_through_f64_seconds() {
    fn webvtt_timestamp(milliseconds: u64) -> String {
        let hours = milliseconds / 3_600_000;
        let minutes = (milliseconds / 60_000) % 60;
        let seconds = (milliseconds / 1_000) % 60;
        let millis = milliseconds % 1_000;
        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    }

    for milliseconds in [9_007_199_254_740_993, u64::MAX - 1_000] {
        let end = webvtt_timestamp(milliseconds);
        let input = format!("WEBVTT\n\n00:00.000 --> {end}\ncaption\n");
        assert!(
            parse_webvtt(&input).is_err(),
            "accepted non-round-trippable timestamp: {end}"
        );
    }
}

#[test]
fn webvtt_signature_requires_a_line_terminator_and_blank_separator() {
    for invalid in [
        "WEBVTT",
        "WEBVTT\n",
        "WEBVTT\n00:00.000 --> 00:01.000\ncaption\n",
    ] {
        assert!(parse_webvtt(invalid).is_err());
    }

    let valid = "\u{feff}WEBVTT source comment\r\n\r\n00:00.000 --> 00:01.000\r\ncaption\r\n";
    assert_eq!(parse_webvtt(valid).unwrap().segments.len(), 1);
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
fn subtitle_payloads_cannot_contain_internal_blank_lines() {
    let contract =
        TimedTextContract::new(vec![TimedTextSegmentContract::new(0, "first\r\n \rsecond")
            .with_time_range(Some(0.0), Some(1.0))
            .unwrap()]);

    assert!(format_srt(&contract).is_err());
    assert!(format_webvtt(&contract).is_err());
    assert!(parse_srt("1\n00:00:00,000 --> 00:00:01,000\nfirst\n \nsecond\n").is_err());
    assert!(parse_webvtt("WEBVTT\n\n00:00.000 --> 00:01.000\nfirst\n \nsecond\n").is_err());
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
