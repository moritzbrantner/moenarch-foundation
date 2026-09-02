# moenarch-corpus-core

`moenarch-corpus-core` defines the small, backend-neutral contract shared by corpus-style applications that ingest source material, address parts of it, derive searchable representations, and retrieve results later.

It is deliberately not a database, search engine, media processor, or NLP package.

## Model

```text
Source
  -> Asset
       -> Segment

Source / Asset / Segment
  -> Representation
       -> Provenance(inputs, producer, operation)
```

- **Source** identifies where material comes from: a filesystem, URL, object store, database, inline source, or another explicitly named source kind.
- **Asset** is one concrete corpus item such as text, a document, image, video, audio file, subtitle file, or web page.
- **Segment** addresses a part of an asset. Common byte, character, page, time, and frame coordinates are first-class; domain-specific selectors remain extensible metadata.
- **Representation** is derived, rebuildable material such as extracted text, OCR, a transcript, thumbnail, embedding, perceptual hash, or another named representation.
- **Provenance** records the operation, producer/version, inputs, and optional parameter hash used to create a derived segment or representation.

Original source assets are the canonical data. Segments, representations, search indexes, thumbnails, embeddings, and similar products should be treated as rebuildable unless a consumer explicitly gives them stronger semantics.

## Identity

The crate provides typed `SourceId`, `AssetId`, `SegmentId`, and `RepresentationId` values. Consumers may retain an existing stable external identifier with `new`, or derive one deterministically from length-delimited components with `derive`.

`ContentHash` is the canonical lowercase SHA-256 identity for immutable bytes. IDs and content hashes solve different problems: an ID names a corpus record; a content hash identifies exact content.

## Storage and retrieval seams

`CorpusStore` is the minimal persistence seam:

- `upsert(record)`
- `get(id)`
- `delete(id)`

`Retriever` is the minimal search seam. `RetrievalQuery` can carry text, an existing corpus item, or a dense vector, plus a strategy, result limit, and deterministic equality filters. `RetrievalHit::rank` is the portable ordering contract; `raw_score` is intentionally backend-defined.

Concrete adapters belong outside this crate. SQLite, Postgres, Qdrant, Tantivy, LanceDB, object stores, and browser persistence can all implement the same foundation contract without becoming dependencies of it.

## Non-goals

This crate does not:

- choose or implement a persistence engine;
- generate embeddings or define embedding models;
- perform OCR, transcription, scene detection, thumbnailing, decoding, or chunking;
- define application-specific ranking policy;
- depend on NLP, audio-analysis, visual-analysis, or application repositories;
- expose CLI, server, or WASM adapters in the initial slice.

The first consumers should dogfood this contract before richer filter expressions, graph traversal, transactions, or backend capabilities are standardized.
