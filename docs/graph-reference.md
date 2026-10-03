# Graph configuration reference

[Documentation index](index.md) · [Runnable examples](examples.md) · [Rust APIs](api-reference.md)

## Document and bindings

A `GraphSpec` declares `inputs`, `nodes`, `outputs` and `subgraphs`. `outputs`
is required; the other fields default to empty maps/lists. A reusable
`GraphBody` has the same fields except `subgraphs`; definitions live in one
flat map. Inputs map names to exact `ValueType` schemas, nodes are ordered
`NodeSpec` entries, and outputs map names to bindings. Definition references
must be acyclic, even when a definition is not invoked by the root.

Each node has a required `id` and `operation`, an `inputs` map (default empty),
and an optional `guard` binding (default absent). IDs are unique within a body.
Node/port/record names contain only ASCII letters, digits, `_` and `-`, and
must be nonempty. Declaration order breaks dependency ties; it does not
replace dependency analysis. Unused declared nodes still execute.

| Binding `kind` | Required fields | Optional fields | Meaning |
|---|---|---|---|
| `input` | `name` | `path: []` | A graph input in the current body |
| `node` | `node`, `port` | `path: []` | An output from a node in this body |
| `literal` | `value`, `value_type` | None | A checked Datum and explicit schema |

`path` is an ordered list of nested record field names. It cannot index a list
or implicitly unwrap Optional. A literal's `value` is a complete Datum:

```yaml
kind: literal
value:
  id: cutoff
  value: {kind: probability, value: 0.8}
value_type: {kind: probability}
```

