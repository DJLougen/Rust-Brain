//! Quantitative benchmark: RBMEM vs plain Markdown for agent memory.
//!
//! Measures:
//!   1. Context retrieval precision (relevant sections / total returned)
//!   2. Token efficiency (chars in context output)
//!   3. Relation-aware recall (graph neighbors pulled in)
//!   4. Temporal filtering capability
//!   5. Compact mode token savings
//!   6. Graph traversal performance (index build, BFS at varying depths)
//!   7. Planner execution time with large constraint sets
//!   8. Document parsing/loading time
use chrono::{Duration, Utc};
use rbmem::commands::{self as api};
use rbmem::document::TimestampPolicy;
use rbmem::document::{GraphInfo, GraphRelation, Section, SectionType};
use rbmem::parser::parse_document;
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
    doc.upsert_section(
        "architecture",
        SectionType::Text,
        "The system is a multi-tenant SaaS platform with a React frontend, \
         Rust API gateway, and PostgreSQL + Redis backend."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.api",
        SectionType::Text,
        "The API gateway is built with Axum, handles auth via JWT, and routes \
         to microservices. Rate limiting is done at the gateway level using Redis counters."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.api.auth",
        SectionType::Text,
        "Authentication uses RS256 JWT tokens. Refresh tokens are stored in \
         HttpOnly cookies. The auth service validates tokens against a public key."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.api.rate_limit",
        SectionType::Text,
        "Rate limiting uses a sliding window algorithm with Redis. Default: \
         100 req/min for authenticated users, 20 req/min for anonymous."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.database",
        SectionType::Text,
        "PostgreSQL 16 with read replicas. Schema migrations via diesel. \
         Row-level security for tenant isolation."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.database.sharding",
        SectionType::Text,
        "Sharding strategy: hash-based on tenant_id. Each shard has its own \
         connection pool. Cross-shard queries go through a fan-out service."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.frontend",
        SectionType::Text,
        "React 18 with TypeScript, Vite for bundling. State management via \
         Zustand. API calls through a typed tRPC client."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "architecture.infra",
        SectionType::Text,
        "Deployed on AWS EKS. Terraform for IaC. Prometheus + Grafana for \
         monitoring. PagerDuty for on-call alerts."
            .to_string(),
        stale,
    );

    // --- Rules (6 sections) ---
    doc.upsert_section(
        "rules",
        SectionType::List,
        "- Always write tests before merging\n- Never commit secrets\n- Use conventional commits"
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "rules.testing",
        SectionType::Text,
        "All PRs must have >80% line coverage on changed files. Integration \
         tests required for any new API endpoint."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "rules.testing.unit",
        SectionType::Text,
        "Unit tests go in the same file as the code under test using #[cfg(test)]. \
         Mock external services with wiremock."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "rules.testing.integration",
        SectionType::Text,
        "Integration tests use testcontainers for PostgreSQL and Redis. \
         Each test gets an isolated schema."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "rules.security",
        SectionType::Text,
        "No raw SQL — always use diesel query builder. All user input validated \
         at the API boundary with garde."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "rules.performance",
        SectionType::Text,
        "No N+1 queries. All database queries must be explainable. \
         p99 latency budget: 200ms for API calls."
            .to_string(),
        recent,
    );

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
    doc.upsert_section(
        "memory.incidents",
        SectionType::Timeline,
        "2026-04-10T14:30:00Z: P1 — API gateway 503s for 12 minutes (connection pool)\n\
         2026-04-15T09:00:00Z: P2 — Auth service returning 401s (clock skew)\n\
         2026-04-20T16:45:00Z: P3 — Dashboard slow queries (missing index)"
            .to_string(),
        recent,
    );

    // --- Tasks (5 sections) ---
    doc.upsert_section("tasks", SectionType::List, "".to_string(), base);
    doc.upsert_section("tasks.current", SectionType::List,
        "- [ ] Add pagination to /api/users endpoint\n- [ ] Write integration test for auth refresh flow\n- [x] Fix connection pool exhaustion bug".to_string(), recent);
    doc.upsert_section("tasks.backlog", SectionType::List,
        "- [ ] Implement webhook delivery system\n- [ ] Add rate limiting per-tenant\n- [ ] Migrate to PostgreSQL 17".to_string(), recent);
    doc.upsert_section(
        "tasks.done",
        SectionType::List,
        "- [x] Set up CI/CD pipeline\n- [x] Implement JWT auth\n- [x] Add Redis caching layer"
            .to_string(),
        recent,
    );
    doc.upsert_section("tasks.blocked", SectionType::List,
        "- [ ] CockroachDB migration (blocked: team prefers PostgreSQL)\n- [ ] GraphQL API (blocked: tRPC chosen instead)".to_string(), recent);

    // --- API endpoints reference (8 sections) ---
    doc.upsert_section(
        "api_ref",
        SectionType::Text,
        "API endpoint reference for the SaaS platform.".to_string(),
        recent,
    );
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
    doc.upsert_section(
        "api_ref.errors",
        SectionType::Text,
        "Error codes: 400 (validation), 401 (unauth), 403 (forbidden), 404 (not found), \
         429 (rate limited), 500 (internal), 503 (service unavailable)"
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "api_ref.versioning",
        SectionType::Text,
        "API versioning via URL prefix: /api/v1/... Breaking changes require new version. \
         Deprecation notices sent 90 days before removal."
            .to_string(),
        recent,
    );

    // --- Config reference (5 sections) ---
    doc.upsert_section(
        "config",
        SectionType::Text,
        "Configuration reference.".to_string(),
        recent,
    );
    doc.upsert_section(
        "config.env",
        SectionType::Text,
        "Environment variables: DATABASE_URL, REDIS_URL, JWT_SECRET, AWS_REGION, \
         LOG_LEVEL, MAX_CONNECTIONS, RATE_LIMIT_WINDOW"
            .to_string(),
        recent,
    );
    doc.upsert_section("config.database", SectionType::Json,
        r#"{"max_connections": 50, "min_connections": 5, "connect_timeout_ms": 5000, "idle_timeout_ms": 300000}"#.to_string(), recent);
    doc.upsert_section(
        "config.redis",
        SectionType::Json,
        r#"{"pool_size": 10, "key_prefix": "rbmem:", "ttl_seconds": 3600}"#.to_string(),
        recent,
    );
    doc.upsert_section(
        "config.logging",
        SectionType::Text,
        "Structured logging via tracing crate. Levels: error, warn, info, debug, trace. \
         JSON format in production, pretty format in development."
            .to_string(),
        recent,
    );

    // --- Deployment (4 sections) ---
    doc.upsert_section(
        "deployment",
        SectionType::Text,
        "Deployment and operations guide.".to_string(),
        recent,
    );
    doc.upsert_section(
        "deployment.ci_cd",
        SectionType::Text,
        "GitHub Actions workflow: lint -> test -> build -> deploy to staging -> \
         manual approval -> deploy to production. Rollback via kubectl rollout undo."
            .to_string(),
        recent,
    );
    doc.upsert_section(
        "deployment.monitoring",
        SectionType::Text,
        "Prometheus scrapes metrics from /metrics endpoint. Grafana dashboards for: \
         request latency, error rates, DB query times, Redis hit rates."
            .to_string(),
        stale,
    );
    doc.upsert_section(
        "deployment.alerts",
        SectionType::Text,
        "PagerDuty alerts for: p99 > 500ms, error rate > 1%, DB connection pool > 80%, \
         disk usage > 90%. On-call rotation: weekly."
            .to_string(),
        stale,
    );

    // --- Graph relations ---
    let relations_auth = vec![
        GraphRelation {
            to: "architecture.api".into(),
            relation_type: "depends_on".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "rules.security".into(),
            relation_type: "requires".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
    ];
    let relations_api = vec![
        GraphRelation {
            to: "architecture.database".into(),
            relation_type: "depends_on".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "config.database".into(),
            relation_type: "uses".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "architecture.frontend".into(),
            relation_type: "collaborates_with".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
    ];
    let relations_testing = vec![
        GraphRelation {
            to: "rules.testing.unit".into(),
            relation_type: "depends_on".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "rules.testing.integration".into(),
            relation_type: "depends_on".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
    ];
    let relations_incidents = vec![
        GraphRelation {
            to: "memory.bugs".into(),
            relation_type: "references".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "architecture.api".into(),
            relation_type: "affects".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
    ];
    let relations_ci = vec![
        GraphRelation {
            to: "deployment.monitoring".into(),
            relation_type: "depends_on".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
        GraphRelation {
            to: "config.logging".into(),
            relation_type: "uses".into(),
            valid_from: Some(recent),
            valid_until: None,
            inferred: false,
            confidence: None,
        },
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
        section.graph = Some(GraphInfo {
            node_type: None,
            relations,
        });
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
            expected_sections: &[
                "architecture.api.rate_limit",
                "api_ref.errors",
                "tasks.backlog",
            ],
            description: "Rate limit feature retrieval",
        },
        QueryTestCase {
            query: "database connection pool",
            expected_sections: &[
                "architecture.database",
                "config.database",
                "memory.bugs",
                "architecture.database.sharding",
            ],
            description: "Database infrastructure retrieval",
        },
        QueryTestCase {
            query: "testing integration tests",
            expected_sections: &[
                "rules.testing",
                "rules.testing.integration",
                "rules.testing.unit",
            ],
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
            expected_sections: &[
                "deployment.monitoring",
                "deployment.alerts",
                "memory.performance",
                "rules.performance",
            ],
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
            expected_sections: &[
                "api_ref.errors",
                "architecture.api.auth",
                "memory.incidents",
            ],
            description: "Error handling retrieval",
        },
        QueryTestCase {
            query: "PostgreSQL migration sharding",
            expected_sections: &[
                "architecture.database",
                "architecture.database.sharding",
                "tasks.backlog",
                "memory.decisions",
            ],
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

    println!(
        "Knowledge Base: {} sections, {} chars RBMEM, {} chars Markdown\n",
        doc.sections.len(),
        doc.to_rbmem_string().len(),
        md.len()
    );

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

    println!(
        " {:<40} {:>10} {:>10} {:>10}",
        "Query", "RBMEM", "MD-full", "MD-kw"
    );
    println!(" {}", "─".repeat(74));

    for case in &cases {
        // RBMEM query
        let rbmem_result = api::query_document(&doc, case.query, true, 1);
        let rbmem_paths: BTreeSet<&str> = rbmem_result
            .sections
            .iter()
            .map(|s| s.path.as_str())
            .collect();

        let rbmem_relevant = case
            .expected_sections
            .iter()
            .filter(|p| rbmem_paths.contains(**p))
            .count();
        let rbmem_precision = if rbmem_paths.is_empty() {
            0.0
        } else {
            rbmem_relevant as f64 / rbmem_paths.len() as f64
        };
        let rbmem_recall = if case.expected_sections.is_empty() {
            0.0
        } else {
            rbmem_relevant as f64 / case.expected_sections.len() as f64
        };

        // Markdown full dump
        let md_full_total_sections = doc.sections.len();
        let md_full_relevant = case.expected_sections.len();
        let md_full_precision = md_full_relevant as f64 / md_full_total_sections as f64;

        // Markdown keyword search
        let (md_kw_text, _md_kw_len) = markdown_keyword_context(&md, case.query);
        let md_kw_relevant = case
            .expected_sections
            .iter()
            .filter(|p| {
                let title = p.replace('.', " > ");
                md_kw_text.contains(&title)
            })
            .count();
        // Count how many "paragraphs" (rough section proxy) were returned
        let md_kw_paragraphs = md_kw_text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .count()
            .max(1);
        let md_kw_precision = md_kw_relevant as f64 / md_kw_paragraphs as f64;
        let md_kw_recall = if case.expected_sections.is_empty() {
            0.0
        } else {
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
        println!(
            " {:<40} {:>9.1}% {:>9.1}% {:>9.1}%",
            label,
            rbmem_precision * 100.0,
            md_full_precision * 100.0,
            md_kw_precision * 100.0
        );
    }

    let n = cases.len() as f64;
    let rbmem_avg_precision = rbmem_precision_sum / n;
    let md_full_avg_precision = md_full_precision_sum / n;
    let md_kw_avg_precision = md_keyword_precision_sum / n;
    let rbmem_avg_recall = rbmem_recall_sum / n;
    let md_kw_avg_recall = md_keyword_recall_sum / n;

    let rbmem_f1 = 2.0 * rbmem_avg_precision * rbmem_avg_recall
        / (rbmem_avg_precision + rbmem_avg_recall).max(0.001);
    let md_kw_f1 = 2.0 * md_kw_avg_precision * md_kw_avg_recall
        / (md_kw_avg_precision + md_kw_avg_recall).max(0.001);

    println!(" {}", "─".repeat(74));
    println!(
        " {:<40} {:>9.1}% {:>9.1}% {:>9.1}%",
        "AVERAGE PRECISION",
        rbmem_avg_precision * 100.0,
        md_full_avg_precision * 100.0,
        md_kw_avg_precision * 100.0
    );
    println!(
        " {:<40} {:>9.1}% {:>10} {:>9.1}%",
        "AVERAGE RECALL",
        rbmem_avg_recall * 100.0,
        "—",
        md_kw_avg_recall * 100.0
    );
    println!(
        " {:<40} {:>9.3}  {:>10} {:>9.3}",
        "F1 SCORE", rbmem_f1, "—", md_kw_f1
    );

    let precision_lift = (rbmem_avg_precision / md_full_avg_precision - 1.0) * 100.0;
    let precision_lift_kw = (rbmem_avg_precision / md_kw_avg_precision.max(0.001) - 1.0) * 100.0;
    println!(
        "\n  → RBMEM precision is {:.1}x higher than MD-full (+{:.0}%)",
        rbmem_avg_precision / md_full_avg_precision,
        precision_lift
    );
    println!(
        "  → RBMEM precision is {:.1}x higher than MD-keyword (+{:.0}%)",
        rbmem_avg_precision / md_kw_avg_precision.max(0.001),
        precision_lift_kw
    );

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
    println!(
        " {:<35} {:>10} {:>10}",
        "Markdown (full dump)",
        format_num(md_full_len),
        "1.00x"
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM canonical",
        format_num(rbmem_full.len()),
        format!("{:.2}x", rbmem_full.len() as f64 / md_full_len as f64)
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM compact",
        format_num(rbmem_compact.len()),
        format!("{:.2}x", rbmem_compact.len() as f64 / md_full_len as f64)
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM minified",
        format_num(rbmem_minified.len()),
        format!("{:.2}x", rbmem_minified.len() as f64 / md_full_len as f64)
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM minified+resolved",
        format_num(rbmem_resolved.len()),
        format!("{:.2}x", rbmem_resolved.len() as f64 / md_full_len as f64)
    );
    println!(" {}", "─".repeat(58));
    println!(" Per-query context (avg over {} queries):", cases.len());
    println!(
        " {:<35} {:>10} {:>10}",
        "Markdown (full dump)",
        format_num(md_full_total / cases.len()),
        format!("{:.2}x", 1.0)
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "Markdown (keyword search)",
        format_num(md_kw_total / cases.len()),
        format!("{:.2}x", md_kw_total as f64 / md_full_total as f64)
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM query (minified+resolved)",
        format_num(rbmem_query_total / cases.len()),
        format!("{:.2}x", rbmem_query_total as f64 / md_full_total as f64)
    );

    let token_savings = 1.0 - (rbmem_query_total as f64 / md_full_total as f64);
    println!(
        "\n  → RBMEM query context uses {:.1}% fewer tokens than MD full dump",
        token_savings * 100.0
    );
    let kw_savings = 1.0 - (rbmem_query_total as f64 / md_kw_total.max(1) as f64);
    if kw_savings > 0.0 {
        println!(
            "  → RBMEM query context uses {:.1}% fewer tokens than MD keyword search",
            kw_savings * 100.0
        );
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
        (
            "authentication JWT",
            "architecture.api.auth",
            vec!["architecture.api", "rules.security"],
        ),
        (
            "API gateway Axum",
            "architecture.api",
            vec![
                "architecture.database",
                "config.database",
                "architecture.frontend",
            ],
        ),
        (
            "testing integration",
            "rules.testing",
            vec!["rules.testing.unit", "rules.testing.integration"],
        ),
        (
            "incidents bugs",
            "memory.incidents",
            vec!["memory.bugs", "architecture.api"],
        ),
        (
            "deployment CI/CD",
            "deployment.ci_cd",
            vec!["deployment.monitoring", "config.logging"],
        ),
    ];

    println!(" {:<30} {:>12} {:>12}", "Query", "w/ graph", "w/o graph");
    println!(" {}", "─".repeat(58));

    let mut with_graph_sum = 0.0f64;
    let mut without_graph_sum = 0.0f64;

    for (query, _primary, expected_neighbors) in &graph_queries {
        let with_graph = api::query_document(&doc, query, true, 1);
        let without_graph = api::query_document(&doc, query, true, 0);

        let with_paths: BTreeSet<&str> = with_graph
            .sections
            .iter()
            .map(|s| s.path.as_str())
            .collect();
        let without_paths: BTreeSet<&str> = without_graph
            .sections
            .iter()
            .map(|s| s.path.as_str())
            .collect();

        let with_neighbors = expected_neighbors
            .iter()
            .filter(|p| with_paths.contains(**p))
            .count();
        let without_neighbors = expected_neighbors
            .iter()
            .filter(|p| without_paths.contains(**p))
            .count();

        let with_recall = with_neighbors as f64 / expected_neighbors.len() as f64;
        let without_recall = without_neighbors as f64 / expected_neighbors.len() as f64;
        with_graph_sum += with_recall;
        without_graph_sum += without_recall;

        let label = if query.len() > 28 {
            &query[..28]
        } else {
            *query
        };
        println!(
            " {:<30} {:>11.0}% {:>11.0}%",
            label,
            with_recall * 100.0,
            without_recall * 100.0
        );
    }

    let gn = graph_queries.len() as f64;
    let with_graph_avg = with_graph_sum / gn;
    let without_graph_avg = without_graph_sum / gn;

    println!(" {}", "─".repeat(58));
    println!(
        " {:<30} {:>11.0}% {:>11.0}%",
        "AVERAGE",
        with_graph_avg * 100.0,
        without_graph_avg * 100.0
    );
    println!(
        "\n  → Graph-aware retrieval finds {:.1}x more related sections",
        with_graph_avg / without_graph_avg.max(0.001)
    );
    println!("  → Markdown has NO equivalent of graph relations");

    // ============================================================
    // TEST 4: Temporal Awareness
    // ============================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 4: Temporal Awareness");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let stale_sections: Vec<&Section> = doc
        .sections
        .iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() > 90)
        .collect();
    let recent_sections: Vec<&Section> = doc
        .sections
        .iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() <= 7)
        .collect();

    println!(" {:<35} {:>10}", "Metric", "Count");
    println!(" {}", "─".repeat(48));
    println!(" {:<35} {:>10}", "Total sections", doc.sections.len());
    println!(
        " {:<35} {:>10}",
        "Stale sections (>90 days)",
        stale_sections.len()
    );
    println!(
        " {:<35} {:>10}",
        "Recent sections (<=7 days)",
        recent_sections.len()
    );
    println!(
        " {:<35} {:>10}",
        "Sections with graph relations",
        doc.sections.iter().filter(|s| s.graph.is_some()).count()
    );
    println!(
        " {:<35} {:>10}",
        "Total graph edges",
        doc.sections
            .iter()
            .filter_map(|s| s.graph.as_ref())
            .map(|g| g.relations.len())
            .sum::<usize>()
    );

    // Query that should prefer recent content
    let temporal_query = api::query_document(&doc, "connection pool database bugs", true, 0);
    let temporal_recent = temporal_query
        .sections
        .iter()
        .filter(|s| (Utc::now() - s.temporal.updated_at).num_days() <= 7)
        .count();
    let temporal_total = temporal_query.sections.len();

    println!("\n  Temporal query: \"connection pool database bugs\"");
    println!(
        "  → Returned {} sections ({} recent, {} stale)",
        temporal_total,
        temporal_recent,
        temporal_total - temporal_recent
    );
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
    let index = rbmem::SectionIndex::build(&doc);
    let start = Instant::now();
    for _ in 0..iterations {
        for case in &cases {
            let _ = api::query_document_with_index(&doc, case.query, true, 1, &index);
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
    println!(
        " {:<35} {:>10} {:>10}",
        "RBMEM (indexed query + graph)",
        format!("{}µs", rbmem_per_query),
        "1.00x"
    );
    println!(
        " {:<35} {:>10} {:>10}",
        "Markdown (keyword grep)",
        format!("{}µs", md_per_query),
        format!(
            "{:.2}x",
            md_per_query as f64 / rbmem_per_query.max(1) as f64
        )
    );
    println!(
        "\n  (Over {} queries × {} iterations = {} total calls)",
        cases.len(),
        iterations,
        iterations * cases.len()
    );

    // ============================================================
    // SUMMARY
    // ============================================================
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                           SUMMARY                                    ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║ Dimension              │ RBMEM           │ Markdown                  ║");
    println!("╠════════════════════════╪═════════════════╪═══════════════════════════╣");
    println!(
        "║ Avg Precision          │ {:>10.1}%    │ {:>10.1}% (full dump)     ║",
        rbmem_avg_precision * 100.0,
        md_full_avg_precision * 100.0
    );
    println!(
        "║ Avg Recall             │ {:>10.1}%    │ {:>10}                ║",
        rbmem_avg_recall * 100.0,
        "—"
    );
    println!(
        "║ F1 Score               │ {:>10.3}     │ {:>10}                ║",
        rbmem_f1, "—"
    );
    println!(
        "║ Token savings (query)  │ {:>10.1}%    │ baseline                   ║",
        token_savings * 100.0
    );
    println!(
        "║ Graph relations        │ {:>10}     │ {:>10}                ║",
        "supported", "none"
    );
    println!(
        "║ Temporal awareness     │ {:>10}     │ {:>10}                ║",
        "per-section", "none"
    );
    println!(
        "║ Compact modes          │ {:>10}     │ {:>10}                ║",
        "3 modes", "none"
    );
    println!(
        "║ Encryption             │ {:>10}     │ {:>10}                ║",
        "per-section", "none"
    );
    println!(
        "║ Provenance tracking    │ {:>10}     │ {:>10}                ║",
        "per-section", "none"
    );
    println!("╚════════════════════════╧═════════════════╧═══════════════════════════╝");

    // New performance profiling tests
    bench_graph_traversal(&doc);
    bench_planner(&doc);
    bench_parsing(&doc);
}

// ============================================================
// TEST 6: Graph Traversal Performance
// ============================================================
fn bench_graph_traversal(doc: &RbmemDocument) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 6: Graph Traversal Performance");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let iterations = 500;

    // 6a: Index build time
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = rbmem::SectionIndex::build(doc);
    }
    let build_elapsed = start.elapsed();
    let build_per_call = build_elapsed.as_micros() / iterations as u128;

    println!(" {:<40} {:>12}", "Metric", "Time");
    println!(" {}", "─".repeat(55));
    println!(" {:<40} {:>10}µs", "SectionIndex::build()", build_per_call);

    let index = rbmem::SectionIndex::build(doc);

    // 6b: BFS traversal at varying depths
    let traversal_paths = [
        "architecture.api.auth",
        "architecture.api",
        "rules.testing",
        "memory.incidents",
        "deployment.ci_cd",
    ];

    for depth in [1, 2, 3, 5] {
        let start = Instant::now();
        for _ in 0..iterations {
            for path in &traversal_paths {
                let _ = index.related(path, depth);
            }
        }
        let elapsed = start.elapsed();
        let per_call = elapsed.as_micros() / (iterations * traversal_paths.len()) as u128;
        println!(
            " {:<40} {:>10}µs",
            format!("related(depth={})", depth),
            per_call
        );
    }

    // 6c: graph_view() construction
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = doc.graph_view();
    }
    let gv_elapsed = start.elapsed();
    let gv_per_call = gv_elapsed.as_micros() / iterations as u128;
    println!(" {:<40} {:>10}µs", "graph_view()", gv_per_call);

    // 6d: petgraph construction
    #[cfg(feature = "graph")]
    {
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = doc.petgraph();
        }
        let pg_elapsed = start.elapsed();
        let pg_per_call = pg_elapsed.as_micros() / iterations as u128;
        println!(" {:<40} {:>10}µs", "petgraph()", pg_per_call);
    }

    // 6e: resolved_sections (hierarchy traversal)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = doc.resolved_sections();
    }
    let rs_elapsed = start.elapsed();
    let rs_per_call = rs_elapsed.as_micros() / iterations as u128;
    println!(" {:<40} {:>10}µs", "resolved_sections()", rs_per_call);

    // 6f: Serialization
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = doc.to_rbmem_string();
    }
    let ser_elapsed = start.elapsed();
    let ser_per_call = ser_elapsed.as_micros() / iterations as u128;
    println!(" {:<40} {:>10}µs", "to_rbmem_string()", ser_per_call);

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = doc.to_minified_string(true);
    }
    let min_elapsed = start.elapsed();
    let min_per_call = min_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<40} {:>10}µs",
        "to_minified_string(resolve=true)", min_per_call
    );

    // 6g: Scale test — build large documents and measure traversal
    println!("\n  Scale test: graph traversal with growing document size");
    println!(
        " {:<20} {:>10} {:>10} {:>10}",
        "Sections", "Build µs", "BFS-d2 µs", "Query µs"
    );
    println!(" {}", "─".repeat(55));

    for scale in [50, 100, 200, 500] {
        let large_doc = build_scaled_document(scale);
        let start = Instant::now();
        for _ in 0..100 {
            let _ = rbmem::SectionIndex::build(&large_doc);
        }
        let build_us = start.elapsed().as_micros() / 100;

        let large_index = rbmem::SectionIndex::build(&large_doc);
        let start = Instant::now();
        for _ in 0..100 {
            let _ = large_index.related("module_0.sub_0.leaf", 2);
        }
        let bfs_us = start.elapsed().as_micros() / 100;

        let start = Instant::now();
        for _ in 0..100 {
            let _ = api::query_document_with_index(
                &large_doc,
                "module alpha data processing",
                true,
                1,
                &large_index,
            );
        }
        let query_us = start.elapsed().as_micros() / 100;

        println!(
            " {:<20} {:>10} {:>10} {:>10}",
            scale, build_us, bfs_us, query_us
        );
    }
}

