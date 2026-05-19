//! SAT-backed planning for RBMEM documents.
//!
//! The planner turns goals plus durable RBMEM context into a small CNF problem:
//! candidate actions become Boolean variables, memory rules become clauses, and
//! the satisfying assignment becomes a plan stored back into the same memory
//! graph. External Kissat/CaDiCaL binaries are used when available; the internal
//! DPLL solver keeps the feature usable without extra installation steps.

use crate::document::{
    GraphInfo, GraphRelation, RbmemDocument, RbmemError, Section, SectionType, SourceInfo,
    TimestampPolicy,
};
use crate::parser::parse_document;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum SatBackend {
    Auto,
    Internal,
    Kissat,
    Cadical,
}

impl std::fmt::Display for SatBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SatBackend::Auto => f.write_str("auto"),
            SatBackend::Internal => f.write_str("internal"),
            SatBackend::Kissat => f.write_str("kissat"),
            SatBackend::Cadical => f.write_str("cadical"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SatStatus {
    Sat,
    Unsat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanOptions {
    pub goal: Option<String>,
    pub from_memory: bool,
    pub file: Option<PathBuf>,
    pub search_dir: PathBuf,
    pub context_pack: Option<String>,
    pub solver: SatBackend,
    pub proof: bool,
    pub proof_path: Option<PathBuf>,
    pub verify_proof: bool,
    pub cube_and_conquer: bool,
    pub dry_run: bool,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub order: usize,
    pub action: String,
    pub source: Option<String>,
    pub score: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProofReport {
    pub requested: bool,
    pub path: Option<String>,
    pub generated: bool,
    pub verified: bool,
    pub verifier: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanReport {
    pub schema: String,
    pub goal: String,
    pub memory_file: String,
    pub discovered_files: Vec<String>,
    pub plan_path: String,
    pub status: SatStatus,
    pub solver: String,
    pub cube_and_conquer: bool,
    pub variables: usize,
    pub clauses: usize,
    pub selected: usize,
    pub steps: Vec<PlanStep>,
    pub context_sections: Vec<String>,
    pub proof: ProofReport,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
struct CandidateAction {
    title: String,
    text: String,
    source_path: Option<String>,
    score: i64,
}

#[derive(Debug, Clone)]
struct RuleClause {
    clause: Vec<i32>,
}

#[derive(Debug, Clone)]
struct PlanningProblem {
    goal: String,
    candidates: Vec<CandidateAction>,
    context_sections: Vec<String>,
    clauses: Vec<RuleClause>,
    requires: Vec<(usize, usize)>,
    conflicts: Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct SolverResult {
    status: SatStatus,
    assignment: Vec<bool>,
    solver: String,
}

pub fn plan_memory(options: PlanOptions) -> Result<PlanReport, RbmemError> {
    let files = discover_memory_files(options.file.as_deref(), &options.search_dir)?;
    let primary_path = select_primary_file(options.file.as_deref(), &options.search_dir, &files);
    let mut primary = load_or_create_document(&primary_path, options.now)?;

    let mut context = Vec::new();
    for file in &files {
        if file == &primary_path {
            context.push(primary.clone());
        } else if let Ok(document) = load_document(file) {
            context.push(document);
        }
    }
    if context.is_empty() {
        context.push(primary.clone());
    }
    if let Some(pack_name) = &options.context_pack {
        let includes = load_context_pack(&primary_path, &options.search_dir, pack_name)?;
        context = filter_context_for_pack(context, &includes);
    }

    let problem = build_problem(&context, options.goal.as_deref(), options.from_memory)?;
    let mut solve = solve_problem(&problem, options.solver, options.cube_and_conquer)?;
    if solve.status == SatStatus::Sat {
        minimize_assignment(&problem, &mut solve.assignment);
    }
    let selected_indices = selected_indices(&solve.assignment, &problem.candidates);
    let steps = order_steps(&problem, &selected_indices);
    let plan_slug = plan_slug(&problem.goal, options.now);
    let plan_path = format!("plans.{plan_slug}");
    let dimacs = dimacs_string(problem.candidates.len(), &problem.clauses);

    let proof = handle_proof(
        &problem,
        &solve,
        &dimacs,
        &primary_path,
        &plan_slug,
        &options,
    )?;

    if !options.dry_run {
        persist_plan(
            &mut primary,
            &primary_path,
            &problem,
            &solve,
            &steps,
            &proof,
            &plan_path,
            &dimacs,
            options.now,
        )?;
    }

    Ok(PlanReport {
        schema: "rbmem.plan.v1".to_string(),
        goal: problem.goal,
        memory_file: primary_path.display().to_string(),
        discovered_files: files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        plan_path,
        status: solve.status,
        solver: solve.solver,
        cube_and_conquer: options.cube_and_conquer,
        variables: problem.candidates.len(),
        clauses: problem.clauses.len(),
        selected: steps.len(),
        steps,
        context_sections: problem.context_sections,
        proof,
        dry_run: options.dry_run,
    })
}

pub fn discover_memory_files(
    explicit: Option<&Path>,
    search_dir: &Path,
) -> Result<Vec<PathBuf>, RbmemError> {
    if let Some(path) = explicit {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    if search_dir.exists() {
        for entry in fs::read_dir(search_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rbmem") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn select_primary_file(explicit: Option<&Path>, search_dir: &Path, files: &[PathBuf]) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }

    for preferred in ["memory.rbmem", "MEMORY.rbmem", "my-agent-memory.rbmem"] {
        let candidate = search_dir.join(preferred);
        if files.iter().any(|path| same_path(path, &candidate)) {
            return candidate;
        }
    }

    files
        .first()
        .cloned()
        .unwrap_or_else(|| search_dir.join("memory.rbmem"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.file_name() == right.file_name() && left.parent() == right.parent()
}

fn load_or_create_document(path: &Path, now: DateTime<Utc>) -> Result<RbmemDocument, RbmemError> {
    if path.exists() {
        load_document(path)
    } else {
        let mut document = RbmemDocument::new(now, "planner");
        document.meta.purpose = "personal-agent-memory".to_string();
        Ok(document)
    }
}

fn load_document(path: &Path) -> Result<RbmemDocument, RbmemError> {
    Ok(parse_document(&fs::read_to_string(path)?, TimestampPolicy::Preserve)?.document)
}

fn load_context_pack(
    primary_path: &Path,
    search_dir: &Path,
    name: &str,
) -> Result<BTreeSet<String>, RbmemError> {
    let candidates = [
        primary_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".rbmempacks"),
        search_dir.join(".rbmempacks"),
    ];

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        if let Some(includes) = parse_context_pack_includes(&text, name) {
            return Ok(includes);
        }
    }

    Err(RbmemError::NotFound(format!(
        "context pack '{name}' not found"
    )))
}

fn parse_context_pack_includes(text: &str, name: &str) -> Option<BTreeSet<String>> {
    let header = format!("[pack: {name}]");
    let mut in_pack = false;
    let mut in_include = false;
    let mut includes = BTreeSet::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[pack:") {
            if in_pack {
                break;
            }
            in_pack = trimmed == header;
            in_include = false;
            continue;
        }
        if !in_pack {
            continue;
        }
        if trimmed == "include:" {
            in_include = true;
            continue;
        }
        if trimmed.ends_with(':') && trimmed != "include:" {
            in_include = false;
            continue;
        }
        if in_include {
            if let Some(path) = trimmed.strip_prefix("- ") {
                includes.insert(path.trim().to_string());
            }
        }
    }

    in_pack.then_some(includes)
}

fn filter_context_for_pack(
    mut documents: Vec<RbmemDocument>,
    includes: &BTreeSet<String>,
) -> Vec<RbmemDocument> {
    if includes.is_empty() {
        return documents;
    }

    for document in &mut documents {
        document.sections.retain(|section| {
            is_planning_context(section)
                || includes.iter().any(|include| {
                    section.path == *include || section.path.starts_with(&format!("{include}."))
                })
        });
    }
    documents
}

fn build_problem(
    documents: &[RbmemDocument],
    explicit_goal: Option<&str>,
    from_memory: bool,
) -> Result<PlanningProblem, RbmemError> {
    let goal = explicit_goal
        .map(str::trim)
        .filter(|goal| !goal.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            from_memory
                .then(|| derive_goal_from_memory(documents))
                .flatten()
        })
        .ok_or_else(|| {
            RbmemError::Parse(
                "missing goal; pass rbmem plan \"<goal>\" or --from-memory".to_string(),
            )
        })?;

    let mut candidates = extract_candidates(documents, &goal);
    if candidates.is_empty() {
        candidates.push(CandidateAction {
            title: format!("Work toward: {goal}"),
            text: goal.clone(),
            source_path: None,
            score: 10,
        });
    }

    rank_candidates(&mut candidates, &goal);
    let mut clauses = Vec::new();
    add_goal_clause(&mut clauses, &candidates, &goal);

    let mut requires = Vec::new();
    let mut conflicts = Vec::new();
    for section in documents.iter().flat_map(|document| &document.sections) {
        apply_section_rules(
            section,
            &candidates,
            &mut clauses,
            &mut requires,
            &mut conflicts,
        );
    }

    let context_sections = documents
        .iter()
        .flat_map(|document| &document.sections)
        .filter(|section| is_planning_context(section))
        .map(|section| section.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(PlanningProblem {
        goal,
        candidates,
        context_sections,
        clauses,
        requires,
        conflicts,
    })
}

fn derive_goal_from_memory(documents: &[RbmemDocument]) -> Option<String> {
    for section in documents.iter().flat_map(|document| &document.sections) {
        if !section.path.contains("goal") && !section.path.contains("task") {
            continue;
        }
        for line in section.content.lines() {
            if let Some(item) = bullet_text(line) {
                if !item.is_empty() {
                    return Some(item);
                }
            }
        }
        let content = section.content.trim();
        if !content.is_empty() {
            return Some(first_sentence(content));
        }
    }
    None
}

fn extract_candidates(documents: &[RbmemDocument], goal: &str) -> Vec<CandidateAction> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();

    for section in documents.iter().flat_map(|document| &document.sections) {
        if section.section_type == SectionType::Encrypted {
            continue;
        }

        let path_relevant = path_has_any(
            &section.path,
            &[
                "action", "actions", "task", "tasks", "step", "steps", "goal", "plan",
            ],
        );

        if path_relevant {
            for line in section.content.lines() {
                if let Some(item) = bullet_text(line) {
                    if item.len() < 4 {
                        continue;
                    }
                    let normalized = normalize(&item);
                    if seen.insert(normalized) {
                        candidates.push(CandidateAction {
                            title: item.clone(),
                            text: format!("{}\n{}", section.path, section.content),
                            source_path: Some(section.path.clone()),
                            score: 4,
                        });
                    }
                }
            }
        }

        if path_relevant && !section.content.trim().is_empty() {
            let title = first_sentence(&section.content);
            let normalized = normalize(&title);
            if seen.insert(normalized) {
                candidates.push(CandidateAction {
                    title,
                    text: section.content.clone(),
                    source_path: Some(section.path.clone()),
                    score: 3,
                });
            }
        }
    }

    if !candidates
        .iter()
        .any(|candidate| overlap_score(&candidate.title, goal) > 0)
    {
        candidates.push(CandidateAction {
            title: format!("Satisfy goal: {goal}"),
            text: goal.to_string(),
            source_path: None,
            score: 8,
        });
    }

    candidates
}

fn rank_candidates(candidates: &mut [CandidateAction], goal: &str) {
    for candidate in candidates {
        candidate.score += overlap_score(&candidate.title, goal) as i64 * 4;
        candidate.score += overlap_score(&candidate.text, goal) as i64;
    }
}

fn add_goal_clause(clauses: &mut Vec<RuleClause>, candidates: &[CandidateAction], goal: &str) {
    let threshold = phrase_threshold(goal);
    let mut vars = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (overlap_score(&candidate.title, goal) >= threshold).then_some((index + 1) as i32)
        })
        .collect::<Vec<_>>();

    if vars.is_empty() {
        vars = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (overlap_score(&candidate.text, goal) > 1).then_some((index + 1) as i32)
            })
            .collect();
    }

    if vars.is_empty() {
        vars = (1..=candidates.len()).map(|index| index as i32).collect();
    }

    if !vars.is_empty() {
        clauses.push(RuleClause { clause: vars });
    }
}

fn apply_section_rules(
    section: &Section,
    candidates: &[CandidateAction],
    clauses: &mut Vec<RuleClause>,
    requires: &mut Vec<(usize, usize)>,
    conflicts: &mut Vec<(usize, usize)>,
) {
    if !is_planning_context(section) {
        return;
    }

    for line in section
        .content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let normalized = normalize(line);
        if let Some((left, right)) = parse_requires(&normalized) {
            for a in matching_candidates(candidates, &left) {
                for b in matching_candidates(candidates, &right) {
                    if a != b {
                        clauses.push(RuleClause {
                            clause: vec![-((a + 1) as i32), (b + 1) as i32],
                        });
                        requires.push((a, b));
                    }
                }
            }
            continue;
        }

        if let Some((left, right)) = parse_conflict(&normalized) {
            for a in matching_candidates(candidates, &left) {
                for b in matching_candidates(candidates, &right) {
                    if a != b {
                        clauses.push(RuleClause {
                            clause: vec![-((a + 1) as i32), -((b + 1) as i32)],
                        });
                        conflicts.push((a, b));
                    }
                }
            }
            continue;
        }

        if starts_with_any(&normalized, &["must ", "always ", "include ", "required "]) {
            for index in matching_candidates(candidates, strip_rule_prefix(&normalized)) {
                clauses.push(RuleClause {
                    clause: vec![(index + 1) as i32],
                });
            }
        }

        if starts_with_any(
            &normalized,
            &[
                "avoid ",
                "never ",
                "do not ",
                "dont ",
                "exclude ",
                "prohibit ",
            ],
        ) {
            for index in matching_candidates(candidates, strip_rule_prefix(&normalized)) {
                clauses.push(RuleClause {
                    clause: vec![-((index + 1) as i32)],
                });
            }
        }
    }
}

fn is_planning_context(section: &Section) -> bool {
    path_has_any(
        &section.path,
        &[
            "rule",
            "rules",
            "constraint",
            "constraints",
            "preference",
            "preferences",
            "guard",
            "guards",
            "task",
            "tasks",
            "goal",
            "goals",
            "plan",
            "plans",
            "context",
        ],
    )
}

fn parse_requires(line: &str) -> Option<(String, String)> {
    if let Some((left, right)) = line.split_once(" requires ") {
        return Some((clean_rule_side(left), clean_rule_side(right)));
    }
    if let Some((left, right)) = line.split_once(" depends on ") {
        return Some((clean_rule_side(left), clean_rule_side(right)));
    }
    if let Some((left, right)) = line.split_once(" after ") {
        return Some((clean_rule_side(left), clean_rule_side(right)));
    }
    if line.starts_with("requires ") || line.starts_with("require ") {
        if let Some((left, right)) = line.split_once(" -> ") {
            return Some((clean_rule_side(left), clean_rule_side(right)));
        }
    }
    None
}

fn parse_conflict(line: &str) -> Option<(String, String)> {
    for marker in [
        " conflicts with ",
        " cannot combine with ",
        " cannot run with ",
        " not with ",
        " mutually exclusive with ",
    ] {
        if let Some((left, right)) = line.split_once(marker) {
            return Some((clean_rule_side(left), clean_rule_side(right)));
        }
    }
    None
}

fn clean_rule_side(value: &str) -> String {
    strip_rule_prefix(value)
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != ' ')
        .trim()
        .to_string()
}

fn strip_rule_prefix(value: &str) -> &str {
    for prefix in [
        "- ",
        "* ",
        "must ",
        "always ",
        "include ",
        "required ",
        "requires ",
        "require ",
        "avoid ",
        "never ",
        "do not ",
        "dont ",
        "exclude ",
        "prohibit ",
        "prefer ",
        "prioritize ",
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return rest;
        }
    }
    value
}

