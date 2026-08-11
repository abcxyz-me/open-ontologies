//! Bi-temporal facts: when something was true, and when we learned it.
//!
//! Two clocks, deliberately independent:
//!
//!   - VALID time is when a statement holds in the world. A cell line was
//!     adherent until May and suspension after; both statements are true, of
//!     different periods.
//!   - RECORDED time is when the store came to hold it. A correction entered
//!     today about last year changes what we know, not what happened.
//!
//! Collapsing them loses the two questions people actually ask: what was true
//! then, and what did we believe then. An audit needs the second; analysis
//! needs the first; a contradiction check needs both, because two statements
//! only conflict if they claim the same period.
//!
//! ## Shape on disk
//!
//! RDF-star would be the elegant carrier and the parser does not accept it,
//! so assertions live in NAMED GRAPHS and their validity is described in the
//! default graph, which is ordinary TriG that any store can read:
//!
//! ```turtle
//! :g1 { :HEK293 a :AdherentCellLine . }
//! :g2 { :HEK293 a :SuspensionCellLine . }
//! {
//!   :g1 t:validFrom "2024-01-01"^^xsd:date ;
//!       t:validTo   "2026-05-01"^^xsd:date ;
//!       t:recordedAt "2024-01-05"^^xsd:dateTime .
//!   :g2 t:validFrom "2026-05-01"^^xsd:date ;
//!       t:recordedAt "2026-05-02"^^xsd:dateTime .
//! }
//! ```
//!
//! An absent `validFrom` means "since always", an absent `validTo` means
//! "still true", and a graph with no temporal description at all is timeless:
//! it is in scope for every snapshot, so adding this vocabulary to an
//! existing store changes nothing until it is used.
//!
//! Intervals are half-open, `[validFrom, validTo)`. Two facts that meet at a
//! boundary do not overlap, which is what makes "adherent until May,
//! suspension from May" a correction rather than a contradiction.

use crate::graph::GraphStore;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const NS: &str = "https://open-ontologies.org/temporal#";

pub struct Temporal {
    graph: Arc<GraphStore>,
}

#[derive(Clone, Debug)]
pub struct Validity {
    pub graph: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub recorded_at: Option<String>,
}

impl Validity {
    /// Was this true at `instant`, on the half-open interval.
    fn valid_at(&self, instant: &str) -> bool {
        self.valid_from.as_deref().is_none_or(|f| f <= instant)
            && self.valid_to.as_deref().is_none_or(|t| instant < t)
    }

    /// Had we recorded it by `instant`.
    fn recorded_by(&self, instant: &str) -> bool {
        self.recorded_at.as_deref().is_none_or(|r| r <= instant)
    }

    /// Do two validity periods share any instant. Half-open, so touching
    /// intervals do not overlap.
    fn overlaps(&self, other: &Validity) -> bool {
        let start_before_end = |a: &Option<String>, b: &Option<String>| match (a, b) {
            (Some(start), Some(end)) => start.as_str() < end.as_str(),
            _ => true, // an open end never closes the interval
        };
        start_before_end(&self.valid_from, &other.valid_to)
            && start_before_end(&other.valid_from, &self.valid_to)
    }

    fn describe(&self) -> String {
        let from = self.valid_from.as_deref().unwrap_or("always");
        let to = self.valid_to.as_deref().unwrap_or("still true");
        format!("{from} to {to}")
    }
}

/// Literal or IRI as SPARQL returns it, without its wrapping.
fn plain(value: &str) -> String {
    let v = value.trim();
    if v.starts_with('<') && v.ends_with('>') {
        return v[1..v.len() - 1].to_string();
    }
    if let Some(body) = v.strip_prefix('"') {
        for cut in ["\"^^", "\"@", "\""] {
            if let Some(i) = body.find(cut) {
                return body[..i].to_string();
            }
        }
    }
    v.to_string()
}

fn local(iri: &str) -> &str {
    iri.rsplit(['#', '/']).next().unwrap_or(iri)
}

impl Temporal {
    pub fn new(graph: Arc<GraphStore>) -> Self {
        Self { graph }
    }