/// Build a scaled document with N sections, organized in a hierarchy with graph relations.
fn build_scaled_document(section_count: usize) -> RbmemDocument {
    let base = Utc::now() - Duration::days(30);
    let mut doc = RbmemDocument::new(base, "benchmark-scale");
    doc.meta.purpose = format!("scale test with {} sections", section_count);

    let modules = (section_count / 10).max(1);
    let subs_per_module = 3;
    let mut count = 0;

    for m in 0..modules {
        let mod_path = format!("module_{}", m);
        doc.upsert_section(
            &mod_path,
            SectionType::Text,
            format!(
                "Module {} handles data processing and transformation for subsystem alpha.",
                m
            ),
            base,
        );

        for s in 0..subs_per_module {
            let sub_path = format!("{}.sub_{}", mod_path, s);
            doc.upsert_section(&sub_path, SectionType::Text,
                format!("Subsystem {} of module {}. Implements alpha-beta filtering and data validation.", s, m), base);

            // Add leaf sections
            let remaining = section_count - count;
            if remaining > 0 {
                let leaf_path = format!("{}.leaf", sub_path);
                doc.upsert_section(&leaf_path, SectionType::Text,
                    format!("Leaf configuration for subsystem {} in module {}. Contains alpha parameters and processing rules.", s, m), base);
                count += 1;
            }
            count += 1;
        }
        count += 1;

        // Add extra flat sections to reach target count
        while count < section_count && count < (m + 1) * (1 + subs_per_module * 2) {
            let flat_path = format!("{}.extra_{}", mod_path, count);
            doc.upsert_section(&flat_path, SectionType::List,
                format!("- Rule alpha for item {}\n- Constraint beta on processing\n- Task: validate data", count), base);
            count += 1;
        }
    }

    // Fill remaining with flat sections
    while count < section_count {
        let path = format!("extra.section_{}", count);
        doc.upsert_section(
            &path,
            SectionType::Text,
            format!(
                "Additional section {} with alpha data processing rules and module references.",
                count
            ),
            base,
        );
        count += 1;
    }

    // Add graph relations between modules
    for m in 0..modules.min(10) {
        let from_path = format!("module_{}.sub_0", m);
        let to_path = format!("module_{}.sub_1", (m + 1) % modules);
        if let Some(section) = doc.sections.iter_mut().find(|s| s.path == from_path) {
            section.graph = Some(GraphInfo {
                node_type: None,
                relations: vec![GraphRelation {
                    to: to_path,
                    relation_type: "depends_on".into(),
                    valid_from: Some(base),
                    valid_until: None,
                    inferred: false,
                    confidence: None,
                }],
            });
        }
    }

    doc
}

