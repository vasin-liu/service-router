# `config-snapshot` workflow (FR-5.3)

The CLI emits **redacted** JSON suitable for pasting into issues or attaching to tickets. It is **not** a hosted share link; hosting stays in your issue tracker, wiki, or object store.

## Command

```bash
cargo run -- config-snapshot --config /path/to/config.yaml -o snapshot.json
```

Stdout when **`-o -`**:

```bash
cargo run -- config-snapshot --config config/mock-config.yaml -o -
```

## What to attach

- Prefer **`config-snapshot.json`** plus the smallest **`route-explain --json`** repro for routing bugs.
- Do **not** paste raw YAML with secrets; the snapshot strips many sensitive fields but you should still treat output as **internal**.

## Automation

`docs/release-acceptance.sh` (and **`.ps1`**) write **`config-snapshot.json`** next to other acceptance artifacts (see **`release-acceptance-matrix.md`** §7) and emit **`section-9-summary.generated.md`**. The same directory (five JSON + that Markdown file) is bundled as the **`release-acceptance-json`** artifact in **`.github/workflows/release-acceptance.yml`** and the GitLab **`release-acceptance-manual`** job (see **`ci-template.md`**).

## Schema

Top-level shape is defined by `service_router::config_snapshot_export::ConfigSnapshotExport` in **`src/lib.rs`** (`diagnostic_version` **1.0**, UUID `snapshot_id`, route **`response_header_keys`** when configured).

## §9 acceptance bundle (optional)

When you run **`docs/release-acceptance.sh`** / **`.ps1`**, the directory also holds **`check-config.json`**, **`doctor.json`**, **`doctor-probe.json`**, **`route-explain-smoke.json`**, and **`config-snapshot.json`**, plus auto-generated **`section-9-summary.generated.md`**. To regenerate that Markdown with different flags (or if the file is missing), use **`python scripts/summarize-section9-release-acceptance.py`** (see **`docs/regression-archive/`** and **`--help`**).
