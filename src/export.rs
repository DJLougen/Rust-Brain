use crate::{RbmemDocument, RbmemError};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Dot,
    Mermaid,
    Cytoscape,
    Gexf,
}

pub fn export_graph(document: &RbmemDocument, format: ExportFormat) -> Result<String, RbmemError> {
    match format {
        ExportFormat::Dot => Ok(document.graph_as_dot()),
        ExportFormat::Mermaid => Ok(export_mermaid(document)),
        ExportFormat::Cytoscape => Ok(serde_json::to_string_pretty(&export_cytoscape(document))?),
        ExportFormat::Gexf => Ok(export_gexf(document)),
    }
}

fn export_mermaid(document: &RbmemDocument) -> String {
    let view = document.graph_view();
    let mut output = String::from("graph TD\n");
    for node in &view.nodes {
        output.push_str(&format!("  {}\n", mermaid_node(node)));
    }
    for edge in &view.edges {
        output.push_str(&format!(
            "  {} -->|{}| {}\n",
            mermaid_node(&edge.from),
            escape_mermaid(&edge.edge_type),
            mermaid_node(&edge.to)
        ));
    }
    output
}

fn export_cytoscape(document: &RbmemDocument) -> serde_json::Value {
    let view = document.graph_view();
    let nodes = view
        .nodes
        .iter()
        .map(|node| json!({ "data": { "id": node, "label": node } }))
        .collect::<Vec<_>>();
    let edges = view
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            json!({
                "data": {
                    "id": format!("e{index}"),
                    "source": edge.from,
                    "target": edge.to,
                    "label": edge.edge_type,
                    "type": edge.edge_type,
                    "inferred": edge.inferred,
                    "confidence": edge.confidence,
                }
            })
        })
        .collect::<Vec<_>>();

    json!({ "elements": { "nodes": nodes, "edges": edges } })
}

fn export_gexf(document: &RbmemDocument) -> String {
    let view = document.graph_view();
    let mut output = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gexf version="1.3">
  <graph mode="static" defaultedgetype="directed">
    <nodes>
"#,
    );
    for (index, node) in view.nodes.iter().enumerate() {
        output.push_str(&format!(
            "      <node id=\"{}\" label=\"{}\" />\n",
            index,
            escape_xml(node)
        ));
    }
    output.push_str("    </nodes>\n    <edges>\n");
    for (index, edge) in view.edges.iter().enumerate() {
        let source = view
            .nodes
            .iter()
            .position(|node| node == &edge.from)
            .unwrap_or(0);
        let target = view
            .nodes
            .iter()
            .position(|node| node == &edge.to)
            .unwrap_or(0);
        output.push_str(&format!(
            "      <edge id=\"{}\" source=\"{}\" target=\"{}\" label=\"{}\" />\n",
            index,
            source,
            target,
            escape_xml(&edge.edge_type)
        ));
    }
    output.push_str("    </edges>\n  </graph>\n</gexf>\n");
    output
}

fn mermaid_node(value: &str) -> String {
    format!("{}[\"{}\"]", stable_id(value), escape_mermaid(value))
}

fn stable_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn escape_mermaid(value: &str) -> String {
    value.replace('"', "'")
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
