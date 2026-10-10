#!/usr/bin/env python3
"""Clean-room contamination lint for kontract.

Usage:
    python3 tools/clean_room_lint.py               scan the repository tree
    python3 tools/clean_room_lint.py --spec PATH   check dirty-room spec files
    python3 tools/clean_room_lint.py --self-test   run the built-in self-test

Mode (a), no argument: scan the text files under src/, tests/, benches/,
examples/, python/ and docs/ (relative to the repository root, i.e. the parent
of this script's directory) for identifiers that are specific to the reference
library. Every hit is printed as ``file:line: identifier``. Exit code 1 if any
hit is found, 0 otherwise. Missing directories are skipped.

Mode (b), ``--spec PATH`` (repeatable): every file must exist, contain the five
required level-2 headings, contain no fenced code block other than ``text`` or
``math`` fences, and pass the denylist. Exit code 1 on any violation.

Mode (c), ``--self-test``: build synthetic trees and specs in a temporary
directory and assert that every detection works. Exit code 0 only if all
assertions hold.

The policy this tool enforces is described in docs/clean-room/PROTOCOL.md.
Only the Python standard library is used.
"""

from __future__ import annotations

import argparse
import io
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

SCAN_DIRS = ("src", "tests", "benches", "examples", "python", "docs")
SKIP_DIR_NAMES = frozenset(
    {
        "target",
        ".venv",
        "venv",
        ".git",
        "__pycache__",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "node_modules",
    }
)
BINARY_SNIFF_BYTES = 8192

REQUIRED_HEADINGS = (
    "## Capabilities",
    "## Behaviour",
    "## Edge cases",
    "## Test scenarios",
    "## References",
)
ALLOWED_FENCE_INFO = frozenset({"text", "math"})

