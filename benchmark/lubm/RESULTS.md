# LUBM results

Measured on the official LUBM data generator (UBA 1.7, Lehigh University),
single node, Apple M-series laptop, release build, in-memory store.

These numbers exist so that claims about this engine can be checked rather
than believed, including the ones that do not flatter it. The reproduction
steps are at the bottom; disagreement is welcome and cheap to settle.

## Load and materialisation

| Dataset | Files | Triples | Load | Rate | OWL-RL | Inferred |
|---|---|---|---|---|---|---|
| LUBM(1) | 15 | 100,866 | 0.2 s | ~649k triples/s | 0.2 s | +37,942 |
| LUBM(10) | 189 | 1,273,246 | 2.0 s | ~625k triples/s | 2.9 s | +472,155 |

Loading is linear and fast, and materialisation of 1.27M triples in under
three seconds is respectable for a single node with no tuning. Nothing here
is a scale claim: LUBM(100) and beyond, and any concurrent workload, remain
unmeasured.

## The 14 queries, LUBM(1), after OWL-RL materialisation

`expected` is the published answer count for LUBM(1) under complete OWL
inference.

| Query | Expected | Ours | Complete | Note |
|---|---|---|---|---|
| Q1 | 4 | 4 | yes | |
| Q2 | 0 | 0 | yes | genuinely empty at LUBM(1), one university |
| Q3 | 6 | 6 | yes | |
| Q4 | 34 | 34 | yes | subclass and property inference |
| Q5 | 719 | 719 | yes | `memberOf` subproperty inference |
| Q6 | 7,790 | 5,916 | **no** | needs `Student` definition |
| Q7 | 67 | 59 | **no** | needs `Student` definition |
| Q8 | 7,790 | 5,916 | **no** | needs `Student` definition |
| Q9 | 208 | 103 | **no** | needs `Student` definition |
| Q10 | 4 | 0 | **no** | needs `Student` definition |
| Q11 | 224 | 224 | yes | transitive `subOrganizationOf` |
| Q12 | 15 | 0 | **no** | needs `Chair` definition |
| Q13 | 1 | 1 | yes | inverse property (`hasAlumnus`) |
| Q14 | 5,916 | 5,916 | yes | |

**8 of 14 complete. The 6 incomplete answers share one cause.**

LUBM defines two classes by equivalence to a restriction:

```
Student ≡ Person ⊓ ∃takesCourse.Course
Chair   ≡ Person ⊓ ∃headOf.Department
```

Recognising a `GraduateStudent` as a `Student` therefore requires reasoning
over an existential restriction on the *left* of an equivalence, which is
OWL 2 DL and outside the OWL-RL profile the materialiser implements. Every
incomplete row above is that single missing capability, not six separate
gaps: Q6, Q7, Q8, Q9 and Q10 all need `Student`, and Q12 needs `Chair`.

This is the honest statement of where the reasoner stands. It is a bounded,
nameable gap rather than a vague one, and closing it is a known piece of
work: recognise `C ≡ D ⊓ ∃P.E` definitions and add the corresponding
instance-recognition rule during materialisation.

## Query latency

The per-query timings from the harness are not reported here because they
are dominated by a measurement artifact: the CLI is stateless, so the
harness reloads and re-materialises the dataset for every query, and each
measurement is therefore ~190 ms of setup around a query that returns in
well under a millisecond. The unreasoned run, where no materialisation
happens, shows queries completing in 0.0 to 2.4 ms.

Measuring query latency properly needs the HTTP server with a warm store,
and concurrent-client throughput needs a load generator. Both are worth
doing before any performance claim is made against a commercial store, and
neither has been done.

## What benchmarking already fixed

Running this suite immediately found a defect that no unit test had:
`load` set no base IRI, so any RDF/XML document using relative IRIs failed
to parse at all. LUBM's generated data is exactly that shape, and loaded
zero triples before the fix. A document's own location is its base per RFC
3986; it is now set from the file path.

That is the argument for benchmarking in one paragraph: the suite paid for
itself before producing a single performance number.

## Reproducing

```bash
# 1. generator (Java), from the LUBM project
curl -sLO https://swat.cse.lehigh.edu/projects/lubm/uba1.7.zip
unzip -q uba1.7.zip
curl -sLO https://swat.cse.lehigh.edu/onto/univ-bench.owl

# 2. data. The generator writes names containing a literal backslash on
#    non-Windows platforms; move them into place afterwards.
java -cp classes edu.lehigh.swat.bench.uba.Generator \
     -univ 1 -index 0 -seed 0 \
     -onto http://swat.cse.lehigh.edu/onto/univ-bench.owl

# 3. run
python3 run_lubm.py --data data1 --reason --runs 3 --out results-lubm1.json
```

## Not yet measured

- LUBM(100) and LUBM(1000): where the single-node ceiling actually is
- Query latency with a warm store, and throughput under concurrency
- BSBM, SP2Bench, WatDiv: query optimisation under other shapes
- Any comparison against another store on the same hardware

Until those exist, this file says what it says and no more.
