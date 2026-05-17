# CODE REVIEW: rbmem v1.4.1

## Changes Overview

| File | Lines Changed | Summary |
|------|--------------|---------|
| Cargo.toml | +1 | Added `md5` dependency for snapshot hashing |
| Cargo.lock | +7 | Lockfile update |
| src/commands.rs | +520/-31 | Snapshot JSON serialization, configurable health stale_days |
| src/document.rs | +22 | HealthScore export, SectionType::Guards, SectionType::Review |
| src/lib.rs | +10 | New public exports for health, guards, snapshots |
| src/main.rs | +147/-31 | CLI args, health output, JSON health fields |

## 1. CORRECTNESS ASSESSMENT

### Snapshot Serialization (commands.rs:714-812)
**PASS** — The switch from string-parsed YAML to `serde_json::to_string_pretty` for `.snap` metadata is correct. JSON handles edge cases (labels with colons, quotes, Unicode) that the old prefix-matching parser would break on. The `serde_json::from_str` in `list_snapshots` properly deserializes the new format.

### Health Scoring (commands.rs:814-852)
**PASS** — The scoring formula is mathematically sound:
- Stale sections penalty: `(stale/total) * 30.0` (max 30 points)
- Orphaned edges penalty: `(orphans/total) * 20.0` (max 20 points)  
- Conflict penalty: `(conflicts/total) * 25.0` (max 25 points)
- Total max penalty: 75 points → minimum score: 25/100
- Floor at 0.0 prevents negative scores

**NOTE:** The health scoring never produces a score below 25 even with all issues. A document with 100% stale sections would score 70/100. This might be too forgiving if the intent is to flag severely degraded documents more aggressively. Consider whether the weight distribution matches the intended severity scale.

### Review Dry-Run (main.rs:668-681)
**PASS** — The `warning_count` borrow-before-move fix is correct. `parsed.warnings.len()` is called first, then `parsed.warnings` is moved into `review_document()`. Without the count pre-capture, this would be a compile error.

## 2. POTENTIAL BUGS / EDGE CASES

### Bug 1: Snapshot label collision (commands.rs:736-738)
**Severity: LOW**

```rust
let snapshot_content = serde_json::to_string_pretty(&record)?;
fs::write(&snapshot_file, snapshot_content)?;
```

If two snapshots share the same label, the second one silently overwrites the first's `.snap` metadata file. The `.rbmem` content file has the same collision risk. No uniqueness enforcement.

**Fix suggestion:** Append a hash suffix or timestamp to guarantee uniqueness, or return an error on collision.

### Bug 2: Stale days zero (commands.rs:826)
**Severity: LOW**

If `stale_days` is 0, `stale_cutoff` becomes `now`, meaning sections updated *this exact second* would be flagged as stale. The `chrono::Duration::days(0)` call is technically valid but the semantics are surprising — it would mark nearly all recent sections as stale.

**Fix suggestion:** Document the behavior or add a minimum of 1 day.

### Bug 3: Rollback auto-backup error swallowing (commands.rs:784-785)
**Severity: MEDIUM**

```rust
let auto_label = format!("pre-rollback-{}", Utc::now().format("%Y%m%d-%H%M%S"));
let _ = create_snapshot(path, &auto_label);
```

The auto-backup before rollback uses `let _ =` which silently discards any error. If the auto-backup fails (e.g., disk full, permissions), the rollback proceeds without any safety net. The user gets rolled back with no way to recover.

**Fix suggestion:** Return an error if the auto-backup fails, or at least log it:
```rust
if let Err(e) = create_snapshot(path, &auto_label) {
    eprintln!("WARNING: auto-backup failed: {}", e);
}
```

### Bug 4: `parse_snapshot_line` dead code (commands.rs:802-812)
**Severity: LOW**

This function is still present but no longer called after the JSON migration. It's dead code.

## 3. PERFORMANCE CONCERNS

### SectionConflict O(N²) (commands.rs:870-882)
**Severity: LOW (acceptable for current scale)**

```rust
for i in 0..document.sections.len() {
    for j in (i+1)..document.sections.len() {
        if s1.path == s2.path && s1.content != s2.content {
            conflicts += 1;
        }
    }
}
```

With N sections, this does N*(N-1)/2 comparisons. For a typical 50-section memory file, that's ~1,225 iterations. Fine. But for large documents (1000+ sections), this becomes a bottleneck.