# Identifiers specific to the reference library: internal module, type,
# constructor, exception and function names. Generic finance or mathematics
# vocabulary (curve, calendar, heston, black_scholes, market_data, ...), names
# of published models and the product or company names are deliberately left
# out. Matching is whole-word and case-insensitive. Grouped by capability area.
DENYLIST = frozenset(
    {
        # --- contract algebra: modules -------------------------------------
        "algebra_compilemc",
        "algebra_compilepde",
        "algebra_discontinuity",
        "algebra_matcher",
        "algebra_wf",
        "contract_algebra_ct",
        "contract_algebra_semantics",
        "managed_contract_core",
        "managed_contract_storage",
        "amc_contract_decomposition",
        # --- contract algebra: types, constructors, functions ---------------
        "aobservable",
        "untyped_observable",
        "continuous_monitoring_explanation",
        "fixings_information",
        "pending_fixing_mapper",
        "ratio_method_parameters",
        "simple_barrier_elt",
        "subcontract_identifier",
        "switch_identifier",
        "instrument_annotation",
        "fold_uthing_nonrec",
        "fold_uthing_nonrec_with_depth",
        "iter_uthing_nonrec",
        "map_uthing_nonrec",
        "map_uthing_nonrec_with_depth",
        "memoize_with_depth_keep_const",
        "memoize_with_env",
        "rewrite_no_identify",
        "wrap_physical_equity",
        "unwrap_physical_equity",
        "aggregate_internal",
        "credit_default_internal",
        "identify_internal",
        "lazy_ifv",
        "is_constant_abs",
        "approx_of_constant",
        "variant_of_constant",
        "event_contract_quantity_per_trade_date",
        "event_contract_quantity_per_value_date",
        "treenode_of_leg_path",
        "pa_of_attributes",
        "continuous_condition_of_condition",
        "extract_credit_spread_identifier",
        "credit_spread_identifier",
        "array_sort_subset",
        "readable_print_parameters",
        "string_of_thing",
        "print_thing",
        "acquire_when",
        "acquire_incompatible_horizons",
        "anytime_incompatible_horizons",
        "until_incompatible_horizons",
        "duplicate_either_label_or_default",
        "empty_either",
        "bad_observable_type",
        "observable_fixed_before_definition_domain",
        "irrelevant_handle_event",
        "irrelevant_effects",
        "checked_and_fixed",
        "first_switch_fixed",
        "multi_applied_hit",
        "observable_type_of_stype",
        "equality_of_observable_type",
        "ot_float",
        "coupon_one",
        "premium_one",
        "principal_one",
        "settlement_one",
        "one_function",
        "one_with_attributes",
        "flow_with_attributes",
        "instrument_untyped",
        "was_instrument_",
        "obs_of_bool",
        "obs_of_date",
        "obs_of_float",
        "obs_of_int",
        "obs_of_string",
        # --- core library modules -------------------------------------------
        "mlfi_acontract",
        "mlfi_array",
        "mlfi_base64",
        "mlfi_calendar_diff",
        "mlfi_check_customization",
        "mlfi_cipher",
        "mlfi_contract",
        "mlfi_contract_calendar_internal",
        "mlfi_contract_check",
        "mlfi_contract_combinators",
        "mlfi_contract_ct",
        "mlfi_contract_decomposition",
        "mlfi_contract_manage",
        "mlfi_contract_new_storage",
        "mlfi_contract_ops",
        "mlfi_contract_pretty_printer",
        "mlfi_contract_printer",
        "mlfi_contract_printer_readable",
        "mlfi_contract_simplifier",
        "mlfi_contract_summary",
        "mlfi_contract_types",
        "mlfi_core_install_printers",
        "mlfi_date",
        "mlfi_dynamic",
        "mlfi_float",
        "mlfi_hashtbl",
        "mlfi_hashtbl_opt",
        "mlfi_isdatypes",
        "mlfi_json",
        "mlfi_list",
        "mlfi_marshal",
        "mlfi_ops",
        "mlfi_option",
        "mlfi_proplist",
        "mlfi_serialize",
        "mlfi_serialize_foreign",
        "mlfi_sets_maps",
        "mlfi_static_data",
        "mlfi_storage",
        "mlfi_ttypes",
        "mlfi_type_path",
        "mlfi_variant_diff",
        "mlfi_variant_lexer",
        "mlfi_variant_parser",
        "mlfi_variant_printer",
        "mlfi_wlazy",
        "mlfi_xtypes",
        "variant_serialize",
        "patset",
        "landmark_graph",
        # --- dates and calendars --------------------------------------------
        "add_act_365",
        "sub_act_365",
        "date_of_explicit_or_shift",
        "explicit_or_shift",
        "date_of_string_fuzzy",
        "date_of_lexbuf_fuzzy",
        "date_list_of_lexbuf_fuzzy",
        "values_at_days_between_inclusive",
        "normalize_date_at_midnight",
        "date_column_from_pdf_selection",
        "dutch_month",
        "french_month",
        "english_month",
        "adjust_then_offset",
        "date_adjustment_from_old_business_day_convention",
        "old_date_adjustment",
        "old_days_offset",
        "old_days_offset_of_triplet",
        "old_day_type",
        "old_from_new_business_day_convention",
        "days_offset_of_days_and_calendars",
        "days_offset_mapper",
        "day_type_mapper",
        "business_day_convention_mapper",
        "business_days_in_period_with_multiplicities",
        "days_months_choice",
        "next_day_month_maturity",
        "monthly_yearly_roll_convention",
        "parametric_to_adjusted_dates",
        "parametric_period_dates",
        "parametric_reset_dates",
        "cms_underlying_swap_period_dates",
        # --- static data, conventions and market data -----------------------
        "basefin",
        "basis_swap_rule_mapper",
        "basis_swap_term_of_swap_term",
        "domestic_settlement_basis_of_bond_class",
        "international_settlement_basis_of_bond_class",
        "coupon_frequency_of_bond_class",
        "calendar_of_bond_class",
        "currency_of_bond_class",
        "day_count_convention_of_bond_class",
        "bond_class_of_string",
        "asset_libor_repo_spread_kind",
        "credit_spread_or_fix_funding",
        "check_market_data_item_unicity",
        "check_market_data_items_validity",
        "editable_market_data_item",
        "editable_market_data_item_desc",
        "market_data_item_desc",
        "market_data_item_desc_of_name",
        "filter_preprocessed_market_data_items",
        "filter_market_underlying_prim",
        "filter_yield_curve_tag",
        "build_union_query",
        "add_curve_tag",
        "apply_curve_tag",
        "has_only_one_tag",
        "marketdataa1",
        "marketdataa3",
        "marketdataitems",
        "market_data_definitions",
        "cds_index_default_proportion",
        "cds_index_nb_defaults",
        "caps_and_swaptions_selection",
        "call_or_put_selection",
        "last_discrete_dividends_date",
        "inflation_indexes_of_market_data",
        "currencies_of_market_data_items",
        "correlation_of_market_data",
        "constant_volatility_of_asset",
        "constant_volatility_of_fx",
        # --- curves ---------------------------------------------------------
        "curve_dlogpdtau",
        "curve_logp",
        "curve_of_curve_descr",
        "curve_of_curve_description",
        "discount_curve_and_zcs",
        "discount_curve_descr_of_yield_curves",
        "forward_curve_descr_of_yield_curves",
        "discount_tenor_for_rate_calibration",
        "needed_curves_and_market_data_discount_tenor_for_rate_calibration",
        "funding_quotes_to_z_spreads",
        "simple_yield_curves_of_zcs",
        "real_curve_of_inflation_market_data",
        "real_curve_of_yield_curves",
        "multi_curve_enabled_for_currency",
        "pillars_rule_of_interpolation",
        "extracts_or_fail",
        "build_curves_with_ignored_quotes",
        "build_curves_for_single_currency",
        "simple_curves_from_fx",
        "closest_discount_curve",
        "closest_curve_tenor",
        "get_discount_curve_from_asset_name",
        "to_single_discount_tenor",
        "discount_curve_of_curve_tenor",
        "forward_curve_spread_array",
        # --- numerics -------------------------------------------------------
        "mlfi_math",
        "mlfimath",
        "mlfi_erfc",
        "mlfi_linalg",
        "mlfi_sobol",
        "mlfi_intel_math",
        "mlfi_mlvalues",
        "mlfi_to_r",
        "mllapack",
        "mllpsolve",
        "mlquadratic_prog",
        "mlcmpfit",
        "fast_ugaussian_p",
        "taylor_protected",
        "taylor_protected_1d",
        "taylor_protected_1d_multidim",
        "taylor_protected_2d",
        "real_sotta",
        "local_stable_quadratic_interpoler",
        "stable_quadratic_interpolation_for_pde",
        "build_stable_quadratic_interpolation",
        "build_monotonic_cubic_spline_interpolation",
        "piecewise_affine_approximation_knots",
        "qpois_def",
        "int_nb_points",
        "fold_c_n_k",
        # --- Monte-Carlo and AMC engines ------------------------------------
        "mlfi_bytecode",
        "mlfi_compilemc",
        "mlfi_compilemc_c",
        "mlfi_compilemc_cg",
        "mlfi_compilemc_code",
        "mlfi_compilemc_expr",
        "mlfi_compilemc_main",
        "mlfi_compilemc_register",
        "mlfi_mc",
        "mc_generic",
        "compute_greeks_weight",
        "default_greek_parameters_with_delta_vega_idx",
        "global_nb_paths",
        "model_for_malliavin",
        "split_cv_idx_and_beta_and_strikes",
        "underlying_print_hook",
        "vanilla_pricer_results",
        "variant_parameters_indexes",
        "build_variant_parameters",
        "relocate_variants",
        "variant_rewriter",
        "link_into_pricer",
        "check_option_structure",
        "unavailable_numeraire",
        "unavailable_underlying",
        "not_supported_by_model",
        "custom_pricer_ticker",
        "deterministic_pricer_ticker",
        "standard_underlying_ticker",
        "register_custom_compiler",
        "extract_original_pay_date",
        # --- PDE engine -----------------------------------------------------
        "pde_contracts",
        "pde_contracts_core",
        "pde_contracts_inspector",
        "pde_contracts_printer",
        "pde_contracts_storage",
        "pde_interpret",
        "pde_optimiser",
        "pde_generic",
        "iter_contract_nonrec",
        "iter_expr_nonrec",
        "map_contract_nonrec",
        "map_expr_nonrec",
        "underlyings_of_contract",
        "sampler_of_pathdep_interpolation",
        "ensure_no_underlying",
        "equity_custom_pricer",
        "fx_custom_pricer",
        "gk_custom_pricer",
        "interest_rate_custom_pricer",
        "ir_with_currency_custom_pricer",
        "generic_equity_parameters",
        "generic_fx_parameters",
        "commodity_2d_underlying",
        "notify_result_2d",
        "notify_result_3d",
        "from_fun_with_idx",
        # --- models, calibration and pricing glue ---------------------------
        "mlfi_pricing",
        "mlfi_model_support",
        "model_md_transformation",
        "mini_blackscholes",
        "blackscholes_common",
        "blackscholes_minmax",
        "jy_common",
        "infla_common",
        "lfm_common",
        "hwev_ts",
        "pde_hwev",
        "pde_ofqg",
        "pde_hybrid_blackscholes_garmankohlagen",
        "pde_hybrid_blackscholes_hullwhite",
        "pde_hybrid_local_volatility_gk",
        "pde_hybrid_local_volatility_hw",
        "super_hybrid",
        "ird_models",
        "ird_volatility_cube",
        "build_generic_model_parameters",
        "generic_asset_models",
        "mk_get_fx_spot",
        "build_other_spread_curves",
    }
)

