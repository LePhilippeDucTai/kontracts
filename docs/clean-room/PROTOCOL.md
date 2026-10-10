# Clean-room protocol

kontract v0.3 reimplements, in pure Rust, the non-GUI core capabilities of a
proprietary contract-algebra pricing library (the *reference library*). This
protocol makes sure the result is an independent work: the reference library
informs **what** kontract must do, never **how** it is written.

The protocol applies to every contributor, human or agent, and to every unit
of the `clean-room-core` plan.

## 1. Roles

There are two rooms. A contributor works in exactly one room for a given unit
and never carries material from the dirty room to the clean room other than
through a published spec.

### Dirty-room spec authors

- May inspect the reference library, only to inventory its capabilities. The
  location of the reference checkout is given in the brief of dirty-room units
  only.
- Write functional specifications under `docs/clean-room/specs/` in their own
  words: capabilities, observable behaviour, conventions, inputs and outputs,
  edge cases and behavioural test scenarios.
- Tie every test scenario to a public reference (textbook, paper, ISDA or other
  market standard) or to a mathematical identity.
- Group capabilities so that they map onto the units of the plan.
- Never write Rust, never review Rust for the plan, and never write tests.

### Clean-room implementers

- Never open the reference library or any material derived from it other than
  the published specs: no source, tests, fixtures, documentation, build
  outputs, screenshots or recollections of them.
- Work only from the allowed sources listed in section 2.
- Write the Rust code, the Python bindings and the tests.

### Clean-room reviewers

- Follow the same restrictions as implementers.
- Check that the code follows the provenance rule (section 4) and the
  test-expectation rule (section 5), and that the lint passes.
- Reject a change whose structure or naming cannot be explained from the
  allowed sources.

## 2. Allowed sources

Clean-room work may use only:

- the specs under `docs/clean-room/specs/`;
- the architecture decision records under `docs/adr/`;
- the existing kontract code and documentation (v0.2 and later);
- public literature: textbooks, peer-reviewed papers, preprints, lecture
  notes and the documentation of open-source libraries under an
  OSI-approved licence (read for ideas, never copied);
- market standards: ISDA definitions and protocols, exchange rulebooks,
  central-bank and regulator publications, ISO standards (ISO 4217, ISO 8601).

## 3. Forbidden material

The following must never enter the clean room, whether in code, tests,
comments, docs, commit messages or review notes:

- source code of the reference library, in any language, even paraphrased
  line by line;
- its signatures and its module, type, constructor, field, exception and
  function names (see the denylist in `tools/clean_room_lint.py`);
- its comments, documentation text, error messages and log messages;
- its numeric expected values, test fixtures, market-data samples and
  calibration results;
- its file layout, used as a template for kontract's own layout;
- anything obtained by running the reference library.

Specs may describe behaviour, but must not contain any of the above. In a
spec, fenced blocks are allowed only as ` ```text ` or ` ```math `; any other
fence (including a fence with no language) is rejected.

## 4. Provenance rule

Every non-trivial formula, algorithm or convention in the Rust code carries a
doc comment that cites its public source: author(s), title, year and, where
useful, the equation, section or page. Examples of acceptable citations:

- a textbook chapter for a pricing formula;
- a paper for a discretisation scheme or a calibration method;
- the section of an ISDA or ISO document for a day-count or date convention.

If a piece of code has no public source because it is kontract's own design,
the doc comment says so and points to the ADR that records the design.

## 5. Test-expectation rule

Every expected value in a test is derived independently of the reference
library, by one of:

- a closed-form solution or a published reference value, with its citation;
- a mathematical identity (put–call parity, symmetry, monotonicity, limit
  cases, conservation of value under a change of numeraire, ...);
- an independent computation inside the test (a second method, a finer grid,
  a larger sample with a stated statistical tolerance);
- a hand computation documented in the test.

A number copied from the reference library, from its tests or from its output
is never an acceptable expected value, even if it is believed to be correct.

## 6. Spec format

Each spec under `docs/clean-room/specs/` is Markdown with these five level-2
headings, exactly as written:

- `## Capabilities`
- `## Behaviour`
- `## Edge cases`
- `## Test scenarios`
- `## References`

Check a spec before committing it:

```text
python3 tools/clean_room_lint.py --spec docs/clean-room/specs/<name>.md
```

## 7. Contamination lint

`tools/clean_room_lint.py` enforces the mechanical part of this protocol. It
uses the Python standard library only.

- `python3 tools/clean_room_lint.py` scans the text files under `src/`,
  `tests/`, `benches/`, `examples/`, `python/` and `docs/` for identifiers
  specific to the reference library (whole-word, case-insensitive). It prints
  `file:line` for each hit and exits 1 on any hit, 0 otherwise.
- `python3 tools/clean_room_lint.py --spec PATH` (repeatable) checks specs as
  described in section 6, plus the denylist. It exits 1 on any violation.
- `python3 tools/clean_room_lint.py --self-test` checks that the lint itself
  detects hits, missing headings and forbidden fences.

The denylist lives only in the lint script. It holds identifiers specific to
the reference library, not generic finance or mathematics vocabulary, not the
names of published models and not product or company names. Only a
dirty-room author may extend it. A clean scan does not prove independence:
the rules in sections 3 to 5 apply even where the lint cannot see.

## 8. Raising a doubt

When in doubt, stop and ask rather than guess.

1. Stop the work that depends on the doubtful material. Do not commit it.
2. Write down the doubt: the file and line, what the material is, where it
   came from and why it may break this protocol.
3. Raise it to the plan coordinator as a blocking decision on the unit (for an
   agent, end the unit as blocked with a structured decision).
4. If the material is already committed, do not rewrite history yourself; say
   so in the report so that the coordinator can decide how to remove it.
5. If a spec is ambiguous or incomplete, ask for a spec update from the dirty
   room. Do not fill the gap from memory of the reference library.

A clean-room contributor who has seen forbidden material for a capability
withdraws from implementing and reviewing that capability.
