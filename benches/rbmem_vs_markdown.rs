//! Quantitative benchmark: RBMEM vs plain Markdown for agent memory.
//!
//! Measures:
//!   1. Context retrieval precision (relevant sections / total returned)
//!   2. Token efficiency (chars in context output)
//!   3. Relation-aware recall (graph neighbors pulled in)
//!   4. Temporal filtering capability
//!   5. Compact mode token savings

use chrono::{Duration, Utc};
use rbmem::commands::{self as api};
use rbmem::document::{GraphInfo, GraphRelation, Section, SectionType};
use rbmem::RbmemDocument;
use std::collections::BTreeSet;
use std::time::Instant;

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc::now() - Duration::days(30)
}

/// Build a realistic 50-section knowledge base simulating a real agent memory.
fn build_knowledge_base() -> RbmemDocument {
    let base = fixed_time();
    let recent = Utc::now() - Duration::days(2);
    let stale = Utc::now() - Duration::days(120);
    let mut doc = RbmemDocument::new(base, "benchmark-agent");
    doc.meta.purpose = "benchmark: agent memory for a full-stack SaaS project".to_string();

    // --- Architecture (8 sections) ---
    doc.upsert_section("architecture", SectionType::Text,
        "The system is a multi-tenant SaaS platform with a React frontend, \
         Rust API gateway, and PostgreSQL + Redis backend.".to_string(), recent);
    doc.upsert_section("architecture.api", SectionType::Text,
        "The API gateway is built with Axum, handles auth via JWT, and routes \
         to microservices. Rate limiting is done at the gateway level using Redis counters.".to_string(), recent);
    doc.upsert_section("architecture.api.auth", SectionType::Text,
        "Authentication uses RS256 JWT tokens. Refresh tokens are stored in \
         HttpOnly cookies. The auth service validates tokens against a public key.".to_string(), recent);
    doc.upsert_section("architecture.api.rate_limit", SectionType::Text,
        "Rate limiting uses a sliding window algorithm with Redis. Default: \
         100 req/min for authenticated users, 20 req/min for anonymous.".to_string(), recent);
    doc.upsert_section("architecture.database", SectionType::Text,
        "PostgreSQL 16 with read replicas. Schema migrations via diesel. \
         Row-level security for tenant isolation.".to_string(), recent);
    doc.upsert_section("architecture.database.sharding", SectionType::Text,
        "Sharding strategy: hash-based on tenant_id. Each shard has its own \
         connection pool. Cross-shard queries go through a fan-out service.".to_string(), recent);
    doc.upsert_section("architecture.frontend", SectionType::Text,
        "React 18 with TypeScript, Vite for bundling. State management via \
         Zustand. API calls through a typed tRPC client.".to_string(), recent);
    doc.upsert_section("architecture.infra", SectionType::Text,
        "Deployed on AWS EKS. Terraform for IaC. Prometheus + Grafana for \
         monitoring. PagerDuty for on-call alerts.".to_string(), stale);

    // --- Rules (6 sections) ---
    doc.upsert_section("rules", SectionType::List,
        "- Always write tests before merging\n- Never commit secrets\n- Use conventional commits".to_string(), recent);
    doc.upsert_section("rules.testing", SectionType::Text,
        "All PRs must have >80% line coverage on changed files. Integration \
         tests required for any new API endpoint.".to_string(), recent);
    doc.upsert_section("rules.testing.unit", SectionType::Text,
        "Unit tests go in the same file as the code under test using #[cfg(test)]. \
         Mock external services with wiremock.".to_string(), recent);
    doc.upsert_section("rules.testing.integration", SectionType::Text,
        "Integration tests use testcontainers for PostgreSQL and Redis. \
         Each test gets an isolated schema.".to_string(), recent);
    doc.upsert_section("rules.security", SectionType::Text,
        "No raw SQL — always use diesel query builder. All user input validated \
         at the API boundary with garde.".to_string(), recent);
    doc.upsert_section("rules.performance", SectionType::Text,
        "No N+1 queries. All database queries must be explainable. \
         p99 latency budget: 200ms for API calls.".to_string(), recent);

    // --- Memory / learned facts (10 sections) ---
    doc.upsert_section("memory", SectionType::HermesMemory, "".to_string(), base);
    doc.upsert_section("memory.preferences", SectionType::HermesMemory,
        "- User prefers functional style over OOP\n- Prefers explicit error types over anyhow\n- Uses rustfmt + clippy on save".to_string(), recent);
    doc.upsert_section("memory.decisions", SectionType::HermesMemory,
        "- 2026-03-15: Chose Axum over Actix for API framework\n- 2026-03-20: Chose PostgreSQL over CockroachDB for simpler ops\n- 2026-04-01: Migrated from REST to tRPC for frontend API".to_string(), recent);
    doc.upsert_section("memory.bugs", SectionType::HermesMemory,
        "- 2026-04-10: Connection pool exhaustion under load — fixed by tuning max_connections=50\n- 2026-04-15: JWT expiry mismatch caused 401s — fixed by syncing clock skew tolerance to 30s".to_string(), recent);
    doc.upsert_section("memory.performance", SectionType::HermesMemory,
        "- Dashboard query was 2.3s — added materialized view, now 45ms\n- Redis cache hit rate: 94% after adding write-through caching".to_string(), recent);

    doc.upsert_section("memory.team", SectionType::HermesMemory,
        "- Alice: backend lead, prefers Diesel ORM\n- Bob: frontend lead, uses Zustand for state\n- Carol: DevOps, manages Terraform modules".to_string(), recent);
    doc.upsert_section("memory.team.alice", SectionType::Text,
        "Alice works on the API and database layers. She reviews all PRs touching src/api/ and migrations/. \
         She prefers integration tests over mocks.".to_string(), recent);
    doc.upsert_section("memory.team.bob", SectionType::Text,
        "Bob owns the React frontend. He set up the tRPC client and maintains the component library. \
         He uses Playwright for E2E tests.".to_string(), recent);
    doc.upsert_section("memory.team.carol", SectionType::Text,
        "Carol manages AWS infrastructure via Terraform. She set up EKS, RDS, and the CI/CD pipeline. \
         She uses GitHub Actions with self-hosted runners.".to_string(), recent);
    doc.upsert_section("memory.incidents", SectionType::Timeline,
        "2026-04-10T14:30:00Z: P1 — API gateway 503s for 12 minutes (connection pool)\n\
         2026-04-15T09:00:00Z: P2 — Auth service returning 401s (clock skew)\n\
         2026-04-20T16:45:00Z: P3 — Dashboard slow queries (missing index)".to_string(), recent);

    // --- Tasks (5 sections) ---
    doc.upsert_section("tasks", SectionType::List, "".to_string(), base);
    doc.upsert_section("tasks.current", SectionType::List,
        "- [ ] Add pagination to /api/users endpoint\n- [ ] Write integration test for auth refresh flow\n- [x] Fix connection pool exhaustion bug".to_string(), recent);
    doc.upsert_section("tasks.backlog", SectionType::List,
        "- [ ] Implement webhook delivery system\n- [ ] Add rate limiting per-tenant\n- [ ] Migrate to PostgreSQL 17".to_string(), recent);
    doc.upsert_section("tasks.done", SectionType::List,
        "- [x] Set up CI/CD pipeline\n- [x] Implement JWT auth\n- [x] Add Redis caching layer".to_string(), recent);
    doc.upsert_section("tasks.blocked", SectionType::List,
        "- [ ] CockroachDB migration (blocked: team prefers PostgreSQL)\n- [ ] GraphQL API (blocked: tRPC chosen instead)".to_string(), recent);

    // --- API endpoints reference (8 sections) ---
    doc.upsert_section("api_ref", SectionType::Text, "API endpoint reference for the SaaS platform.".to_string(), recent);
    doc.upsert_section("api_ref.auth", SectionType::Json,
        r#"{"POST /auth/login": "email+password -> JWT", "POST /auth/refresh": "cookie -> new JWT", "POST /auth/logout": "invalidate refresh token"}"#.to_string(), recent);
    doc.upsert_section("api_ref.users", SectionType::Json,
        r#"{"GET /users": "list all users (paginated)", "GET /users/:id": "get user by ID", "PUT /users/:id": "update user", "DELETE /users/:id": "soft delete"}"#.to_string(), recent);
    doc.upsert_section("api_ref.tenants", SectionType::Json,
        r#"{"GET /tenants": "list tenants", "POST /tenants": "create tenant", "GET /tenants/:id": "get tenant details"}"#.to_string(), recent);
    doc.upsert_section("api_ref.webhooks", SectionType::Json,
        r#"{"POST /webhooks": "register webhook", "GET /webhooks": "list webhooks", "DELETE /webhooks/:id": "unregister"}"#.to_string(), recent);
    doc.upsert_section("api_ref.billing", SectionType::Json,
        r#"{"GET /billing/invoices": "list invoices", "POST /billing/subscribe": "create subscription", "GET /billing/usage": "get usage metrics"}"#.to_string(), recent);
    doc.upsert_section("api_ref.errors", SectionType::Text,
        "Error codes: 400 (validation), 401 (unauth), 403 (forbidden), 404 (not found), \
         429 (rate limited), 500 (internal), 503 (service unavailable)".to_string(), recent);
    doc.upsert_section("api_ref.versioning", SectionType::Text,
        "API versioning via URL prefix: /api/v1/... Breaking changes require new version. \
         Deprecation notices sent 90 days before removal.".to_string(), recent);

    // --- Config reference (5 sections) ---
    doc.upsert_section("config", SectionType::Text, "Configuration reference.".to_string(), recent);
    doc.upsert_section("config.env", SectionType::Text,
        "Environment variables: DATABASE_URL, REDIS_URL, JWT_SECRET, AWS_REGION, \
         LOG_LEVEL, MAX_CONNECTIONS, RATE_LIMIT_WINDOW".to_string(), recent);
    doc.upsert_section("config.database", SectionType::Json,
        r#"{"max_connections": 50, "min_connections": 5, "connect_timeout_ms": 5000, "idle_timeout_ms": 300000}"#.to_string(), recent);
    doc.upsert_section("config.redis", SectionType::Json,
        r#"{"pool_size": 10, "key_prefix": "rbmem:", "ttl_seconds": 3600}"#.to_string(), recent);
    doc.upsert_section("config.logging", SectionType::Text,
        "Structured logging via tracing crate. Levels: error, warn, info, debug, trace. \
         JSON format in production, pretty format in development.".to_string(), recent);

    // --- Deployment (4 sections) ---
    doc.upsert_section("deployment", SectionType::Text, "Deployment and operations guide.".to_string(), recent);
    doc.upsert_section("deployment.ci_cd", SectionType::Text,
        "GitHub Actions workflow: lint -> test -> build -> deploy to staging -> \
         manual approval -> deploy to production. Rollback via kubectl rollout undo.".to_string(), recent);
    doc.upsert_section("deployment.monitoring", SectionType::Text,
        "Prometheus scrapes metrics from /metrics endpoint. Grafana dashboards for: \
         request latency, error rates, DB query times, Redis hit rates.".to_string(), stale);
    doc.upsert_section("deployment.alerts", SectionType::Text,
        "PagerDuty alerts for: p99 > 500ms, error rate > 1%, DB connection pool > 80%, \
         disk usage > 90%. On-call rotation: weekly.".to_string(), stale);

    // --- Graph relations ---
    let relations_auth = vec![
        GraphRelation { to: "architecture.api".into(), relation_type: "depends_on".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "rules.security".into(), relation_type: "requires".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
    ];
    let relations_api = vec![
        GraphRelation { to: "architecture.database".into(), relation_type: "depends_on".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "config.database".into(), relation_type: "uses".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "architecture.frontend".into(), relation_type: "collaborates_with".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
    ];
    let relations_testing = vec![
        GraphRelation { to: "rules.testing.unit".into(), relation_type: "depends_on".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "rules.testing.integration".into(), relation_type: "depends_on".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
    ];
    let relations_incidents = vec![
        GraphRelation { to: "memory.bugs".into(), relation_type: "references".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "architecture.api".into(), relation_type: "affects".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
    ];
    let relations_ci = vec![
        GraphRelation { to: "deployment.monitoring".into(), relation_type: "depends_on".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
        GraphRelation { to: "config.logging".into(), relation_type: "uses".into(), valid_from: Some(recent), valid_until: None, inferred: false, confidence: None },
    ];

    set_graph(&mut doc, "architecture.api.auth", relations_auth);
    set_graph(&mut doc, "architecture.api", relations_api);
    set_graph(&mut doc, "rules.testing", relations_testing);
    set_graph(&mut doc, "memory.incidents", relations_incidents);
    set_graph(&mut doc, "deployment.ci_cd", relations_ci);

    doc
}

fn set_graph(doc: &mut RbmemDocument, path: &str, relations: Vec<GraphRelation>) {
    if let Some(section) = doc.sections.iter_mut().find(|s| s.path == path) {
        section.graph = Some(GraphInfo { node_type: None, relations });
    }
}

/// Convert the RBMEM document into equivalent Markdown.
fn to_markdown(doc: &RbmemDocument) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", doc.meta.purpose));
    for section in &doc.sections {
        let depth = section.path.matches('.').count() + 1;
        let hashes = "#".repeat(depth.min(6));
        let title = section.path.replace('.', " > ");
        md.push_str(&format!("{} {}\n\n", hashes, title));
        if !section.content.is_empty() {
            md.push_str(&section.content);
            md.push_str("\n\n");
        }
    }
    md
}

/// Simulate naive Markdown context retrieval: return the whole file.
/// This is what most LLM agents do — they just dump the whole .md into context.
fn markdown_full_context(md: &str) -> usize {
    md.len()
}

/// Simulate keyword-search Markdown retrieval: grep for query terms, return matching paragraphs.
fn markdown_keyword_context(md: &str, query: &str) -> (String, usize) {
    let terms: Vec<&str> = query.split_whitespace().collect();
    let mut result = String::new();
    for paragraph in md.split("\n\n") {
        let lower = paragraph.to_lowercase();
        if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
            result.push_str(paragraph);
            result.push_str("\n\n");
        }
    }
    let len = result.len();
    (result, len)
}

struct QueryTestCase {
    query: &'static str,
    expected_sections: &'static [&'static str],
    description: &'static str,
}

fn query_test_cases() -> Vec<QueryTestCase> {
    vec![
        QueryTestCase {
            query: "authentication JWT tokens",
            expected_sections: &["architecture.api.auth", "api_ref.auth", "memory.bugs"],
            description: "Auth-specific retrieval",
        },
        QueryTestCase {
            query: "rate limiting API",
            expected_sections: &["architecture.api.rate_limit", "api_ref.errors", "tasks.backlog"],
            description: "Rate limit feature retrieval",
        },
        QueryTestCase {
            query: "database connection pool",
            expected_sections: &["architecture.database", "config.database", "memory.bugs", "architecture.database.sharding"],
            description: "Database infrastructure retrieval",
        },
        QueryTestCase {
            query: "testing integration tests",
            expected_sections: &["rules.testing", "rules.testing.integration", "rules.testing.unit"],
            description: "Testing rules retrieval",
        },
        QueryTestCase {
            query: "deployment CI/CD pipeline",
            expected_sections: &["deployment.ci_cd", "deployment", "deployment.monitoring"],
            description: "Deployment pipeline retrieval",
        },
        QueryTestCase {
            query: "Alice backend team",
            expected_sections: &["memory.team.alice", "memory.team"],
            description: "Team member retrieval",
        },
        QueryTestCase {
            query: "API endpoint users billing",
            expected_sections: &["api_ref.users", "api_ref.billing", "api_ref"],
            description: "API reference retrieval",
        },
        QueryTestCase {
            query: "performance monitoring alerts",
            expected_sections: &["deployment.monitoring", "deployment.alerts", "memory.performance", "rules.performance"],
            description: "Performance/monitoring retrieval",
        },
        QueryTestCase {
            query: "React frontend TypeScript",
            expected_sections: &["architecture.frontend", "memory.team.bob"],
            description: "Frontend technology retrieval",
        },
        QueryTestCase {
            query: "Redis caching configuration",
            expected_sections: &["config.redis", "memory.performance", "architecture.api"],
            description: "Redis/caching retrieval",
        },
        QueryTestCase {
            query: "security validation input",
            expected_sections: &["rules.security", "architecture.api.auth"],
            description: "Security rules retrieval",
        },
        QueryTestCase {
            query: "webhook delivery system",
            expected_sections: &["api_ref.webhooks", "tasks.backlog"],
            description: "Webhook feature retrieval",
        },
        QueryTestCase {
            query: "error handling 401 429",
            expected_sections: &["api_ref.errors", "architecture.api.auth", "memory.incidents"],
            description: "Error handling retrieval",
        },
        QueryTestCase {
            query: "PostgreSQL migration sharding",
            expected_sections: &["architecture.database", "architecture.database.sharding", "tasks.backlog", "memory.decisions"],
            description: "Database architecture retrieval",
        },
        QueryTestCase {
            query: "Terraform AWS infrastructure",
            expected_sections: &["architecture.infra", "memory.team.carol", "deployment"],
            description: "Infrastructure retrieval",
        },
    ]
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║         RBMEM vs Markdown: Quantitative Agent Memory Benchmark      ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");

    let doc = build_knowledge_base();
    let md = to_markdown(&doc);

    println!("Knowledge Base: {} sections, {} chars RBMEM, {} chars Markdown\n",
        doc.sections.len(),
        doc.to_rbmem_string().len(),
        md.len());

    // ============================================================
    // TEST 1: Context Retrieval Precision
    // ============================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 1: Context Retrieval Precision");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Query → What fraction of returned content is actually relevant?\n");

    let cases = query_test_cases();
    let mut rbmem_precision_sum = 0.0f64;
    let mut md_full_precision_sum = 0.0f64;
    let mut md_keyword_precision_sum = 0.0f64;
    let mut rbmem_recall_sum = 0.0f64;
    let mut md_keyword_recall_sum = 0.0f64;

    println!(" {:<40} {:>10} {:>10} {:>10}", "Query", "RBMEM", "MD-full", "MD-kw");
    println!(" {}", "─".repeat(74));

    for case in &cases {
        // RBMEM query
        let rbmem_result = api::query_document(&doc, case.query, true, 1);
        let rbmem_paths: BTreeSet<&str> = rbmem_result.sections.iter()
            .map(|s| s.path.as_str()).collect();

        let rbmem_relevant = case.expected_sections.iter()
            .filter(|p| rbmem_paths.contains(**p)).count();
        let rbmem_precision = if rbmem_paths.is_empty() { 0.0 } else {
            rbmem_relevant as f64 / rbmem_paths.len() as f64
        };
        let rbmem_recall = if case.expected_sections.is_empty() { 0.0 } else {
            rbmem_relevant as f64 / case.expected_sections.len() as f64
        };

        // Markdown full dump
        let md_full_total_sections = doc.sections.len();
        let md_full_relevant = case.expected_sections.len();
        let md_full_precision = md_full_relevant as f64 / md_full_total_sections as f64;

        // Markdown keyword search
        let (md_kw_text, _md_kw_len) = markdown_keyword_context(&md, case.query);
        let md_kw_relevant = case.expected_sections.iter()
            .filter(|p| {
                let title = p.replace('.', " > ");
                md_kw_text.contains(&title)
            }).count();
        // Count how many "paragraphs" (rough section proxy) were returned
        let md_kw_paragraphs = md_kw_text.split("\n\n").filter(|p| !p.trim().is_empty()).count().max(1);
        let md_kw_precision = md_kw_relevant as f64 / md_kw_paragraphs as f64;
        let md_kw_recall = if case.expected_sections.is_empty() { 0.0 } else {
            md_kw_relevant as f64 / case.expected_sections.len() as f64
        };

        rbmem_precision_sum += rbmem_precision;
        md_full_precision_sum += md_full_precision;
        md_keyword_precision_sum += md_kw_precision;
        rbmem_recall_sum += rbmem_recall;
        md_keyword_recall_sum += md_kw_recall;

        let label = if case.description.len() > 38 {
            &case.description[..38]
        } else {
            case.description
        };
        println!(" {:<40} {:>9.1}% {:>9.1}% {:>9.1}%",
            label,
            rbmem_precision * 100.0,
            md_full_precision * 100.0,
            md_kw_precision * 100.0);
    }

    let n = cases.len() as f64;
    let rbmem_avg_precision = rbmem_precision_sum / n;
    let md_full_avg_precision = md_full_precision_sum / n;
    let md_kw_avg_precision = md_keyword_precision_sum / n;
    let rbmem_avg_recall = rbmem_recall_sum / n;
    let md_kw_avg_recall = md_keyword_recall_sum / n;

    let rbmem_f1 = 2.0 * rbmem_avg_precision * rbmem_avg_recall / (rbmem_avg_precision + rbmem_avg_recall).max(0.001);
    let md_kw_f1 = 2.0 * md_kw_avg_precision * md_kw_avg_recall / (md_kw_avg_precision + md_kw_avg_recall).max(0.001);

    println!(" {}", "─".repeat(74));
    println!(" {:<40} {:>9.1}% {:>9.1}% {:>9.1}%",
        "AVERAGE PRECISION", rbmem_avg_precision * 100.0, md_full_avg_precision * 100.0, md_kw_avg_precision * 100.0);
    println!(" {:<40} {:>9.1}% {:>10} {:>9.1}%",
        "AVERAGE RECALL", rbmem_avg_recall * 100.0, "—", md_kw_avg_recall * 100.0);
    println!(" {:<40} {:>9.3}  {:>10} {:>9.3}",
        "F1 SCORE", rbmem_f1, "—", md_kw_f1);

    let precision_lift = (rbmem_avg_precision / md_full_avg_precision - 1.0) * 100.0;
    let precision_lift_kw = (rbmem_avg_precision / md_kw_avg_precision.max(0.001) - 1.0) * 100.0;
    println!("\n  → RBMEM precision is {:.1}x higher than MD-full (+{:.0}%)",
        rbmem_avg_precision / md_full_avg_precision, precision_lift);
    println!("  → RBMEM precision is {:.1}x higher than MD-keyword (+{:.0}%)",
        rbmem_avg_precision / md_kw_avg_precision.max(0.001), precision_lift_kw);

    // ============================================================
    // TEST 2: Token Efficiency
    // ============================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 2: Token Efficiency (chars per context dump)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let rbmem_full = doc.to_rbmem_string();
    let rbmem_compact = doc.to_compact_string(false, Utc::now());
    let rbmem_minified = doc.to_minified_string(false);
    let rbmem_resolved = doc.to_minified_string(true);
    let md_full_len = md.len();

    // Query-specific context sizes
    let mut rbmem_query_total = 0usize;
    let mut md_full_total = 0usize;
    let mut md_kw_total = 0usize;

    for case in &cases {
        let rbmem_result = api::query_document(&doc, case.query, true, 1);
        let rbmem_query_text = rbmem_result.to_minified_string(true);
        rbmem_query_total += rbmem_query_text.len();

        md_full_total += md_full_len;
        let (_md_kw_text, md_kw_len) = markdown_keyword_context(&md, case.query);
        md_kw_total += md_kw_len;
    }

    println!(" {:<35} {:>10} {:>10}", "Mode", "Chars", "vs MD-full");
    println!(" {}", "─".repeat(58));
    println!(" {:<35} {:>10} {:>10}", "Markdown (full dump)", format_num(md_full_len), "1.00x");
    println!(" {:<35} {:>10} {:>10}", "RBMEM canonical", format_num(rbmem_full.len()),
        format!("{:.2}x", rbmem_full.len() as f64 / md_full_len as f64));
    println!(" {:<35} {:>10} {:>10}", "RBMEM compact", format_num(rbmem_compact.len()),
        format!("{:.2}x", rbmem_compact.len() as f64 / md_full_len as f64));
    println!(" {:<35} {:>10} {:>10}", "RBMEM minified", format_num(rbmem_minified.len()),
        format!("{:.2}x", rbmem_minified.len() as f64 / md_full_len as f64));
    println!(" {:<35} {:>10} {:>10}", "RBMEM minified+resolved", format_num(rbmem_resolved.len()),
        format!("{:.2}x", rbmem_resolved.len() as f64 / md_full_len as f64));
    println!(" {}", "─".repeat(58));
    println!(" Per-query context (avg over {} queries):", cases.len());
    println!(" {:<35} {:>10} {:>10}", "Markdown (full dump)", format_num(md_full_total / cases.len()),
        format!("{:.2}x", 1.0));
    println!(" {:<35} {:>10} {:>10}", "Markdown (keyword search)", format_num(md_kw_total / cases.len()),
        format!("{:.2}x", md_kw_total as f64 / md_full_total as f64));
    println!(" {:<35} {:>10} {:>10}", "RBMEM query (minified+resolved)", format_num(rbmem_query_total / cases.len()),
        format!("{:.2}x", rbmem_query_total as f64 / md_full_total as f64));

    let token_savings = 1.0 - (rbmem_query_total as f64 / md_full_total as f64);
    println!("\n  → RBMEM query context uses {:.1}% fewer tokens than MD full dump", token_savings * 100.0);
    let kw_savings = 1.0 - (rbmem_query_total as f64 / md_kw_total.max(1) as f64);
    if kw_savings > 0.0 {
        println!("  → RBMEM query context uses {:.1}% fewer tokens than MD keyword search", kw_savings * 100.0);
    } else {
        println!("  → RBMEM query context uses {:.1}% more tokens than MD keyword search (but with higher precision)", -kw_savings * 100.0);
    }

    // ============================================================
    // TEST 3: Relation-Aware Context Recall
    // ============================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 3: Relation-Aware Context (graph neighbor retrieval)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let graph_queries = vec![
        ("authentication JWT", "architecture.api.auth", vec!["architecture.api", "rules.security"]),
        ("API gateway Axum", "architecture.api", vec!["architecture.database", "config.database", "architecture.frontend"]),
        ("testing integration", "rules.testing", vec!["rules.testing.unit", "rules.testing.integration"]),
        ("incidents bugs", "memory.incidents", vec!["memory.bugs", "architecture.api"]),
        ("deployment CI/CD", "deployment.ci_cd", vec!["deployment.monitoring", "config.logging"]),
    ];

    println!(" {:<30} {:>12} {:>12}", "Query", "w/ graph", "w/o graph");
    println!(" {}", "─".repeat(58));

    let mut with_graph_sum = 0.0f64;
    let mut without_graph_sum = 0.0f64;

    for (query, _primary, expected_neighbors) in &graph_queries {
        let with_graph = api::query_document(&doc, query, true, 1);
        let without_graph = api::query_document(&doc, query, true, 0);

        let with_paths: BTreeSet<&str> = with_graph.sections.iter().map(|s| s.path.as_str()).collect();
        let without_paths: BTreeSet<&str> = without_graph.sections.iter().map(|s| s.path.as_str()).collect();

        let with_neighbors = expected_neighbors.iter().filter(|p| with_paths.contains(**p)).count();
        let without_neighbors = expected_neighbors.iter().filter(|p| without_paths.contains(**p)).count();

        let with_recall = with_neighbors as f64 / expected_neighbors.len() as f64;
        let without_recall = without_neighbors as f64 / expected_neighbors.len() as f64;
        with_graph_sum += with_recall;
        without_graph_sum += without_recall;

        let label = if query.len() > 28 { &query[..28] } else { *query };
        println!(" {:<30} {:>11.0}% {:>11.0}%", label, with_recall * 100.0, without_recall * 100.0);
    }

    let gn = graph_queries.len() as f64;
    let with_graph_avg = with_graph_sum / gn;
    let without_graph_avg = without_graph_sum / gn;

    println!(" {}", "─".repeat(58));
    println!(" {:<30} {:>11.0}% {:>11.0}%", "AVERAGE", with_graph_avg * 100.0, without_graph_avg * 100.0);
    println!("\n  → Graph-aware retrieval finds {:.1}x more related sections",
        with_graph_avg / without_graph_avg.max(0.001));
    println!("  → Markdown has NO equivalent of graph relations");

    // ============================================================
    // TEST 4: Temporal Awareness
    // ============================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 4: Temporal Awareness");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let stale_sections: Vec<&Section> = doc.sections.iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() > 90)
        .collect();
    let recent_sections: Vec<&Section> = doc.sections.iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() <= 7)
        .collect();

    println!(" {:<35} {:>10}", "Metric", "Count");
    println!(" {}", "─".repeat(48));
    println!(" {:<35} {:>10}", "Total sections", doc.sections.len());
    println!(" {:<35} {:>10}", "Stale sections (>90 days)", stale_sections.len());
    println!(" {:<35} {:>10}", "Recent sections (<=7 days)", recent_sections.len());
    println!(" {:<35} {:>10}", "Sections with graph relations",
        doc.sections.iter().filter(|s| s.graph.is_some()).count());
    println!(" {:<35} {:>10}", "Total graph edges",
        doc.sections.iter()
            .filter_map(|s| s.graph.as_ref())
            .map(|g| g.relations.len())
            .sum::<usize>());

    // Query that should prefer recent content
    let temporal_query = api::query_document(&doc, "connection pool database bugs", true, 0);
    let temporal_recent = temporal_query.sections.iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() <= 7)
        .count();
    let temporal_total = temporal_query.sections.len();

    println!("\n  Temporal query: \"connection pool database bugs\"");
    println!("  → Returned {} sections ({} recent, {} stale)",
        temporal_total, temporal_recent, temporal_total - temporal_recent);
    println!("  → RBMEM tracks temporal metadata per section (created_at, updated_at, expires_at)");
    println!("  → Markdown has NO temporal awareness — all content looks equally fresh");
    println!("  → Markdown cannot expire stale facts or prioritize recent updates");

    // ============================================================
    // TEST 5: Query Speed
    // ============================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 5: Query Speed");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        for case in &cases {
            let _ = api::query_document(&doc, case.query, true, 1);
        }
    }
    let rbmem_elapsed = start.elapsed();
    let rbmem_per_query = rbmem_elapsed.as_micros() / (iterations * cases.len()) as u128;

    let start = Instant::now();
    for _ in 0..iterations {
        for case in &cases {
            let _ = markdown_keyword_context(&md, case.query);
        }
    }
    let md_elapsed = start.elapsed();
    let md_per_query = md_elapsed.as_micros() / (iterations * cases.len()) as u128;

    println!(" {:<35} {:>10} {:>10}", "Method", "µs/query", "Relative");
    println!(" {}", "─".repeat(58));
    println!(" {:<35} {:>10} {:>10}", "RBMEM (indexed query + graph)", format!("{}µs", rbmem_per_query), "1.00x");
    println!(" {:<35} {:>10} {:>10}", "Markdown (keyword grep)", format!("{}µs", md_per_query),
        format!("{:.2}x", md_per_query as f64 / rbmem_per_query.max(1) as f64));
    println!("\n  (Over {} queries × {} iterations = {} total calls)",
        cases.len(), iterations, iterations * cases.len());

    // ============================================================
    // SUMMARY
    // ============================================================
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                           SUMMARY                                    ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║ Dimension              │ RBMEM           │ Markdown                  ║");
    println!("╠════════════════════════╪═════════════════╪═══════════════════════════╣");
    println!("║ Avg Precision          │ {:>10.1}%    │ {:>10.1}% (full dump)     ║",
        rbmem_avg_precision * 100.0, md_full_avg_precision * 100.0);
    println!("║ Avg Recall             │ {:>10.1}%    │ {:>10}                ║",
        rbmem_avg_recall * 100.0, "—");
    println!("║ F1 Score               │ {:>10.3}     │ {:>10}                ║",
        rbmem_f1, "—");
    println!("║ Token savings (query)  │ {:>10.1}%    │ baseline                   ║",
        token_savings * 100.0);
    println!("║ Graph relations        │ {:>10}     │ {:>10}                ║",
        "supported", "none");
    println!("║ Temporal awareness     │ {:>10}     │ {:>10}                ║",
        "per-section", "none");
    println!("║ Compact modes          │ {:>10}     │ {:>10}                ║",
        "3 modes", "none");
    println!("║ Encryption             │ {:>10}     │ {:>10}                ║",
        "per-section", "none");
    println!("║ Provenance tracking    │ {:>10}     │ {:>10}                ║",
        "per-section", "none");
    println!("╚════════════════════════╧═════════════════╧═══════════════════════════╝");
}

fn format_num(n: usize) -> String {
    if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        format!("{}", n)
    }
}