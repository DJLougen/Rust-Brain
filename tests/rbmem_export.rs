use chrono::{TimeZone, Utc};
use rbmem::{export_graph, ExportFormat, RbmemDocument, SectionType};

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 7, 14, 0, 0).unwrap()
}

#[test]
fn exports_graph_visualization_formats() {
    let now = fixed_time();
    let mut document = RbmemDocument::new(now, "me");
    document.upsert_section(
        "agents.reader",
        SectionType::Text,
        "Reader.".to_string(),
        now,
    );
    document.upsert_section(
        "agents.writer",
        SectionType::Text,
        "Writer.".to_string(),
        now,
    );

    let mermaid = export_graph(&document, ExportFormat::Mermaid).unwrap();
    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.contains("agents.reader"));

    let cytoscape = export_graph(&document, ExportFormat::Cytoscape).unwrap();
    assert!(cytoscape.contains("\"nodes\""));
    assert!(cytoscape.contains("\"edges\""));

    let gexf = export_graph(&document, ExportFormat::Gexf).unwrap();
    assert!(gexf.contains("<gexf"));
    assert!(gexf.contains("<edges>"));
}
