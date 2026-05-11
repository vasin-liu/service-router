#!/usr/bin/env python3
"""Build a §9 regression summary (Markdown) from the five release-acceptance CLI JSON files (§7).

Reads the default directory written by docs/release-acceptance.sh / .ps1:
  artifacts/release-acceptance/

Expected inputs (same names as CI bundle release-acceptance-json):
  check-config.json, doctor.json, doctor-probe.json,
  route-explain-smoke.json, config-snapshot.json

When those exist, docs/release-acceptance.sh / docs/release-acceptance.ps1 also
write section-9-summary.generated.md in the same directory (same content as
redirecting this script's stdout to that file).

Global gates (cargo check / test) are not recorded in these JSON files; pass
--global-gates or set SERVICE_ROUTER_ACCEPTANCE_GLOBAL_GATES when known.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ARTIFACT_NAMES = (
    "check-config.json",
    "doctor.json",
    "doctor-probe.json",
    "route-explain-smoke.json",
    "config-snapshot.json",
)


def _load_json(path: Path) -> tuple[Any | None, str | None]:
    if not path.is_file():
        return None, "missing"
    try:
        data = path.read_bytes()
    except OSError as e:
        return None, f"read error: {e}"
    try:
        raw = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        try:
            raw = data.decode("utf-16")
        except UnicodeDecodeError as e:
            return None, f"decode error: {e}"
    try:
        return json.loads(raw), None
    except json.JSONDecodeError as e:
        return None, f"invalid JSON: {e}"


def _git_head() -> str:
    env = (os.environ.get("GIT_COMMIT") or os.environ.get("CI_COMMIT_SHA") or "").strip()
    if env:
        return env
    try:
        r = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            timeout=8,
            check=False,
        )
        if r.returncode == 0 and r.stdout:
            return r.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        pass
    return ""


def _count_unreachable_probes(doc: dict[str, Any]) -> int:
    n = 0
    for key in ("registry_endpoint_probe", "upstream_probe"):
        arr = doc.get(key)
        if not isinstance(arr, list):
            continue
        for item in arr:
            if isinstance(item, dict) and item.get("reachable") is False:
                n += 1
    return n


def _cli_gates_summary(
    check: dict[str, Any] | None,
    check_err: str | None,
    doc: dict[str, Any] | None,
    doc_err: str | None,
    probe: dict[str, Any] | None,
    probe_err: str | None,
) -> str:
    parts: list[str] = []

    if check_err:
        parts.append(f"check-config --strict: ({check_err})")
    elif check is None:
        parts.append("check-config --strict: (no artifact)")
    elif check.get("strict_enabled") is not True:
        parts.append("check-config --strict: skipped in artifact (strict_enabled=false)")
    elif check.get("strict_passed") is True:
        parts.append("check-config --strict: pass")
    else:
        parts.append("check-config --strict: fail")

    if doc_err:
        parts.append(f"doctor: ({doc_err})")
    elif doc is None:
        parts.append("doctor: (no artifact)")
    elif doc.get("status") == "pass":
        parts.append("doctor: pass")
    elif doc.get("error"):
        parts.append(f"doctor: fail ({doc.get('error')})")
    else:
        parts.append(f"doctor: {doc.get('status', 'unknown')}")

    if probe_err:
        parts.append(f"doctor --probe-upstream: ({probe_err})")
    elif probe is None:
        parts.append("doctor --probe-upstream: (no artifact)")
    elif probe.get("status") == "pass" and _count_unreachable_probes(probe) == 0:
        parts.append("doctor --probe-upstream: pass")
    elif probe.get("error"):
        parts.append(f"doctor --probe-upstream: fail ({probe.get('error')})")
    else:
        unreachable = _count_unreachable_probes(probe)
        st = probe.get("status")
        parts.append(f"doctor --probe-upstream: {st} ({unreachable} unreachable probe(s))")

    return "; ".join(parts)


def _route_smoke_line(route_doc: dict[str, Any] | None, route_err: str | None) -> str:
    if route_err:
        return f"§4 route-explain: ({route_err})"
    if route_doc is None:
        return "§4 route-explain: (no artifact)"
    matched = route_doc.get("matched")
    path = route_doc.get("path", "?")
    method = route_doc.get("method", "?")
    if matched is True:
        rid = route_doc.get("rule_id", "?")
        return f"§4 route-explain: matched yes — path `{path}`, method `{method}`, rule `{rid}`"
    if matched is False:
        return f"§4 route-explain: matched no — path `{path}`, method `{method}`"
    return f"§4 route-explain: unknown envelope — path `{path}`, method `{method}`"


def _config_basename(
    check: dict[str, Any] | None,
    snap: dict[str, Any] | None,
) -> str:
    if check and isinstance(check.get("config_path"), str):
        return Path(check["config_path"]).name
    if snap and isinstance(snap.get("config_basename"), str):
        return snap["config_basename"]
    return ""


def main() -> int:
    p = argparse.ArgumentParser(
        description="Emit Markdown section-9 summary table from release-acceptance CLI JSON inputs (§7).",
    )
    p.add_argument(
        "--artifacts-dir",
        type=Path,
        default=Path(os.environ.get("SERVICE_ROUTER_ACCEPTANCE_OUT", "artifacts/release-acceptance")),
        help="Directory with the five §7 JSON outputs (default: SERVICE_ROUTER_ACCEPTANCE_OUT or artifacts/release-acceptance); release-acceptance runners also write section-9-summary.generated.md here.",
    )
    p.add_argument(
        "--profile",
        default=os.environ.get("SERVICE_ROUTER_ACCEPTANCE_PROFILE", "").strip(),
        help="Profile label e.g. Mock / Nacos (default: SERVICE_ROUTER_ACCEPTANCE_PROFILE)",
    )
    p.add_argument(
        "--router-version",
        default=os.environ.get("SERVICE_ROUTER_VERSION", "").strip(),
        help="Router version string (default: SERVICE_ROUTER_VERSION)",
    )
    p.add_argument(
        "--global-gates",
        default=os.environ.get("SERVICE_ROUTER_ACCEPTANCE_GLOBAL_GATES", "").strip(),
        help='e.g. pass / fail / skipped (RUN_GLOBAL=0) (default: SERVICE_ROUTER_ACCEPTANCE_GLOBAL_GATES)',
    )
    p.add_argument(
        "--artifacts-url",
        default=os.environ.get("SERVICE_ROUTER_ACCEPTANCE_ARTIFACTS_URL", "").strip(),
        help="Optional CI artifact URL (default: SERVICE_ROUTER_ACCEPTANCE_ARTIFACTS_URL)",
    )
    p.add_argument(
        "--deviations",
        default=os.environ.get("SERVICE_ROUTER_ACCEPTANCE_DEVIATIONS", "").strip(),
        help="e.g. ALLOW_PROBE_FAIL=1 (default: SERVICE_ROUTER_ACCEPTANCE_DEVIATIONS)",
    )
    p.add_argument(
        "--sign-off",
        default=os.environ.get("SERVICE_ROUTER_ACCEPTANCE_SIGN_OFF", "").strip(),
        help="Sign-off line (default: SERVICE_ROUTER_ACCEPTANCE_SIGN_OFF)",
    )
    p.add_argument(
        "--date",
        default="",
        help="ISO-8601 timestamp (default: now UTC)",
    )
    args = p.parse_args()
    base: Path = args.artifacts_dir

    check, check_err = _load_json(base / "check-config.json")
    doc, doc_err = _load_json(base / "doctor.json")
    probe, probe_err = _load_json(base / "doctor-probe.json")
    route, route_err = _load_json(base / "route-explain-smoke.json")
    snap, snap_err = _load_json(base / "config-snapshot.json")

    check_d = check if isinstance(check, dict) else None
    doc_d = doc if isinstance(doc, dict) else None
    probe_d = probe if isinstance(probe, dict) else None
    route_d = route if isinstance(route, dict) else None
    snap_d = snap if isinstance(snap, dict) else None

    when = args.date.strip()
    if not when:
        when = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    git_rev = _git_head()
    git_cell = f"`{git_rev}`" if git_rev else "`(unknown; set GIT_COMMIT or run in a git repo)`"

    router_v = args.router_version.strip()
    if not router_v:
        router_v = "_(set --router-version or SERVICE_ROUTER_VERSION)_"

    profile = args.profile.strip()
    if not profile:
        profile = "_(Mock / Nacos / Eureka / Kubernetes — set --profile)_"

    cfg_name = _config_basename(check_d, snap_d)
    if not cfg_name:
        cfg_name = "_(from check-config.json / config-snapshot.json)_"

    global_gates = args.global_gates.strip()
    if not global_gates:
        global_gates = (
            "_(not recorded in §7 JSON; set --global-gates or SERVICE_ROUTER_ACCEPTANCE_GLOBAL_GATES)_"
        )

    cli_gates = _cli_gates_summary(check_d, check_err, doc_d, doc_err, probe_d, probe_err)
    route_line = _route_smoke_line(route_d, route_err)

    snap_present = snap_err is None and snap_d is not None
    snap_cell = "yes (redacted export)" if snap_present else "no / incomplete"

    if args.artifacts_url.strip():
        artifacts_display = args.artifacts_url.strip()
    else:
        artifacts_display = f"`{base.resolve()}`"

    deviations = args.deviations.strip()
    sign_off = args.sign_off.strip()

    lines = [
        "# §9 regression summary (generated)",
        "",
        "| Field | Value |",
        "|:------|:------|",
        f"| **Date / TZ** | {when} |",
        f"| **Git** | {git_cell} |",
        f"| **Router binary** | {router_v} |",
        f"| **Profile** | {profile} |",
        f"| **Config** | {cfg_name} |",
        f"| **Global gates** | §1 `cargo check` / `cargo test` — {global_gates} |",
        f"| **CLI gates** | §3 {cli_gates} |",
        f"| **Route smoke** | {route_line} |",
        f"| **Config snapshot** | `config-snapshot.json` — {snap_cell} |",
        f"| **Artifacts dir** | {artifacts_display} |",
        f"| **Deviations** | {deviations or '—'} |",
        f"| **Sign-off** | {sign_off or '—'} |",
        "",
        "## Expected artifacts (JSON + Markdown)",
        "",
    ]

    checklist_issues = False
    for name in ARTIFACT_NAMES:
        _data, err = _load_json(base / name)
        if err is None:
            mark = "x"
        else:
            mark = " "
            checklist_issues = True
        lines.append(f"- [{mark}] `{name}`")

    if checklist_issues:
        lines.append("")
        lines.append("_(Some artifacts are missing or invalid JSON; fix and re-run.)_")

    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
