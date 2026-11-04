//! Clean-checkout smoke example for the `corpus-core` interchange contract.
//!
//! Builds one source, asset, segment, and derived representation with
//! provenance, serializes every record through the public `CorpusRecord` wire,
//! and checks that decoding restores identical records and relationships.
//!
//! Run with `cargo run -p moenarch-corpus-core --example corpus_round_trip`;
//! `cargo test` runs the same check as a test.

use corpus_core::{
    Asset, AssetId, AssetKind, ContentHash, ContentRef, CorpusItemRef, CorpusRecord, Metadata,
    Provenance, Representation, RepresentationId, RepresentationKind, Segment, SegmentId,
    SegmentLocator, Source, SourceId, SourceKind, Span,
};

fn records() -> Vec<CorpusRecord> {
    let source = Source {
        id: SourceId::derive(&["file:///corpus/lectures"]),
        kind: SourceKind::FileSystem,
        uri: "file:///corpus/lectures".to_string(),
        metadata: Metadata::from([("collection".to_string(), "lectures".to_string())]),
    };
    let asset = Asset {
        id: AssetId::derive(&[source.id.as_str(), "intro.mp4"]),
        source_id: source.id.clone(),
        kind: AssetKind::Video,
        content: ContentRef {
            uri: "file:///corpus/lectures/intro.mp4".to_string(),
            hash: Some(ContentHash::from_bytes(b"fixture video bytes")),
            media_type: Some("video/mp4".to_string()),
            byte_length: Some(19),
        },
        metadata: Metadata::new(),
    };
    let segment = Segment {
        id: SegmentId::derive(&[asset.id.as_str(), "0", "30000"]),
        asset_id: asset.id.clone(),
        locator: SegmentLocator {
            time_range_ms: Some(Span::new(0, 30_000).expect("valid span")),
            frame_range: Some(Span::new(0, 720).expect("valid span")),
            ..SegmentLocator::default()
        },
        provenance: Some(Provenance {
            operation: "chunk".to_string(),
            producer: "fixed-window".to_string(),
            producer_version: Some("1".to_string()),
            inputs: vec![CorpusItemRef::Asset(asset.id.clone())],
            parameters_hash: Some(ContentHash::from_bytes(b"window_ms=30000")),
        }),
        metadata: Metadata::new(),
    };
    let representation = Representation {
        id: RepresentationId::derive(&[segment.id.as_str(), "transcript", "v1"]),
        owner: CorpusItemRef::Segment(segment.id.clone()),
        kind: RepresentationKind::Transcript,
        content_hash: ContentHash::from_bytes(b"hello corpus"),
        content: None,
        provenance: Provenance {
            operation: "transcribe".to_string(),
            producer: "fixture-producer".to_string(),
            producer_version: Some("1.0.0".to_string()),
            inputs: vec![CorpusItemRef::Segment(segment.id.clone())],
            parameters_hash: None,
        },
        metadata: Metadata::new(),
    };
    vec![
        CorpusRecord::Source(source),
        CorpusRecord::Asset(asset),
        CorpusRecord::Segment(segment),
        CorpusRecord::Representation(representation),
    ]
}

fn round_trip() -> String {
    let records = records();
    let encoded = serde_json::to_string_pretty(&records).expect("records serialize");
    let decoded: Vec<CorpusRecord> = serde_json::from_str(&encoded).expect("records deserialize");
    assert_eq!(decoded, records);

    let refs: Vec<CorpusItemRef> = decoded.iter().map(CorpusRecord::item_ref).collect();
    let CorpusRecord::Asset(asset) = &decoded[1] else {
        panic!("second record is the asset");
    };
    assert_eq!(CorpusItemRef::Source(asset.source_id.clone()), refs[0]);
    let CorpusRecord::Segment(segment) = &decoded[2] else {
        panic!("third record is the segment");
    };
    assert_eq!(CorpusItemRef::Asset(segment.asset_id.clone()), refs[1]);
    let CorpusRecord::Representation(representation) = &decoded[3] else {
        panic!("fourth record is the representation");
    };
    assert_eq!(representation.owner, refs[2]);
    assert_eq!(representation.provenance.inputs, vec![refs[2].clone()]);
    encoded
}

fn main() {
    let encoded = round_trip();
    println!("{encoded}");
}

#[test]
fn corpus_records_round_trip_through_json() {
    round_trip();
}