    fn rows(&self, query: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let raw = self.graph.sparql_select(query)?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)?;
        Ok(parsed
            .get("results")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Every graph that carries validity metadata.
    fn validities(&self) -> anyhow::Result<BTreeMap<String, Validity>> {
        let query = format!(
            "SELECT ?g ?from ?to ?rec WHERE {{ \
             {{ ?g <{NS}validFrom> ?from }} UNION {{ ?g <{NS}validTo> ?to }} \
             UNION {{ ?g <{NS}recordedAt> ?rec }} }} LIMIT 20000"
        );
        let mut out: BTreeMap<String, Validity> = BTreeMap::new();
        for row in self.rows(&query)? {
            let Some(g) = row.get("g").and_then(|v| v.as_str()).map(plain) else {
                continue;
            };
            let entry = out.entry(g.clone()).or_insert(Validity {
                graph: g,
                valid_from: None,
                valid_to: None,
                recorded_at: None,
            });
            if let Some(v) = row.get("from").and_then(|v| v.as_str()) {
                entry.valid_from = Some(plain(v));
            }
            if let Some(v) = row.get("to").and_then(|v| v.as_str()) {
                entry.valid_to = Some(plain(v));
            }
            if let Some(v) = row.get("rec").and_then(|v| v.as_str()) {
                entry.recorded_at = Some(plain(v));
            }
        }
        Ok(out)
    }

    /// Named graphs holding assertions, whether or not they are described.
    fn all_graphs(&self) -> anyhow::Result<BTreeSet<String>> {
        Ok(self
            .rows("SELECT DISTINCT ?g WHERE { GRAPH ?g { ?s ?p ?o } } LIMIT 20000")?
            .iter()
            .filter_map(|r| r.get("g").and_then(|v| v.as_str()).map(plain))
            .collect())
    }

    /// Which graphs are in scope for a snapshot, and why.
    pub fn snapshot(&self, valid_at: Option<&str>, as_of: Option<&str>) -> anyhow::Result<String> {
        let validities = self.validities()?;
        let graphs = self.all_graphs()?;

        let mut in_scope = Vec::new();
        let mut excluded = Vec::new();
        for g in &graphs {
            match validities.get(g) {
                // Undescribed graphs are timeless and always in scope, so
                // this vocabulary is additive to an existing store.
                None => in_scope.push(serde_json::json!({"graph": g, "reason": "no validity recorded, timeless"})),
                Some(v) => {
                    let valid_ok = valid_at.is_none_or(|t| v.valid_at(t));
                    let recorded_ok = as_of.is_none_or(|t| v.recorded_by(t));
                    if valid_ok && recorded_ok {
                        in_scope.push(serde_json::json!({"graph": g, "valid": v.describe()}));
                    } else {
                        excluded.push(serde_json::json!({
                            "graph": g,
                            "valid": v.describe(),
                            "reason": if !valid_ok { "not true at that instant" } else { "not yet recorded then" },
                        }));
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "ok": true,
            "valid_at": valid_at,
            "as_of": as_of,
            "in_scope": in_scope,
            "excluded": excluded,
            "note": "Graphs without validity metadata are timeless and always in scope.",
        })
        .to_string())
    }

    /// Run a query against only the graphs in temporal scope.
    ///
    /// The query is wrapped rather than rewritten: its pattern is evaluated
    /// inside a GRAPH block restricted to the snapshot, which keeps arbitrary
    /// SPARQL working without parsing it.
    pub fn query_at(
        &self,
        pattern: &str,
        valid_at: Option<&str>,
        as_of: Option<&str>,
    ) -> anyhow::Result<String> {
        let snapshot: serde_json::Value = serde_json::from_str(&self.snapshot(valid_at, as_of)?)?;
        let graphs: Vec<String> = snapshot
            .get("in_scope")
            .and_then(|v| v.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r.get("graph").and_then(|g| g.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if graphs.is_empty() {
            return Ok(serde_json::json!({
                "ok": true,
                "results": [],
                "note": "no graphs in scope at that instant",
            })
            .to_string());
        }

        let values = graphs
            .iter()
            .map(|g| format!("<{g}>"))
            .collect::<Vec<_>>()
            .join(" ");
        let body = pattern.trim();
        let body = body
            .strip_prefix('{')
            .and_then(|b| b.strip_suffix('}'))
            .unwrap_or(body);
        let wrapped = format!(
            "SELECT * WHERE {{ VALUES ?__g {{ {values} }} GRAPH ?__g {{ {body} }} }} LIMIT 10000"
        );
        let raw = self.graph.sparql_select(&wrapped)?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)?;
        Ok(serde_json::json!({
            "ok": true,
            "valid_at": valid_at,
            "as_of": as_of,
            "graphs_in_scope": graphs.len(),
            "results": parsed.get("results").cloned().unwrap_or(serde_json::json!([])),
        })
        .to_string())
    }

    /// Disjointness violations, but only where the two assertions claim
    /// OVERLAPPING validity.
    ///
    /// This is the point of carrying valid time at all. Without it, a
    /// correction reads as a contradiction: an entity recorded as one thing
    /// until May and another thereafter trips every disjointness check, and
    /// the finding is noise. With it, a superseded statement is superseded,
    /// and only genuine disagreement about the same period survives.
    pub fn conflicts(&self) -> anyhow::Result<String> {
        let validities = self.validities()?;
        let rows = self.rows(
            "PREFIX owl: <http://www.w3.org/2002/07/owl#> \
             PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#> \
             SELECT DISTINCT ?s ?a ?b ?ga ?gb WHERE { \
               GRAPH ?ga { ?s a ?a } GRAPH ?gb { ?s a ?b } \
               FILTER(STR(?a) < STR(?b)) \
               ?a rdfs:subClassOf* ?da . ?b rdfs:subClassOf* ?db . \
               { ?da owl:disjointWith ?db } UNION { ?db owl:disjointWith ?da } \
             } LIMIT 5000",
        )?;

        let mut conflicts = Vec::new();
        let mut superseded = Vec::new();
        for row in &rows {
            let get = |k: &str| row.get(k).and_then(|v| v.as_str()).map(plain);
            let (Some(s), Some(a), Some(b), Some(ga), Some(gb)) = (
                get("s"), get("a"), get("b"), get("ga"), get("gb"),
            ) else {
                continue;
            };
            if ga == gb {
                continue;
            }

            let timeless = Validity {
                graph: String::new(),
                valid_from: None,
                valid_to: None,
                recorded_at: None,
            };
            let va = validities.get(&ga).unwrap_or(&timeless);
            let vb = validities.get(&gb).unwrap_or(&timeless);

            let entry = serde_json::json!({
                "subject": local(&s),
                "types": [local(&a), local(&b)],
                "periods": [va.describe(), vb.describe()],
                "graphs": [ga, gb],
            });
            if va.overlaps(vb) {
                conflicts.push(entry);
            } else {
                superseded.push(entry);
            }
        }

        Ok(serde_json::json!({
            "ok": true,
            "contradictions": conflicts,
            "contradiction_count": conflicts.len(),
            "superseded": superseded,
            "superseded_count": superseded.len(),
            "note": "contradictions claim overlapping validity and genuinely disagree. \
                     superseded pairs are corrections: one period ends where the other begins, \
                     which is a history rather than a conflict.",
        })
        .to_string())
    }
}
