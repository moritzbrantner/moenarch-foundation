# moenarch-semantic-core

`moenarch-semantic-core` defines the small, domain-neutral contract for attaching inspectable semantic claims to arbitrary entities.

It is deliberately not an ontology engine, classifier framework, vector store, knowledge-graph database, or application policy package.

## Model

```text
EntityId
  -> Annotation(concept, confidence?, producer?, evidence[])

EntityId / ConceptId
  -> Relation(predicate, EntityId / ConceptId, confidence?, producer?, evidence[])
```

The crate separates three layers that consumers should keep distinct:

1. **Observations** are evidence: measured or observed scalar/text values, or references to source evidence.
2. **Claims** are annotations and relations: a producer may infer that an entity belongs to a concept or that two semantic nodes are related.
3. **Policy** belongs to the consumer: whether a speed reader displays a page number, a storefront chooses one navigation category, or a recommender uses a genre label is not part of this crate.

This means overlapping annotations are valid. An entity can simultaneously be classified as several concepts with independent evidence and confidence. `semantic-core` does not select a winner or require a single category.

## Identity

`EntityId`, `ConceptId`, and `ProducerId` are stable string identifiers. Consumers may retain an existing external identity with `new`, or derive a deterministic identity from length-delimited components with `derive`.

The identifiers intentionally do not depend on corpus, NLP, OCR, music, product, or application types. Capability repositories can map their native identities into this shared contract without Foundation becoming the owner of those domains.

## Confidence

`Confidence` is a finite number in the inclusive range `[0, 1]`. Invalid values are rejected both by constructors and deserialization. Confidence is deliberately optional: deterministic facts or uncalibrated classifiers should not be forced to invent a probability-like score.

`FiniteNumber` provides the corresponding finite-only scalar for numeric evidence where values outside `[0, 1]` are meaningful.

## Evidence

Evidence is intentionally small and inspectable:

- an `Observation` carries a named feature plus a typed text, boolean, integer, or finite numeric value;
- a `Reference` carries an opaque stable source reference plus an optional selector.

A reference may point to a corpus record, OCR region, file fragment, external URI, model artifact, or another consumer-owned source. The shared crate does not interpret the reference or depend on the repository that owns it.

## Relations

A relation connects two `SemanticNodeRef` values through a predicate `ConceptId`. Endpoints may be entities or concepts, which is enough to express domain-owned relationships such as `is_a`, `part_of`, `used_for`, `supports`, or `contradicts` without baking those predicates into Foundation.

The predicate vocabulary remains consumer-owned. Graph traversal, inheritance, cycle validation, transitive closure, contradiction resolution, and persistence are later layers.

## Non-goals

This crate does not:

- define OCR roles, music genres, product categories, NLP labels, or another ontology;
- define a classifier trait or model-provider interface;
- resolve conflicting or overlapping claims;
- assign application actions such as `include`, `display`, `rank`, or `delete`;
- generate or store embeddings;
- choose a graph/vector/database backend;
- depend on `corpus-core`, `nlp-stack`, `visual-analysis`, `speedreader`, or another capability repository;
- publish or release itself merely because source-development consumers need the contract.

The first consumers should dogfood these primitives before classifier composition, ontology tooling, graph traversal, or persistence is standardized.
