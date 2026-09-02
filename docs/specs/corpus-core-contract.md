# Corpus core contract

`moenarch-corpus-core` owns only the domain-neutral seam shared by applications that collect heterogeneous source material and later retrieve or relate it. It must remain independent of storage engines, search engines, NLP, audio-analysis, visual-analysis, and application policy.

## Ownership boundary

The core model is:

```text
Source -> Asset -> Segment
                 \
                  -> Representation
```

A representation may also attach directly to a source or asset. Derived records retain explicit provenance inputs rather than relying on implicit filenames, database joins, or backend-specific point identifiers.

## Invariants

1. **Record identity and content identity are distinct.** Typed corpus IDs identify logical records. `ContentHash` identifies exact immutable bytes with canonical lowercase SHA-256.
2. **IDs are stable and backend-neutral.** Existing external identities may be retained. Deterministic derived IDs hash length-delimited components so component boundaries cannot alias through naive concatenation.
3. **Original assets are canonical by default.** Segments, extracted text, OCR, transcripts, thumbnails, embeddings, perceptual hashes, and indexes are expected to be rebuildable unless a consuming product explicitly grants stronger semantics.
4. **Segments remain traceable to assets.** Portable locators cover byte, character, page, time, and frame coordinates. Domain-specific selectors use deterministic extension metadata until a coordinate has enough cross-domain use to become a shared typed field.
5. **Derived records explain their inputs.** Provenance records the operation, producer, optional producer version, direct input records, and optional canonical parameter hash.
6. **The store seam does not own payload placement.** A `CorpusStore` persists corpus records; referenced payload bytes may live in files, object storage, a database, browser storage, or another consumer-selected location.
7. **Retrieval ordering is portable; scores are not.** `RetrievalHit.rank` communicates result order. `raw_score` remains backend-defined and must not be compared across retrieval implementations unless those implementations define a shared score contract separately.
8. **Dense vectors are inputs, not an embedding policy.** The core accepts finite vectors supplied by capability layers but does not generate embeddings, select models, normalize vectors, or choose an ANN implementation.

## Deliberately deferred

The first contract does not standardize transactions, pagination/cursors, graph traversal, compound boolean filters, index lifecycle, background jobs, capability discovery, chunking policy, model metadata, or backend-specific consistency guarantees.

Those should be added only after `document-search`, `youtube-corpus`, `media-similarity`, or another real consumer demonstrates a shared requirement. This keeps the initial seam small enough to change before downstream adoption hardens it.