Values use `{kind: TYPE, value: PAYLOAD}`. Schemas use `{kind: TYPE}` plus
`inner` for Optional, `item` for List, or `fields` for Record. Datum fields
are `id`, `value` and `sources` (default `[]`). Lists contain Datums; record
fields contain Values. See [data representation](api-reference.md#values-and-interchange).

YAML 1.2 and JSON share this schema. CLI extensions `.yaml`, `.yml` and `.json`
select the format; `--format` overrides discovery. In Rust,
`GraphSpec::parse` selects YAML and `parse_with_format` selects explicitly.
Unknown fields, duplicate keys, YAML merges/custom tags are refused; anchors
and aliases have zero budgets. Quote strings that look numeric or Boolean.
Question whitespace, including the newline introduced by `|`, affects replay.
Graph TOML is not supported. Configuration is capped at 1 MiB and parser
nesting at 128; value/type nesting is capped at 32, compilation at 4096 nodes
and mapped subgraph nesting at 16.

## Operation ports

Unless stated otherwise, every operation returns a port named `result`.
Required inputs are exact: extra or missing ports refuse compilation. Types
never coerce automatically. `T`, `L` and `R` below stand for declared types.

| `kind` | Inputs | Result | Configuration fields | Complete example |
|---|---|---|---|---|
| `record` | Any named typed operands | Record of those fields | None | [facts](../examples/reference/facts.yaml) |
| `list` | Named operands all of type T | List(T) | `item_type`, `order` | [facts](../examples/reference/facts.yaml) |
| `code` | Primitive signature | Primitive outputs | `primitive`, `params` (default `{}`) | [native](../examples/reference/native.yaml), [Rust host](../examples/reference.rs) |
| `questions` | None | Questions | `questions` | [questions](../examples/reference/questions.yaml) |
| `ask` | `state: Record`, `questions: Questions` | `answers: Answers`, `model: Text` | `backend` | [questions](../examples/reference/questions.yaml) |
| `map` | `items: List(T)` and subgraph captures | List(U) | `graph` | [collections](../examples/reference/collections.yaml) |
| `filter` | `items: List(T)`, `mask: List(Boolean)` | List(T) | None | [collections](../examples/reference/collections.yaml) |
| `pairs` | `left: List(L)`, `right: List(R)` | List(Record(left: L, right: R)) | None | [collections](../examples/reference/collections.yaml) |
| `join` | `left`, `right`: record lists | List(Record(left, right)) | `left_key`, `right_key` | [collections](../examples/reference/collections.yaml) |
| `collect` | `items: List(List(T))` | List(T) | None | [collections](../examples/reference/collections.yaml) |
| `and`, `or` | `a: Boolean`, `b: Boolean` | Boolean | None | [facts](../examples/reference/facts.yaml) |
| `not` | `value: Boolean` | Boolean | None | [facts](../examples/reference/facts.yaml) |
| `compare` | `a`, `b`: same scalar type | Boolean | `comparator` | [facts](../examples/reference/facts.yaml) |
| `probability` | `answers: Answers` | Probability | `question`, `labels` | [questions](../examples/reference/questions.yaml) |
| `degree` | `value: Probability` or Number | Degree | None | [strengths](../examples/reference/strengths.yaml) |
| `reduce` | `values: List(Degree)`; weighted mean also `weights: List(Number)` | Degree | `reducer`, `empty` | [strengths](../examples/reference/strengths.yaml) |
| `complement` | `value: Degree` | Degree | None | [strengths](../examples/reference/strengths.yaml) |
| `coalesce` | `value: Optional(T)`, `default: T` | T | None | [guards](../examples/reference/guards.yaml) |

## Assemble data and call Rust

`record` preserves field names and types; an empty operand map creates an
empty record. `list.order` names every operand exactly once, with no repeated
names. Its `item_type` is required even for an empty list. Each operand becomes
an identified occurrence, so two operands referring to the same datum produce
different occurrence IDs. Nested item IDs remain part of their values.

`code` resolves the named Primitive during compilation. Register it in Rust
before calling `compile`; the stock CLI cannot load application-native code.
`params` are typed Values owned by your application, not graph inputs and not
part of the Primitive port signature. Validate their meaning in the primitive.
Returned ports and values are checked against its captured Signature. The
[TextLength example](../examples/reference.rs) requires `unit: {kind: text,
value: bytes}`, checks cancellation and returns a finite Number.

## Questions and inference

`questions` creates an ordered block of `NamedQuestion {id, question}`. IDs
must be nonempty and unique; instructions must be nonblank. Choice labels are
nonempty and distinct; choice and score spaces require at least two entries.

```yaml
questions:
  - id: observable
    question:
      kind: boolean
      instructions: Does the text name an observable action?
      yes: Observable action
      no: No observable action
  - id: role
    question:
      kind: choice
      instructions: Which role is present?
      options:
        - {label: actor, description: An actor}
        - {label: other, description: Another role}
  - id: quality
    question:
      kind: score
      instructions: How explicit is the action?
      levels: [Absent, Implied, Explicit]
```

Boolean outcome labels are `false` and `true`. Choice labels are configured
strings. Score outcome labels are zero-based decimal strings (`"0"`, `"1"`,
`"2"` here), and the expected score lies in `[0, levels.len()-1]`.

`ask` makes one explicit batch request through a registered backend. There is
no implicit batching, repair or retry. `state` must be a Record; source sidecars
and Datum IDs are omitted from its plain model JSON. Put any identity needed
by the question in a record field. The runtime validates all answers and the
actual model identity before downstream use. See [answer semantics](api-reference.md#questions-answers-and-distributions)
and [adapter setup](api-reference.md#model-adapters).

`probability` sums known mass for nonempty distinct `labels` from one question.
Missing outcome mass returns `UnsupportedDistribution`, rather than zero.
Approximate complete distributions normalize only under an explicit backend
policy. Choice confidence and expected ordinal scores are not outcome masses.

## Collections and reusable graphs

A mapped subgraph requires an `item` input matching the collection element
schema and exactly one output named `result`. All its other inputs are captures:
provide corresponding bindings on the map node. The collections example
captures a cutoff. A map preserves each outer item ID and order while changing
its value; this makes its Boolean results suitable as a filter mask or its
numeric outputs suitable as weights for another map of the same items.

Filter masks must have exactly the same item IDs as the input collection,
regardless of mask order. The result preserves selected item order, IDs and
sources. Independently decoded plain lists get different IDs; deriving masks
from a shared collection avoids membership errors.

`pairs` expands the Cartesian product in left order, then right order.
`join` emits all matches with the same ordering. Keys name direct record
fields of the same Text, Boolean or Number type. Duplicate keys yield
many-to-many results; unmatched items are omitted. Pair identities derive
from both item IDs and pair sources merge both sides. Expansion is budgeted
before allocation. Probability/Degree are not join key types.

`collect` flattens one level in outer/inner order and preserves inner identities.
Inner IDs must remain globally unique in the flattened result; collisions
return `DuplicateId`. Empty lists remain typed and are valid for map/filter,
pairs/join and collect.

## Logic and degrees

`and` and `or` operate on already computed Booleans. They do not skip upstream
work. Use guards on expensive nodes to control execution.

`compare.comparator` is `equal`, `less`, `less_equal`, `greater` or
`greater_equal`. Both operands must have the same scalar type. Equality
supports Boolean, Text, Number, Probability and Degree; ordering supports
Number, Probability and Degree. Equality is exact; comparisons never cross
semantic numeric types. The facts example compares 2 with 3; the
[threshold example](../examples/graphs/review.yaml) shows a Probability cutoff.

`degree` explicitly converts a Number or Probability in `[0,1]` into a
heuristic Degree. `complement` computes `1 - degree`. Degrees describe your
policy; reductions make no claim about joint probability or correctness.

`reduce.reducer` is `min`, `max` or `weighted_mean`. All require an `empty`
Degree returned for an empty values collection. Weighted mean requires exactly
matching weight IDs, finite nonnegative weights, and a positive total for a
nonempty collection. Weight order need not match value order. The strengths
example maps shared records into aligned values and weights: values 0.8, 0.4,
0.6 with weights 2, 1, 1 yield min 0.4, max 0.8, mean 0.65 and complement 0.35.

## Guards, absence and failure

A node guard must be Boolean. A false guard skips operation work and returns
Optional absence on every output. A true guard wraps each normal output in
Optional presence. Guarded schemas are Optional regardless of the runtime
condition. Use `coalesce` with a same-inner-type default before connecting to
a consumer that needs a concrete value. Guard evaluation happens before
operation input resolution; the backend must still be registered at engine
construction. See [guards.yaml](../examples/reference/guards.yaml).

`coalesce` selects its present inner value or its supplied default. Both
upstream producers may already have run; it does not make default computation
lazy. Check [runtime/errors](api-reference.md#execution-traces-and-errors) for
limits, cooperative native cancellation, deterministic traces and partial
failure evidence.