_IDENT_CHARS = "A-Za-z0-9_"
DENY_RE = re.compile(
    r"(?<![" + _IDENT_CHARS + r"])("
    + "|".join(sorted((re.escape(w) for w in DENYLIST), key=len, reverse=True))
    + r")(?![" + _IDENT_CHARS + r"])",
    re.IGNORECASE,
)
FENCE_RE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")


def is_text_file(path: Path) -> bool:
    try:
        with path.open("rb") as handle:
            chunk = handle.read(BINARY_SNIFF_BYTES)
    except OSError:
        return False
    if b"\0" in chunk:
        return False
    try:
        chunk.decode("utf-8")
    except UnicodeDecodeError as err:
        # A multi-byte sequence cut at the sniff boundary is still text.
        return err.start >= len(chunk) - 3
    return True


def iter_text_files(root: Path):
    for name in SCAN_DIRS:
        base = root / name
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            rel_parts = path.relative_to(root).parts
            if any(part in SKIP_DIR_NAMES for part in rel_parts):
                continue
            if path.is_file() and not path.is_symlink() and is_text_file(path):
                yield path


def denylist_hits(path: Path):
    """Return (line_number, identifier) pairs for every denylist hit."""
    text = path.read_text(encoding="utf-8", errors="replace")
    return [
        (lineno, match.group(1))
        for lineno, line in enumerate(text.splitlines(), start=1)
        for match in DENY_RE.finditer(line)
    ]


