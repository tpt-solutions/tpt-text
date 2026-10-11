# Unicode line break data

`tpt-text-linebreak` implements UAX #14 with generated tables. Nothing here depends on
the `unicode-linebreak` crate, which has been unreleased since 2023 and only
covers Unicode 15.0.0.

## Layout

- `crates/tpt-text-linebreak-gen/`: generator. Reads `LineBreak.txt`, `UnicodeData.txt`,
  `EastAsianWidth.txt` and `emoji-data.txt` and writes `crates/tpt-text-linebreak/src/tables.rs`.
- `crates/tpt-text-linebreak/src/tables.rs`: generated. Do not edit by hand.
- `crates/tpt-text-linebreak/src/rules.rs`: the UAX #14 rules, LB1 to LB31, applied in order
  as in revision 57 of the spec. This is the only hand-written part that changes with
  Unicode releases.
- `testdata/unicode/ucd/<version>/`: the UCD inputs for each version.
- `testdata/unicode/LineBreakTest.txt`: conformance cases for the current version.

## Updating to a new Unicode version

Run the script with the version, or with no argument for the latest published one:

    scripts/update-unicode.sh 18.0.0

It downloads the inputs, regenerates the tables and then runs
`cargo test --workspace`. The GitHub Actions workflow
`.github/workflows/unicode-update.yml` runs the same steps every March and October
and opens a pull request if the tests pass.

If the tests fail after an update, the new release changed a line breaking rule.
Compare the failing cases with the rule list in UAX #14 (`https://www.unicode.org/reports/tr14/`)
and change `rules.rs` to match.

## Known limits

- Rules are hand-coded; a new rule in a future UAX #14 revision needs a code change.
- The conformance test is the only check that the rules match the spec. If
  `LineBreakTest.txt` is not updated with the data, the test can pass while the
  tables are stale.
