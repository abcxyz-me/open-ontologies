# Using Open Ontologies beside a knowledge graph platform

Several excellent open projects build knowledge graphs from documents:
[Microsoft GraphRAG](https://github.com/microsoft/graphrag) (MIT),
[Semantica](https://github.com/semantica-agi/semantica) (MIT), and
[TrustGraph](https://github.com/trustgraph-ai/trustgraph) (Apache-2.0).

Open Ontologies is not a competitor to any of them. It is a different layer,
and the combination is stronger than either part.

## The division of labour

Those platforms **generate**: they ingest documents, call a model to extract
entities and relationships, and assemble a graph. They are pipelines, written
in Python, with orchestration, storage, and retrieval included.

Open Ontologies **verifies and governs**: it is a single Rust binary with no
model client inside it, exposing ~95 MCP tools over a formal store. Its job
starts where extraction ends.

```
document corpus
      |
      v
  [ platform ]      GraphRAG / Semantica / TrustGraph
  extract, index    a model proposes entities and relations
      |
      v
  [ open-ontologies ]
  onto_validate     is it even syntactically RDF
  onto_load         all or nothing: no silently partial graphs
  onto_vocab_check  CLOSED-WORLD: which terms were invented
  onto_shacl        cardinality and datatype constraints
  onto_enforce      design patterns, including competing modelling patterns
  onto_reason       materialise, then find contradictions with provenance
  onto_plan/apply   governed change with risk scoring and locked IRIs
      |
      v
  a graph you can defend
```

The asymmetry worth naming: a generation pipeline cannot check its own
output. An extractor that invents `:hasProteinName` because it sounded
plausible produces RDF that parses, loads, and passes open-world SHACL
without complaint, because in the open world an undeclared term is merely
unknown, not wrong. Closed-world checking is the missing half, and it is
what `onto_vocab_check` does.

## Interop is free: everyone speaks the standards

No adapters, no plugins, no coupling. All four projects read and write W3C
standards, so the handoff is a file or a SPARQL endpoint:

| Concern | Shared ground |
|---|---|
| Serialisation | Turtle, N-Triples, RDF/XML, JSON-LD, TriG |
| Schema | OWL, RDFS, SKOS |
| Constraints | SHACL |
| Provenance | PROV-O (`prov:wasDerivedFrom`) |
| Transport | SPARQL 1.1, and MCP for tool access |

```bash
# whatever produced graph.ttl, verify it before trusting it
open-ontologies validate graph.ttl
open-ontologies load graph.ttl
open-ontologies enforce generic
```

Or, over MCP, let an agent do the same with `onto_load`, `onto_vocab_check`,
`onto_enforce` and `onto_reason` as tools in its session.

## What each pairing gives you

**With Microsoft GraphRAG.** GraphRAG's community detection and community
reports answer global questions ("what are the themes across this corpus")
that entity traversal cannot. Open Ontologies adds a schema those entities
must conform to, and a check that the extraction did not invent any of it.
`onto_communities` computes the same hierarchical community structure
deterministically inside the engine, and returns skeletons for the connected
model to summarise, so the expensive part stays under your control.

**With Semantica.** Semantica already embeds Oxigraph and emits PROV-O, so
the graphs move between the two with no conversion at all. Its pipeline is
close in shape to what verification wants: extract, detect conflicts,
deduplicate, record provenance. Add closed-world checking after extraction
and the plan/enforce/apply lifecycle around changes.

**With TrustGraph.** Knowledge cores are versioned, promotable knowledge
artifacts. `onto_pack` produces the same kind of artifact from a verified
graph (ontology, instances, provenance, embedding fingerprint, manifest,
checksum), so what you promote between environments is a graph that has
already passed its checks, with the evidence bundled alongside it.

## The MCP-native convention, and why it matters here

Open Ontologies deliberately contains no model client. Where a task needs
judgement (is this alignment candidate a true duplicate; what should this
community be called), the engine returns the structured evidence and the
*connected orchestrator* decides, feeding verdicts back through feedback
tools that retrain the scorers.

This is why the pairing composes rather than conflicts. The platform brings
its own models and its own pipeline; the engine never competes for that
role. It supplies the primitives a model cannot compute for itself: formal
semantics, sound inference, closed-world checks, deterministic community
structure, and an audit trail.