def display(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def scan_tree(root: Path, out=sys.stdout) -> int:
    """Scan the repository tree; return the number of hits."""
    hits = 0
    for path in iter_text_files(root):
        for lineno, ident in denylist_hits(path):
            print(f"{display(path, root)}:{lineno}: denylisted identifier '{ident}'", file=out)
            hits += 1
    return hits


def spec_violations(path: Path):
    """Return a list of (line_number, message) violations for one spec file."""
    if not path.is_file():
        return [(0, "spec file does not exist")]
    if not is_text_file(path):
        return [(0, "spec file is not UTF-8 text")]
    lines = path.read_text(encoding="utf-8").splitlines()
    violations = []
    headings = set()
    open_fence = None  # (char, length) while inside a fenced block
    for lineno, line in enumerate(lines, start=1):
        fence = FENCE_RE.match(line)
        if open_fence is not None:
            if fence and fence.group(1)[0] == open_fence[0] and len(fence.group(1)) >= open_fence[1] and not fence.group(2).strip():
                open_fence = None
            continue
        if fence:
            marker, info = fence.group(1), fence.group(2).strip()
            lang = info.split()[0].lower() if info else ""
            if lang not in ALLOWED_FENCE_INFO:
                shown = lang or "(no language)"
                violations.append((lineno, f"forbidden code fence '{shown}': only text or math fences are allowed"))
            open_fence = (marker[0], len(marker))
            continue
        if line.startswith("## "):
            headings.add(line.rstrip())
    if open_fence is not None:
        violations.append((len(lines), "unterminated code fence"))
    violations.extend(
        (0, f"missing required heading '{heading}'")
        for heading in REQUIRED_HEADINGS
        if heading not in headings
    )
    violations.extend(
        (lineno, f"denylisted identifier '{ident}'") for lineno, ident in denylist_hits(path)
    )
    return sorted(violations)


def check_specs(paths, root: Path, out=sys.stdout) -> int:
    """Check spec files; return the number of violations."""
    total = 0
    for raw in paths:
        path = Path(raw)
        for lineno, message in spec_violations(path):
            print(f"{display(path.resolve(), root)}:{lineno}: {message}", file=out)
            total += 1
    return total


# --------------------------------------------------------------------------
# Self-test
# --------------------------------------------------------------------------

CLEAN_SPEC = """# Example capability spec

## Capabilities

- Year fractions under the usual day-count conventions.

## Behaviour

The year fraction between equal dates is zero.

```text
yf(d, d) = 0
```

## Edge cases

- End-of-month start dates.

## Test scenarios

```math
yf(d_1, d_3) = yf(d_1, d_2) + yf(d_2, d_3)
```

## References

- ISDA 2006 Definitions, section 4.16.
"""


def _sample_identifier() -> str:
    return sorted(DENYLIST)[0]


def run_self_test() -> int:
    failures = []

    def check(condition: bool, label: str) -> None:
        print(f"[{'ok' if condition else 'FAIL'}] {label}")
        if not condition:
            failures.append(label)

    ident = _sample_identifier()
    sink = open(os.devnull, "w", encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="clean_room_lint_") as tmp, sink:
        root = Path(tmp)

        # Denylist matching on lines.
        check(DENY_RE.search(f"let x = {ident}(y);") is not None, "denylist matches a whole word")
        check(DENY_RE.search(f"use foo::{ident.upper()};") is not None, "denylist matching is case-insensitive")
        check(DENY_RE.search(f"x{ident}") is None, "denylist ignores a longer identifier (prefix)")
        check(DENY_RE.search(f"{ident}_x") is None, "denylist ignores a longer identifier (suffix)")
        check(DENY_RE.search("let curve = market_data.calendar;") is None, "generic finance words are not denylisted")

        # Tree scan.
        tree = root / "tree"
        (tree / "src").mkdir(parents=True)
        (tree / "src" / "clean.rs").write_text("pub fn price() -> f64 { 0.0 }\n", encoding="utf-8")
        check(scan_tree(tree, out=sink) == 0, "clean tree has no hits")
        (tree / "docs").mkdir()
        (tree / "docs" / "note.md").write_text(f"line one\nuses {ident} here\n", encoding="utf-8")
        (tree / "target").mkdir()
        (tree / "target" / "gen.rs").write_text(f"{ident}\n", encoding="utf-8")
        (tree / "python" / ".venv").mkdir(parents=True)
        (tree / "python" / ".venv" / "lib.py").write_text(f"{ident}\n", encoding="utf-8")
        (tree / "src" / "blob.bin").write_bytes(b"\0\1" + ident.encode() + b"\0")
        (tree / "tests").mkdir()
        (tree / "tests" / "hit.rs").write_text(f"// ok\n// ok\nlet a = {ident};\n", encoding="utf-8")
        report = io.StringIO()
        hits = scan_tree(tree, out=report)
        lines = sorted(report.getvalue().splitlines())
        check(hits == 2, "tree scan finds exactly the two text hits")
        check(any(l.startswith("docs/note.md:2:") for l in lines), "hit reported as file:line (docs)")
        check(any(l.startswith("tests/hit.rs:3:") for l in lines), "hit reported as file:line (tests)")
        check(not any("target" in l or ".venv" in l or "blob.bin" in l for l in lines),
              "target/, .venv/ and binary files are skipped")

        # No-argument CLI on a copy of the script placed inside the tree.
        (tree / "tools").mkdir()
        script_copy = tree / "tools" / Path(__file__).name
        shutil.copyfile(__file__, script_copy)
        proc = subprocess.run([sys.executable, str(script_copy)], capture_output=True, text=True, cwd=tmp)
        check(proc.returncode == 1 and "tests/hit.rs:3:" in proc.stdout, "CLI scan exits 1 and prints hits")
        (tree / "docs" / "note.md").unlink()
        (tree / "tests" / "hit.rs").unlink()
        proc = subprocess.run([sys.executable, str(script_copy)], capture_output=True, text=True, cwd=tmp)
        check(proc.returncode == 0, "CLI scan exits 0 on a clean tree")
        empty = root / "empty"
        (empty / "tools").mkdir(parents=True)
        shutil.copyfile(__file__, empty / "tools" / Path(__file__).name)
        proc = subprocess.run([sys.executable, str(empty / "tools" / Path(__file__).name)],
                              capture_output=True, text=True, cwd=tmp)
        check(proc.returncode == 0, "CLI scan skips missing directories")

        # Spec checks.
        specs = root / "specs"
        specs.mkdir()
        clean = specs / "clean.md"
        clean.write_text(CLEAN_SPEC, encoding="utf-8")
        check(spec_violations(clean) == [], "clean spec passes")

        missing = specs / "missing_heading.md"
        missing.write_text(CLEAN_SPEC.replace("## Edge cases\n", "## Corner cases\n"), encoding="utf-8")
        found = spec_violations(missing)
        check(any("missing required heading '## Edge cases'" in m for _, m in found), "missing heading detected")

        level3 = specs / "wrong_level.md"
        level3.write_text(CLEAN_SPEC.replace("## References\n", "### References\n"), encoding="utf-8")
        check(any("'## References'" in m for _, m in spec_violations(level3)), "heading at wrong level detected")

        in_fence = specs / "heading_in_fence.md"
        in_fence.write_text(
            CLEAN_SPEC.replace("## References\n", "```text\n## References\n```\n"), encoding="utf-8"
        )
        check(any("'## References'" in m for _, m in spec_violations(in_fence)),
              "heading inside a fence does not count")

        for lang, body in (("rust", "fn main() {}"), ("", "plain block"), ("python", "x = 1"), ("ocaml", "let x = 1")):
            bad = specs / f"fence_{lang or 'bare'}.md"
            bad.write_text(CLEAN_SPEC + f"\n```{lang}\n{body}\n```\n", encoding="utf-8")
            check(any("forbidden code fence" in m for _, m in spec_violations(bad)),
                  f"forbidden fence detected ({lang or 'no language'})")
        tilde = specs / "fence_tilde.md"
        tilde.write_text(CLEAN_SPEC + "\n~~~rust\nfn main() {}\n~~~\n", encoding="utf-8")
        check(any("forbidden code fence" in m for _, m in spec_violations(tilde)), "forbidden tilde fence detected")

        hit_spec = specs / "hit.md"
        hit_spec.write_text(CLEAN_SPEC.replace("is zero.", f"is zero, see {ident}."), encoding="utf-8")
        check(any("denylisted identifier" in m for _, m in spec_violations(hit_spec)), "denylist hit in spec detected")

        absent = specs / "absent.md"
        check(any("does not exist" in m for _, m in spec_violations(absent)), "missing spec file detected")

        # --spec CLI.
        proc = subprocess.run([sys.executable, __file__, "--spec", str(clean)], capture_output=True, text=True)
        check(proc.returncode == 0, "CLI --spec exits 0 on a clean spec")
        proc = subprocess.run([sys.executable, __file__, "--spec", str(clean), "--spec", str(missing)],
                              capture_output=True, text=True)
        check(proc.returncode == 1, "CLI --spec exits 1 when one of several specs fails")
        proc = subprocess.run([sys.executable, __file__, "--spec", str(absent)], capture_output=True, text=True)
        check(proc.returncode == 1, "CLI --spec exits 1 on a missing file")

    if failures:
        print(f"self-test FAILED: {len(failures)} assertion(s)")
        return 1
    print("self-test passed")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Clean-room contamination lint (see docs/clean-room/PROTOCOL.md).")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--spec", action="append", metavar="PATH", help="check a dirty-room spec file (repeatable)")
    group.add_argument("--self-test", action="store_true", help="run the built-in self-test")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_test()
    if args.spec:
        violations = check_specs(args.spec, REPO_ROOT)
        print(f"clean-room spec check: {len(args.spec)} file(s), {violations} violation(s)")
        return 1 if violations else 0
    hits = scan_tree(REPO_ROOT)
    print(f"clean-room scan: {hits} denylist hit(s)")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