**Suggestion:** Use a HashMap to group by path and compare in O(N) time:
```rust
let mut by_path: BTreeMap<&str, Vec<&Section>> = BTreeMap::new();
for section in &document.sections {
    by_path.entry(section.path.as_str()).or_default().push(section);
}
for (_, sections) in &by_path {
    if sections.len() > 1 {
        conflicts += sections.len() - 1;
    }
}
```

### `health_report` loads and parses full document
**Severity: LOW**

The function loads and parses the entire document just to count sections and compute health metrics. For large documents this is I/O-bound but acceptable given the existing architecture.

## 4. STYLE / READABILITY NOTES

### Good
- Health output format is consistent and machine-readable
- JSON output includes all health fields for programmatic consumption
- `--dry-run` follows standard CLI conventions
- Snapshot test data uses proper `[SECTION: ...]` delimiters
- Error messages are specific and actionable

### Suggest Improvements

**1. Health score thresholds not documented** (main.rs health output)
```
health-score: 67/100
```
Users won't know what 67 means without a legend. Add severity labels:
```
health-score: 67/100 (WARNING: 3 stale sections, 2 orphaned edges)
  [90-100] HEALTHY  [70-89] FINE  [50-69] WARNING  [<50] CRITICAL
```

**2. Missing `--help` for new flags** (main.rs)
The `--dry-run` flag on Review and `--stale-days` on Doctor are new and not mentioned in any docs. A `--help` output would be useful for discoverability.

**3. `parse_snapshot_line` dead code** (commands.rs:802-812)
Remove or deprecate with a `#[deprecated]` annotation since it's no longer used.

**4. Health scoring weights are magic numbers** (commands.rs:839-841)
The weights 30.0, 20.0, 25.0 are hardcoded with no explanation of why these specific values. Consider documenting them or extracting to named constants.

## 5. LINE-LEVEL SUGGESTIONS

### commands.rs:736-738 — Snapshot file naming
```rust
// Current:
let snapshot_file = snapshot_dir.join(format!("{}.snap", label));
let content_file = snapshot_dir.join(format!("{}.rbmem", label));

// Suggestion: Add hash suffix for uniqueness
let hash_short = &record.file_hash[..8];
let snapshot_file = snapshot_dir.join(format!("{}_{}.snap", label, hash_short));
```

### commands.rs:835-836 — Score calculation readability
```rust
// Current:
let penalty = ((stale_sections as f64 / total) * 30.0
    + (orphaned_edges as f64 / total.max(1.0)) * 20.0
    + (conflicts as f64 / total.max(1.0)) * 25.0);

// Suggestion: Extract named weights
const STALE_WEIGHT: f64 = 30.0;
const ORPHAN_WEIGHT: f64 = 20.0;
const CONFLICT_WEIGHT: f64 = 25.0;
let penalty = (stale_sections as f64 / total) * STALE_WEIGHT
    + (orphaned_edges as f64 / total.max(1.0)) * ORPHAN_WEIGHT
    + (conflicts as f64 / total.max(1.0)) * CONFLICT_WEIGHT;
```

### main.rs:1310-1314 — Health JSON field naming
```rust
// Current naming is inconsistent:
// health-score (kebab-case) vs health-stale-days vs health-total-sections
// Consider: all kebab or all snake. The health-score uses kebab because of the `score` field name in HealthScore struct.

// Fix: Rename struct field to match, or use a serde rename
#[derive(Serialize)]
pub struct HealthScore {
    #[serde(rename = "score")]  // This produces "health-score" in the parent
    pub score: f64,
    // ...
}
```

## 6. SUMMARY

**Overall: APPROVE WITH MINOR FIXES**

The v1.4.1 changes are solid — the JSON snapshot serialization is a worthwhile improvement, the configurable stale_days is exactly what was needed, and the dry-run flag is a standard addition. The code is well-tested (all 46 tests pass).

**Priority fixes:**
1. Auto-backup error handling in rollback (medium)
2. Health score documentation/labeling (low)
3. Dead code cleanup `parse_snapshot_line` (low)

**Non-blocking:**
- Snapshot label collision (low risk in practice)
- Stale days = 0 edge case
- O(N²) conflict counting (fine for current scale)