// ============================================================
// TEST 7: Planner Execution Time
// ============================================================
fn bench_planner(doc: &RbmemDocument) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 7: Planner Execution Time");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let iterations = 50;

    println!(" {:<45} {:>10}", "Scenario", "µs/call");
    println!(" {}", "─".repeat(58));

    // Build a planning document from the existing knowledge base
    let plan_doc = build_planning_document(doc);
    let plan_rbmem = plan_doc.to_rbmem_string();

    // Measure parse of plan doc
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = parse_document(&plan_rbmem, TimestampPolicy::Preserve).unwrap();
    }
    let parse_elapsed = start.elapsed();
    let parse_per_call = parse_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<45} {:>10}µs",
        "parse_document (plan doc)", parse_per_call
    );

    // Use temp file for plan_memory
    let temp_dir = std::env::temp_dir().join("rbmem_bench_planner");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_file = temp_dir.join("bench_plan.rbmem");
    std::fs::write(&temp_file, &plan_rbmem).unwrap();

    // Small problem
    let start = Instant::now();
    for _ in 0..iterations {
        let options = rbmem::PlanOptions {
            goal: Some("Deploy authentication service".to_string()),
            file: Some(temp_file.clone()),
            search_dir: temp_dir.clone(),
            solver: rbmem::SatBackend::Internal,
            from_memory: false,
            dry_run: true,
            cube_and_conquer: false,
            context_pack: None,
            now: Utc::now(),
            proof: false,
            proof_path: None,
            verify_proof: false,
        };
        let _ = rbmem::plan_memory(options);
    }
    let small_elapsed = start.elapsed();
    let small_per_call = small_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<45} {:>10}µs",
        "plan_memory (small, ~20 candidates)", small_per_call
    );

    // Large constraint set
    let large_plan_doc = build_large_planning_problem(200);
    let large_plan_rbmem = large_plan_doc.to_rbmem_string();
    let large_temp_file = temp_dir.join("bench_large_plan.rbmem");
    std::fs::write(&large_temp_file, &large_plan_rbmem).unwrap();

    let start = Instant::now();
    for _ in 0..10 {
        let options = rbmem::PlanOptions {
            goal: Some("Complete system migration with zero downtime".to_string()),
            file: Some(large_temp_file.clone()),
            search_dir: temp_dir.clone(),
            solver: rbmem::SatBackend::Internal,
            from_memory: false,
            dry_run: true,
            cube_and_conquer: false,
            context_pack: None,
            now: Utc::now(),
            proof: false,
            proof_path: None,
            verify_proof: false,
        };
        let _ = rbmem::plan_memory(options);
    }
    let large_elapsed = start.elapsed();
    let large_per_call = large_elapsed.as_micros() / 10_u128;
    println!(
        " {:<45} {:>10}µs",
        "plan_memory (large, ~200 candidates)", large_per_call
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn build_planning_document(base_doc: &RbmemDocument) -> RbmemDocument {
    let base = Utc::now() - Duration::days(30);
    let mut doc = RbmemDocument::new(base, "benchmark-planner");
    doc.meta.purpose = "planner benchmark".to_string();

    // Copy some sections from base doc
    for section in &base_doc.sections {
        if section.path.starts_with("rules") || section.path.starts_with("tasks") {
            doc.sections.push(section.clone());
        }
    }

    // Add goal and action sections that the planner looks for
    doc.upsert_section("goals", SectionType::List,
        "- Deploy authentication service\n- Set up monitoring pipeline\n- Migrate database to new cluster".to_string(), base);
    doc.upsert_section("actions", SectionType::List,
        "- Deploy auth service to staging\n- Run integration tests\n- Configure Prometheus alerts\n- \
         Set up Grafana dashboards\n- Migrate user data\n- Update API gateway routes\n- \
         Enable rate limiting\n- Set up health checks\n- Configure log aggregation\n- \
         Deploy to production\n- Run smoke tests\n- Enable feature flags\n- \
         Set up backup schedule\n- Configure auto-scaling\n- Update DNS records\n- \
         Enable TLS certificates\n- Set up CI/CD pipeline\n- Configure secrets manager\n- \
         Deploy monitoring agent\n- Set up alerting rules".to_string(), base);
    doc.upsert_section(
        "constraints",
        SectionType::List,
        "- Deploy auth service requires Run integration tests\n- \
         Deploy to production requires Run smoke tests\n- \
         Migrate user data requires Deploy auth service\n- \
         Enable rate limiting conflicts with Enable feature flags\n- \
         Update DNS records requires Deploy to production\n- \
         Enable TLS certificates must come before Update API gateway routes\n- \
         must Run integration tests\n- avoid Deploy to production without Run smoke tests\n- \
         Set up monitoring pipeline requires Configure Prometheus alerts\n- \
         Set up Grafana dashboards depends on Configure Prometheus alerts\n- \
         Configure auto-scaling requires Deploy to production\n- \
         Set up backup schedule must come after Migrate user data"
            .to_string(),
        base,
    );

    doc
}

fn build_large_planning_problem(candidate_count: usize) -> RbmemDocument {
    let base = Utc::now() - Duration::days(30);
    let mut doc = RbmemDocument::new(base, "benchmark-large-planner");
    doc.meta.purpose = "large planner benchmark".to_string();

    let mut actions = Vec::new();
    let modules = [
        "auth",
        "api",
        "database",
        "cache",
        "frontend",
        "monitoring",
        "deploy",
        "test",
    ];
    let verbs = [
        "Deploy",
        "Configure",
        "Update",
        "Migrate",
        "Enable",
        "Set up",
        "Optimize",
        "Refactor",
    ];

    for i in 0..candidate_count {
        let module = modules[i % modules.len()];
        let verb = verbs[i % verbs.len()];
        actions.push(format!("- {} {} component {}", verb, module, i));
    }
    doc.upsert_section("actions", SectionType::List, actions.join("\n"), base);

    let mut constraints = Vec::new();
    for i in 0..(candidate_count / 4) {
        let a_module = modules[i % modules.len()];
        let b_module = modules[(i + 1) % modules.len()];
        let a_verb = verbs[i % verbs.len()];
        let b_verb = verbs[(i + 1) % verbs.len()];
        constraints.push(format!(
            "- {} {} component {} requires {} {} component {}",
            a_verb,
            a_module,
            i,
            b_verb,
            b_module,
            (i + 1) % candidate_count
        ));
    }
    for i in 0..(candidate_count / 10) {
        let a_module = modules[i % modules.len()];
        let b_module = modules[(i + 3) % modules.len()];
        constraints.push(format!(
            "- Deploy {} component {} conflicts with Update {} component {}",
            a_module,
            i,
            b_module,
            (i + 5) % candidate_count
        ));
    }
    doc.upsert_section(
        "constraints",
        SectionType::List,
        constraints.join("\n"),
        base,
    );

    doc.upsert_section(
        "goals",
        SectionType::Text,
        "Complete system migration with zero downtime".to_string(),
        base,
    );

    doc
}

// ============================================================
// TEST 8: Document Parsing/Loading Time
// ============================================================
fn bench_parsing(doc: &RbmemDocument) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" TEST 8: Document Parsing & Loading Time");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let iterations = 500;

    // 8a: Parse small document (existing KB)
    let rbmem_text = doc.to_rbmem_string();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = parse_document(&rbmem_text, TimestampPolicy::Preserve).unwrap();
    }
    let small_elapsed = start.elapsed();
    let small_per_call = small_elapsed.as_micros() / iterations as u128;

    println!(" {:<45} {:>10}", "Scenario", "µs/call");
    println!(" {}", "─".repeat(58));
    println!(
        " {:<45} {:>10}µs",
        format!(
            "parse ({} sections, {} chars)",
            doc.sections.len(),
            rbmem_text.len()
        ),
        small_per_call
    );

    // 8b: Parse minified format
    let minified_text = doc.to_minified_string(false);
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = parse_document(&minified_text, TimestampPolicy::Preserve).unwrap();
    }
    let min_elapsed = start.elapsed();
    let min_per_call = min_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<45} {:>10}µs",
        format!("parse minified ({} chars)", minified_text.len()),
        min_per_call
    );

    // 8c: Scale test
    println!("\n  Scale test: parsing time vs document size");
    println!(
        " {:<20} {:>10} {:>10} {:>10}",
        "Sections", "Chars", "Parse µs", "µs/section"
    );
    println!(" {}", "─".repeat(55));

    for scale in [50, 100, 200, 500] {
        let large_doc = build_scaled_document(scale);
        let large_text = large_doc.to_rbmem_string();
        let iters = (500 / (scale / 50).max(1)).max(10);
        let start = Instant::now();
        for _ in 0..iters {
            let _ = parse_document(&large_text, TimestampPolicy::Preserve).unwrap();
        }
        let elapsed = start.elapsed();
        let per_call = elapsed.as_micros() / iters as u128;
        let per_section = per_call / scale as u128;
        println!(
            " {:<20} {:>10} {:>10} {:>10}",
            scale,
            large_text.len(),
            per_call,
            per_section
        );
    }

    // 8d: Round-trip
    let start = Instant::now();
    for _ in 0..iterations {
        let serialized = doc.to_rbmem_string();
        let _ = parse_document(&serialized, TimestampPolicy::Preserve).unwrap();
    }
    let rt_elapsed = start.elapsed();
    let rt_per_call = rt_elapsed.as_micros() / iterations as u128;
    println!(
        "\n {:<45} {:>10}µs",
        "round-trip (serialize + parse)", rt_per_call
    );

    // 8e: Index build from parsed document
    let parsed = parse_document(&rbmem_text, TimestampPolicy::Preserve).unwrap();
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = rbmem::SectionIndex::build(&parsed.document);
    }
    let idx_elapsed = start.elapsed();
    let idx_per_call = idx_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<45} {:>10}µs",
        "SectionIndex::build() from parsed doc", idx_per_call
    );

    // 8f: Full pipeline
    let start = Instant::now();
    for _ in 0..iterations {
        let parsed = parse_document(&rbmem_text, TimestampPolicy::Preserve).unwrap();
        let index = rbmem::SectionIndex::build(&parsed.document);
        let _ = api::query_document_with_index(
            &parsed.document,
            "authentication JWT tokens",
            true,
            1,
            &index,
        );
    }
    let full_elapsed = start.elapsed();
    let full_per_call = full_elapsed.as_micros() / iterations as u128;
    println!(
        " {:<45} {:>10}µs",
        "full pipeline (parse+index+query)", full_per_call
    );
}

fn format_num(n: usize) -> String {
    if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        format!("{}", n)
    }
}