fn matching_candidates(candidates: &[CandidateAction], phrase: &str) -> Vec<usize> {
    let phrase = normalize(phrase);
    if phrase.len() < 3 {
        return Vec::new();
    }
    let threshold = phrase_threshold(&phrase);

    let mut scored = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let haystack = normalize(&candidate.title);
            let score = if haystack.contains(&phrase) {
                100
            } else {
                overlap_score(&haystack, &phrase)
            };
            (score >= threshold).then_some((index, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.1.cmp(&left.1));
    if !scored.is_empty() {
        return scored.into_iter().take(3).map(|(index, _)| index).collect();
    }

    let mut scored = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let haystack = normalize(&candidate.text);
            let score = if haystack.contains(&phrase) {
                50
            } else {
                overlap_score(&haystack, &phrase)
            };
            (score >= threshold).then_some((index, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.1.cmp(&left.1));
    scored.into_iter().take(3).map(|(index, _)| index).collect()
}

fn phrase_threshold(phrase: &str) -> usize {
    let count = terms(phrase).len();
    if count <= 1 {
        1
    } else {
        2
    }
}

fn solve_problem(
    problem: &PlanningProblem,
    backend: SatBackend,
    cube_and_conquer: bool,
) -> Result<SolverResult, RbmemError> {
    if cube_and_conquer {
        return solve_cube_and_conquer(problem, backend);
    }

    match backend {
        SatBackend::Kissat => {
            solve_external(problem, "kissat").or_else(|_| solve_internal(problem))
        }
        SatBackend::Cadical => {
            solve_external(problem, "cadical").or_else(|_| solve_internal(problem))
        }
        SatBackend::Auto => solve_external(problem, "kissat")
            .or_else(|_| solve_external(problem, "cadical"))
            .or_else(|_| solve_internal(problem)),
        SatBackend::Internal => solve_internal(problem),
    }
}

fn solve_cube_and_conquer(
    problem: &PlanningProblem,
    backend: SatBackend,
) -> Result<SolverResult, RbmemError> {
    let cube_vars = problem.candidates.len().min(3);
    if cube_vars == 0 {
        return solve_internal(problem);
    }

    for mask in 0..(1usize << cube_vars) {
        let mut cubed = problem.clone();
        for index in 0..cube_vars {
            let var = (index + 1) as i32;
            let lit = if (mask & (1 << index)) != 0 {
                var
            } else {
                -var
            };
            cubed.clauses.push(RuleClause { clause: vec![lit] });
        }
        let result = solve_problem(&cubed, backend_without_cube(backend), false)?;
        if result.status == SatStatus::Sat {
            return Ok(SolverResult {
                solver: format!("cube-and-conquer+{}", result.solver),
                ..result
            });
        }
    }

    Ok(SolverResult {
        status: SatStatus::Unsat,
        assignment: vec![false; problem.candidates.len() + 1],
        solver: "cube-and-conquer+internal".to_string(),
    })
}

fn backend_without_cube(backend: SatBackend) -> SatBackend {
    match backend {
        SatBackend::Auto | SatBackend::Kissat | SatBackend::Cadical => backend,
        SatBackend::Internal => SatBackend::Internal,
    }
}

fn solve_external(problem: &PlanningProblem, binary: &str) -> Result<SolverResult, RbmemError> {
    let dimacs = dimacs_string(problem.candidates.len(), &problem.clauses);
    let path = std::env::temp_dir().join(format!(
        "rbmem-plan-{}-{}.cnf",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, dimacs)?;

    let output = Command::new(binary).arg(&path).output();
    let _ = fs::remove_file(&path);
    let output = output.map_err(|error| RbmemError::Parse(error.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}");

    if text.contains("UNSATISFIABLE") {
        return Ok(SolverResult {
            status: SatStatus::Unsat,
            assignment: vec![false; problem.candidates.len() + 1],
            solver: binary.to_string(),
        });
    }

    if text.contains("SATISFIABLE") {
        let mut assignment = vec![false; problem.candidates.len() + 1];
        for token in text.split_whitespace() {
            if let Ok(lit) = token.parse::<i32>() {
                if lit > 0 && (lit as usize) < assignment.len() {
                    assignment[lit as usize] = true;
                }
            }
        }
        return Ok(SolverResult {
            status: SatStatus::Sat,
            assignment,
            solver: binary.to_string(),
        });
    }

    Err(RbmemError::Parse(format!(
        "{binary} did not return a SAT status"
    )))
}

fn solve_internal(problem: &PlanningProblem) -> Result<SolverResult, RbmemError> {
    let mut assignment = vec![None; problem.candidates.len() + 1];
    let clauses = problem
        .clauses
        .iter()
        .map(|rule| rule.clause.clone())
        .collect::<Vec<_>>();
    let status = dpll(&clauses, &mut assignment);
    let concrete = assignment
        .into_iter()
        .map(|value| value.unwrap_or(false))
        .collect::<Vec<_>>();
    Ok(SolverResult {
        status: if status {
            SatStatus::Sat
        } else {
            SatStatus::Unsat
        },
        assignment: concrete,
        solver: "internal-dpll".to_string(),
    })
}

fn dpll(clauses: &[Vec<i32>], assignment: &mut [Option<bool>]) -> bool {
    loop {
        let mut changed = false;
        for clause in clauses {
            match eval_clause(clause, assignment) {
                ClauseEval::Satisfied => {}
                ClauseEval::Conflict => return false,
                ClauseEval::Unit(lit) => {
                    let var = lit.unsigned_abs() as usize;
                    let value = lit > 0;
                    if let Some(existing) = assignment[var] {
                        if existing != value {
                            return false;
                        }
                    } else {
                        assignment[var] = Some(value);
                        changed = true;
                    }
                }
                ClauseEval::Open => {}
            }
        }
        if !changed {
            break;
        }
    }

    if clauses
        .iter()
        .all(|clause| matches!(eval_clause(clause, assignment), ClauseEval::Satisfied))
    {
        return true;
    }

    let var = assignment.iter().position(Option::is_none);
    let Some(var) = var else {
        return false;
    };
    for value in [true, false] {
        let mut next = assignment.to_vec();
        next[var] = Some(value);
        if dpll(clauses, &mut next) {
            assignment.copy_from_slice(&next);
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseEval {
    Satisfied,
    Conflict,
    Unit(i32),
    Open,
}

fn eval_clause(clause: &[i32], assignment: &[Option<bool>]) -> ClauseEval {
    let mut unassigned = None;
    let mut unassigned_count = 0;

    for &lit in clause {
        let var = lit.unsigned_abs() as usize;
        match assignment.get(var).copied().flatten() {
            Some(value) if value == (lit > 0) => return ClauseEval::Satisfied,
            Some(_) => {}
            None => {
                unassigned = Some(lit);
                unassigned_count += 1;
            }
        }
    }

    match unassigned_count {
        0 => ClauseEval::Conflict,
        1 => ClauseEval::Unit(unassigned.unwrap_or(0)),
        _ => ClauseEval::Open,
    }
}

fn selected_indices(assignment: &[bool], candidates: &[CandidateAction]) -> Vec<usize> {
    let mut selected = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            assignment
                .get(index + 1)
                .copied()
                .unwrap_or(false)
                .then_some((index, candidate.score))
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    selected.into_iter().map(|(index, _)| index).collect()
}

fn minimize_assignment(problem: &PlanningProblem, assignment: &mut [bool]) {
    let mut selected = problem
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            assignment
                .get(index + 1)
                .copied()
                .unwrap_or(false)
                .then_some((index + 1, candidate.score))
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));

    for (var, _) in selected {
        assignment[var] = false;
        if !assignment_satisfies(problem, assignment) {
            assignment[var] = true;
        }
    }
}

fn assignment_satisfies(problem: &PlanningProblem, assignment: &[bool]) -> bool {
    problem.clauses.iter().all(|rule| {
        rule.clause.iter().any(|lit| {
            let var = lit.unsigned_abs() as usize;
            assignment.get(var).copied().unwrap_or(false) == (*lit > 0)
        })
    })
}

fn order_steps(problem: &PlanningProblem, selected: &[usize]) -> Vec<PlanStep> {
    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    let mut indegree = HashMap::<usize, usize>::new();
    let mut outgoing = HashMap::<usize, Vec<usize>>::new();

    for &(action, prerequisite) in &problem.requires {
        if selected_set.contains(&action) && selected_set.contains(&prerequisite) {
            *indegree.entry(action).or_default() += 1;
            outgoing.entry(prerequisite).or_default().push(action);
        }
    }

    let mut ready = selected
        .iter()
        .copied()
        .filter(|index| indegree.get(index).copied().unwrap_or(0) == 0)
        .collect::<Vec<_>>();
    ready.sort_by(|left, right| {
        problem.candidates[*right]
            .score
            .cmp(&problem.candidates[*left].score)
            .then_with(|| left.cmp(right))
    });

    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(index) = ready.pop() {
        if !seen.insert(index) {
            continue;
        }
        ordered.push(index);
        if let Some(children) = outgoing.get(&index) {
            for &child in children {
                let entry = indegree.entry(child).or_default();
                *entry = entry.saturating_sub(1);
                if *entry == 0 {
                    ready.push(child);
                }
            }
        }
    }

    for &index in selected {
        if seen.insert(index) {
            ordered.push(index);
        }
    }

    ordered
        .into_iter()
        .enumerate()
        .map(|(order, index)| PlanStep {
            order: order + 1,
            action: problem.candidates[index].title.clone(),
            source: problem.candidates[index].source_path.clone(),
            score: problem.candidates[index].score,
        })
        .collect()
}

fn handle_proof(
    problem: &PlanningProblem,
    solve: &SolverResult,
    dimacs: &str,
    primary_path: &Path,
    plan_slug: &str,
    options: &PlanOptions,
) -> Result<ProofReport, RbmemError> {
    if !options.proof {
        return Ok(ProofReport {
            requested: false,
            path: None,
            generated: false,
            verified: false,
            verifier: "none".to_string(),
            note: "proof generation not requested".to_string(),
        });
    }

    let proof_path = options.proof_path.clone().unwrap_or_else(|| {
        primary_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".rbmem")
            .join("plans")
            .join(format!("{plan_slug}.drat"))
    });

    if let Some(parent) = proof_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let note = if solve.status == SatStatus::Unsat && contains_empty_clause(problem) {
        fs::write(&proof_path, "0\n")?;
        "generated internal empty-clause DRAT proof".to_string()
    } else if solve.status == SatStatus::Unsat {
        fs::write(&proof_path, "")?;
        "UNSAT found; install Kissat/CaDiCaL plus drat-trim for full DRAT traces".to_string()
    } else {
        fs::write(&proof_path, "")?;
        "SAT plans do not have UNSAT DRAT proofs; DIMACS and model are stored in RBMEM".to_string()
    };

    let (verified, verifier) = if options.verify_proof {
        verify_proof(dimacs, &proof_path)
    } else {
        (false, "not-requested".to_string())
    };

    Ok(ProofReport {
        requested: true,
        path: Some(proof_path.display().to_string()),
        generated: true,
        verified,
        verifier,
        note,
    })
}

fn contains_empty_clause(problem: &PlanningProblem) -> bool {
    problem
        .clauses
        .iter()
        .any(|clause| clause.clause.is_empty())
}

fn verify_proof(dimacs: &str, proof_path: &Path) -> (bool, String) {
    if let Ok(output) = Command::new("drat-trim").arg("-").arg(proof_path).output() {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() || text.to_ascii_uppercase().contains("VERIFIED") {
            return (true, "drat-trim".to_string());
        }
    }

    let proof = fs::read_to_string(proof_path).unwrap_or_default();
    let verified = proof.lines().any(|line| line.trim() == "0") && dimacs.contains(" 0\n");
    (verified, "internal-empty-clause-check".to_string())
}

fn persist_plan(
    document: &mut RbmemDocument,
    path: &Path,
    problem: &PlanningProblem,
    solve: &SolverResult,
    steps: &[PlanStep],
    proof: &ProofReport,
    plan_path: &str,
    dimacs: &str,
    now: DateTime<Utc>,
) -> Result<(), RbmemError> {
    let goal_path = format!("{plan_path}.goal");
    let steps_path = format!("{plan_path}.steps");
    let sat_path = format!("{plan_path}.sat");
    let proof_path = format!("{plan_path}.proof");

    document.upsert_section(&goal_path, SectionType::Text, problem.goal.clone(), now);
    document.upsert_section(&steps_path, SectionType::List, render_steps(steps), now);
    document.upsert_section(
        &sat_path,
        SectionType::Json,
        serde_json::to_string_pretty(&json!({
            "schema": "rbmem.plan.sat.v1",
            "status": solve.status,
            "solver": solve.solver,
            "variables": problem.candidates.len(),
            "clauses": problem.clauses.len(),
            "cube_and_conquer": solve.solver.contains("cube-and-conquer"),
            "selected_variables": steps.iter().map(|step| step.order).collect::<Vec<_>>(),
            "conflicts": problem.conflicts.len(),
            "requires": problem.requires.len(),
            "dimacs": dimacs,
        }))?,
        now,
    );
    document.upsert_section(
        &proof_path,
        SectionType::Json,
        serde_json::to_string_pretty(proof)?,
        now,
    );
    document.upsert_section(
        "timeline",
        SectionType::Timeline,
        format!(
            "{}: SAT plan '{}' produced {:?} with {} step(s).",
            now.to_rfc3339(),
            problem.goal,
            solve.status,
            steps.len()
        ),
        now,
    );

    set_planner_section_metadata(
        document,
        &goal_path,
        "plan_goal",
        vec![
            relation(&steps_path, "has_steps", now),
            relation(&sat_path, "encoded_as", now),
        ],
        now,
    );
    let mut step_relations = Vec::new();
    for source in steps.iter().filter_map(|step| step.source.as_deref()) {
        step_relations.push(relation(source, "uses_context", now));
    }
    set_planner_section_metadata(document, &steps_path, "plan_steps", step_relations, now);
    set_planner_section_metadata(
        document,
        &sat_path,
        "sat_problem",
        vec![
            relation(&goal_path, "solves", now),
            relation(&proof_path, "has_proof", now),
        ],
        now,
    );
    set_planner_section_metadata(
        document,
        &proof_path,
        "sat_proof",
        vec![relation(&sat_path, "verifies", now)],
        now,
    );

    fs::write(path, document.to_rbmem_string())?;
    Ok(())
}

fn relation(to: &str, relation_type: &str, now: DateTime<Utc>) -> GraphRelation {
    GraphRelation {
        to: to.to_string(),
        relation_type: relation_type.to_string(),
        valid_from: Some(now),
        valid_until: None,
        inferred: false,
        confidence: None,
    }
}

fn set_planner_section_metadata(
    document: &mut RbmemDocument,
    path: &str,
    node_type: &str,
    relations: Vec<GraphRelation>,
    now: DateTime<Utc>,
) {
    if let Some(section) = document
        .sections
        .iter_mut()
        .find(|section| section.path == path)
    {
        section.source = Some(SourceInfo {
            kind: "planner".to_string(),
            path: None,
            actor: Some("rbmem plan".to_string()),
            hash: None,
        });
        section.graph = Some(GraphInfo {
            node_type: Some(node_type.to_string()),
            relations,
        });
        section.temporal.updated_at = now;
    }
}

fn render_steps(steps: &[PlanStep]) -> String {
    if steps.is_empty() {
        return "- No satisfying plan was found.".to_string();
    }

    steps
        .iter()
        .map(|step| match &step.source {
            Some(source) => format!("- {}. {} (source: {})", step.order, step.action, source),
            None => format!("- {}. {}", step.order, step.action),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn dimacs_string(vars: usize, clauses: &[RuleClause]) -> String {
    let mut output = format!("p cnf {} {}\n", vars, clauses.len());
    for clause in clauses {
        for lit in &clause.clause {
            output.push_str(&format!("{lit} "));
        }
        output.push_str("0\n");
    }
    output
}

fn plan_slug(goal: &str, now: DateTime<Utc>) -> String {
    let base = normalize(goal)
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    let base = if base.is_empty() {
        "goal".to_string()
    } else {
        base
    };
    format!("{}-{}", base, now.format("%Y%m%d%H%M%S"))
}

fn bullet_text(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let item = trimmed
        .strip_prefix("- [ ] ")
        .or_else(|| trimmed.strip_prefix("- [x] "))
        .or_else(|| trimmed.strip_prefix("- "))
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| {
            let (number, rest) = trimmed.split_once(". ")?;
            number.chars().all(|ch| ch.is_ascii_digit()).then_some(rest)
        })?;
    Some(item.trim().to_string())
}

fn first_sentence(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(content)
        .trim_end_matches('.')
        .to_string()
}

fn overlap_score(left: &str, right: &str) -> usize {
    let left_terms = terms(left);
    let right_terms = terms(right);
    left_terms.intersection(&right_terms).count()
}

fn terms(text: &str) -> BTreeSet<String> {
    normalize(text)
        .split_whitespace()
        .filter(|term| term.len() > 2)
        .filter(|term| !STOP_WORDS.contains(term))
        .map(ToString::to_string)
        .collect()
}

fn normalize(text: &str) -> String {
    let mut output = String::new();
    let mut last_space = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_space = false;
        } else if !last_space {
            output.push(' ');
            last_space = true;
        }
    }
    output.trim().to_string()
}

fn path_has_any(path: &str, needles: &[&str]) -> bool {
    let normalized = normalize(path);
    needles
        .iter()
        .any(|needle| normalized.split_whitespace().any(|part| part == *needle))
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "this", "that", "from", "into", "your", "you", "are", "was",
    "were", "have", "has", "had", "will", "shall", "should", "must", "need", "needs", "goal",
    "task", "plan", "step", "agent", "memory",
];

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 18, 20, 0, 0).unwrap()
    }

    #[test]
    fn internal_solver_respects_requires_and_conflicts() {
        let now = fixed_time();
        let mut doc = RbmemDocument::new(now, "test");
        doc.upsert_section(
            "tasks",
            SectionType::List,
            "- Gather requirements\n- Run tests\n- Deploy release".to_string(),
            now,
        );
        doc.upsert_section(
            "rules",
            SectionType::List,
            "- deploy release requires run tests\n- gather requirements conflicts with deploy release".to_string(),
            now,
        );

        let problem = build_problem(&[doc], Some("deploy release"), false).unwrap();
        let result = solve_internal(&problem).unwrap();
        let selected = selected_indices(&result.assignment, &problem.candidates);
        let steps = order_steps(&problem, &selected);

        assert_eq!(result.status, SatStatus::Sat);
        assert!(steps.iter().any(|step| step.action.contains("Run tests")));
        assert!(steps
            .iter()
            .any(|step| step.action.contains("Deploy release")));
        assert!(!steps
            .iter()
            .any(|step| step.action.contains("Gather requirements")));
    }
}
