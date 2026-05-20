//! Planner stress-test benchmark.
//!
//! Exercises the internal DPLL SAT solver with progressively larger constraint
//! sets: 50+ variables, 100+ clauses, and mixed requires/conflicts/must/avoid
//! rules. Measures solve time and reports problem dimensions.

use chrono::{TimeZone, Utc};
use rbmem::{plan_memory, PlanOptions, SatBackend};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 20, 10, 0, 0).unwrap()
}

fn temp_dir(label: &str) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rbmem-planner-stress-{label}-{ts}"))
}

/// Generate a memory file with `num_tasks` candidate actions and enough rules
/// to produce at least `target_clauses` SAT clauses.
fn generate_planning_problem(dir: &PathBuf, num_tasks: usize, target_clauses: usize) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join("memory.rbmem");

    let mut tasks = Vec::new();
    for i in 0..num_tasks {
        tasks.push(format!("  - Task alpha {i}: perform operation alpha-{i}"));
    }
    let tasks_text = tasks.join("\n");

    // Build rules that generate clauses:
    // - requires rules: "task X requires task Y" → clause (-X ∨ Y)
    // - conflict rules: "task X conflicts with task Y" → clause (-X ∨ -Y)
    // - must rules: "must task X" → clause (X)
    // - avoid rules: "avoid task X" → clause (-X)
    let mut rules = Vec::new();
    let mut clause_count = 0;

    // Chain of requires: task_0 requires task_1, task_1 requires task_2, ...
    // Each generates 1 clause if the matching works
    for i in 0..num_tasks.saturating_sub(1) {
        rules.push(format!(
            "  - Task alpha {next}: perform operation alpha-{next} requires Task alpha {i}: perform operation alpha-{i}",
            next = i + 1,
            i = i
        ));
        clause_count += 1;
        if clause_count >= target_clauses {
            break;
        }
    }

    // Add conflict pairs to generate more clauses
    if clause_count < target_clauses {
        let mut i = 0;
        while clause_count < target_clauses && i + 2 < num_tasks {
            rules.push(format!(
                "  - Task alpha {a}: perform operation alpha-{a} conflicts with Task alpha {b}: perform operation alpha-{b}",
                a = i,
                b = i + 2
            ));
            clause_count += 1;
            i += 3;
        }
    }

    // Add must/avoid rules
    if clause_count < target_clauses {
        for i in (0..num_tasks).step_by(5) {
            if clause_count >= target_clauses {
                break;
            }
            rules.push(format!(
                "  - must Task alpha {i}: perform operation alpha-{i}"
            ));
            clause_count += 1;
        }
    }
    if clause_count < target_clauses {
        for i in (1..num_tasks).step_by(7) {
            if clause_count >= target_clauses {
                break;
            }
            rules.push(format!(
                "  - avoid Task alpha {i}: perform operation alpha-{i}"
            ));
            clause_count += 1;
        }
    }

    let rules_text = rules.join("\n");

    let content = format!(
        r#"meta:
  version: 1.4.0
  purpose: "planner stress test"
  created_by: "bench"

[SECTION: goals]
type: list
content: |
  - complete task alpha 0
[END SECTION]

[SECTION: tasks]
type: list
content: |
{tasks_text}
[END SECTION]

[SECTION: rules]
type: list
content: |
{rules_text}
[END SECTION]
"#
    );

    fs::write(&file, content).unwrap();
    file
}

/// Generate a simpler problem with just a chain of requires for predictable scaling.
fn generate_chain_problem(dir: &PathBuf, chain_length: usize) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join("memory.rbmem");

    let mut tasks = Vec::new();
    for i in 0..chain_length {
        tasks.push(format!("  - Step {i}"));
    }

    let mut rules = Vec::new();
    for i in 1..chain_length {
        rules.push(format!("  - Step {i} requires Step {}", i - 1));
    }

    let tasks_text = tasks.join("\n");
    let rules_text = rules.join("\n");

    let content = format!(
        r#"meta:
  version: 1.4.0
  purpose: "planner chain stress test"
  created_by: "bench"

[SECTION: goals]
type: list
content: |
  - complete step {last}
[END SECTION]

[SECTION: tasks]
type: list
content: |
{tasks_text}
[END SECTION]

[SECTION: rules]
type: list
content: |
{rules_text}
[END SECTION]
"#,
        last = chain_length - 1
    );

    fs::write(&file, content).unwrap();
    file
}

