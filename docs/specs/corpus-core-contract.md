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

## Capability-repository compatibility

`Retriever`, `RetrievalQuery`, and `RetrievalHit` are interchange contracts, not a replacement for capability-owned search APIs. A capability repository may expose richer request and response types and adapt them at its boundary when a real corpus consumer needs cross-domain interchange.

For `nlp-stack`, the intended ownership split is:

- `moenarch-corpus-core` owns corpus identity, portable item references, the minimal retrieval input/strategy/limit/equality-filter contract, portable result rank, and backend-opaque score transport;
- `text-index` owns text index materialization, storage, mutation, text chunking policy, and index-specific query support;
- `text-retrieval` owns text query interpretation, lexical/semantic/hybrid ranking, fusion, filtering beyond the portable equality subset, facets, snippets, candidate windows, related-content behavior, and reranking;
- `moenarch-vector-analysis-index` owns exact in-memory vector lookup mechanics and remains an implementation primitive rather than a second corpus contract.

A future NLP adapter may map the portable subset losslessly:

| Foundation request | NLP capability mapping |
| --- | --- |
| text + `Lexical` | full-text retrieval |
| text + `Semantic` | semantic text retrieval |
| text + `Hybrid` | hybrid text retrieval |
| corpus item + `Similarity` | related-content lookup when the item maps to an indexed text chunk |
| dense vector + `Similarity`/`Semantic` | vector-search-backed capability path when the adapter has compatible vector metadata |
| equality metadata filters | the equality subset of NLP filtering |

NLP-only controls such as semantic/full-text weights, candidate windows, metadata-contains filters, tag constraints, facets, snippets, sorting, model selection, reranking, and score decomposition stay in `nlp-stack`. They must not be added to `corpus-core` merely to make the two APIs structurally identical.

Likewise, `RetrievalStrategy` names intent at the portable boundary; it does not standardize BM25, embedding generation, vector metrics, rank-fusion formulas, or score calibration. If a ranking, fusion, top-k, filtering, or indexing primitive is later proposed for Foundation, it needs evidence of reuse outside NLP before moving downward.

No pairwise `foundation-nlp` bridge crate should be created for mechanical conversions. Put small conversions in the consuming adapter or application until repeated independent consumers prove a stable shared transformation.

## Deliberately deferred

The first contract does not standardize transactions, pagination/cursors, graph traversal, compound boolean filters, index lifecycle, background jobs, capability discovery, chunking policy, model metadata, or backend-specific consistency guarantees.

Those should be added only after `document-search`, `youtube-corpus`, `media-similarity`, or another real consumer demonstrates a shared requirement. This keeps the initial seam small enough to change before downstream adoption hardens it.