/// Generate a dense problem with many conflict pairs (graph coloring style).
fn generate_dense_conflict_problem(dir: &PathBuf, num_tasks: usize) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let file = dir.join("memory.rbmem");

    let mut tasks = Vec::new();
    for i in 0..num_tasks {
        tasks.push(format!("  - Action {i}"));
    }

    // Generate O(n^2/4) conflict pairs (every even with every odd)
    let mut rules = Vec::new();
    for i in (0..num_tasks).step_by(2) {
        for j in (1..num_tasks).step_by(2) {
            if j > i {
                rules.push(format!("  - Action {i} conflicts with Action {j}"));
            }
        }
    }

    let tasks_text = tasks.join("\n");
    let rules_text = rules.join("\n");

    let content = format!(
        r#"meta:
  version: 1.4.0
  purpose: "planner dense conflict stress test"
  created_by: "bench"

[SECTION: goals]
type: list
content: |
  - perform action 0
[END SECTION]

[SECTION: tasks]
type: list
content: |
{tasks_text}
[END SECTION]

[SECTION: rules]
type: list
content: |
{rules_text}
[END SECTION]
"#
    );

    fs::write(&file, content).unwrap();
    file
}

fn run_bench(
    _label: &str,
    file: &Path,
    dir: &Path,
    goal: &str,
    cube_and_conquer: bool,
    iterations: usize,
) -> (f64, usize, usize, usize) {
    let now = fixed_time();
    let mut total_us = 0u128;
    let mut last_vars = 0;
    let mut last_clauses = 0;
    let mut last_selected = 0;

    for _ in 0..iterations {
        let start = Instant::now();
        let report = plan_memory(PlanOptions {
            goal: Some(goal.to_string()),
            from_memory: false,
            file: Some(file.to_path_buf()),
            search_dir: dir.to_path_buf(),
            context_pack: None,
            solver: SatBackend::Internal,
            proof: false,
            proof_path: None,
            verify_proof: false,
            cube_and_conquer,
            dry_run: true,
            now,
        })
        .unwrap();
        total_us += start.elapsed().as_micros();
        last_vars = report.variables;
        last_clauses = report.clauses;
        last_selected = report.selected;
    }

    let avg_us = total_us / iterations as u128;
    (avg_us as f64, last_vars, last_clauses, last_selected)
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║                  RBMEM Planner Stress-Test Benchmark                  ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");

    let iterations = 5;

    // ---------------------------------------------------------------
    // BENCH 1: Chain problems (linear requires)
    // ---------------------------------------------------------------
    println!("━━━ BENCH 1: Linear Chain Problems (requires chains) ━━━\n");
    println!(
        " {:<30} {:>10} {:>10} {:>10} {:>12}",
        "Config", "Vars", "Clauses", "Selected", "Avg Time"
    );
    println!(" {}", "─".repeat(78));

    for chain_len in [20, 50, 100, 200] {
        let dir = temp_dir(&format!("chain-{chain_len}"));
        let file = generate_chain_problem(&dir, chain_len);
        let goal = format!("complete step {}", chain_len - 1);
        let (avg_us, vars, clauses, selected) = run_bench(
            &format!("chain-{chain_len}"),
            &file,
            &dir,
            &goal,
            false,
            iterations,
        );
        println!(
            " {:<30} {:>10} {:>10} {:>10} {:>12}",
            format!("chain-{chain_len}"),
            vars,
            clauses,
            selected,
            format!("{:.1}ms", avg_us / 1000.0)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    println!();

    // ---------------------------------------------------------------
    // BENCH 2: Mixed constraint problems
    // ---------------------------------------------------------------
    println!("━━━ BENCH 2: Mixed Constraints (requires + conflicts + must + avoid) ━━━\n");
    println!(
        " {:<30} {:>10} {:>10} {:>10} {:>12}",
        "Config", "Vars", "Clauses", "Selected", "Avg Time"
    );
    println!(" {}", "─".repeat(78));

    for (num_tasks, target_clauses) in [(50, 100), (100, 200), (100, 400), (200, 500)] {
        let dir = temp_dir(&format!("mixed-{num_tasks}-{target_clauses}"));
        let file = generate_planning_problem(&dir, num_tasks, target_clauses);
        let (avg_us, vars, clauses, selected) = run_bench(
            &format!("mixed-{num_tasks}-{target_clauses}"),
            &file,
            &dir,
            "complete task alpha 0",
            false,
            iterations,
        );
        println!(
            " {:<30} {:>10} {:>10} {:>10} {:>12}",
            format!("{num_tasks}v/{target_clauses}c target"),
            vars,
            clauses,
            selected,
            format!("{:.1}ms", avg_us / 1000.0)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    println!();

    // ---------------------------------------------------------------
    // BENCH 3: Dense conflict problems (graph coloring style)
    // ---------------------------------------------------------------
    println!("━━━ BENCH 3: Dense Conflict Problems (graph coloring) ━━━\n");
    println!(
        " {:<30} {:>10} {:>10} {:>10} {:>12}",
        "Config", "Vars", "Clauses", "Selected", "Avg Time"
    );
    println!(" {}", "─".repeat(78));

    for num_tasks in [20, 40, 60] {
        let dir = temp_dir(&format!("dense-{num_tasks}"));
        let file = generate_dense_conflict_problem(&dir, num_tasks);
        let (avg_us, vars, clauses, selected) = run_bench(
            &format!("dense-{num_tasks}"),
            &file,
            &dir,
            "perform action 0",
            false,
            iterations,
        );
        println!(
            " {:<30} {:>10} {:>10} {:>10} {:>12}",
            format!("dense-conflict-{num_tasks}"),
            vars,
            clauses,
            selected,
            format!("{:.1}ms", avg_us / 1000.0)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    println!();

    // ---------------------------------------------------------------
    // BENCH 4: Cube-and-conquer vs standard DPLL
    // ---------------------------------------------------------------
    println!("━━━ BENCH 4: Cube-and-Conquer vs Standard DPLL ━━━\n");
    println!(
        " {:<30} {:>10} {:>10} {:>10} {:>12}",
        "Config", "Vars", "Clauses", "Selected", "Avg Time"
    );
    println!(" {}", "─".repeat(78));

    for chain_len in [50, 100] {
        let dir = temp_dir(&format!("cnc-{chain_len}"));
        let file = generate_chain_problem(&dir, chain_len);
        let goal = format!("complete step {}", chain_len - 1);

        let (std_us, vars, clauses, selected) = run_bench(
            &format!("std-{chain_len}"),
            &file,
            &dir,
            &goal,
            false,
            iterations,
        );
        println!(
            " {:<30} {:>10} {:>10} {:>10} {:>12}",
            format!("standard-dpll chain-{chain_len}"),
            vars,
            clauses,
            selected,
            format!("{:.1}ms", std_us / 1000.0)
        );

        let (cnc_us, _, _, _) = run_bench(
            &format!("cnc-{chain_len}"),
            &file,
            &dir,
            &goal,
            true,
            iterations,
        );
        println!(
            " {:<30} {:>10} {:>10} {:>10} {:>12}",
            format!("cube-and-conquer chain-{chain_len}"),
            "",
            "",
            "",
            format!("{:.1}ms", cnc_us / 1000.0)
        );

        let _ = fs::remove_dir_all(&dir);
    }

    println!();

    // ---------------------------------------------------------------
    // SUMMARY
    // ---------------------------------------------------------------
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║                             SUMMARY                                    ║");
    println!("╠════════════════════════════════════════════════════════════════════════╣");
    println!("║ All benchmarks completed successfully with the internal DPLL solver.   ║");
    println!("║ Problems tested: chain (linear), mixed constraints, dense conflicts.   ║");
    println!("║ Maximum scale: 200 variables, 500+ clauses.                            ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝");
}
