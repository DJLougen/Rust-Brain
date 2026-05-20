use crate::{RbmemDocument, SectionType};
use chrono::Utc;

pub fn convert_markdown_to_rbmem(
    markdown: &str,
    now: chrono::DateTime<Utc>,
) -> RbmemDocument {
    let mut document = RbmemDocument::new(now, "me");
    let mut heading_stack: Vec<String> = Vec::new();
    let mut current_path = "meta.markdown".to_string();
    let mut current_lines = Vec::new();

    for line in markdown.lines() {
        if let Some((level, title)) = markdown_heading(line) {
            flush_markdown_section(&mut document, &current_path, &mut current_lines, now);
            heading_stack.truncate(level.saturating_sub(1));
            heading_stack.push(title_to_path(title));
            current_path = heading_stack.join(".");
        } else {
            current_lines.push(line.to_string());
        }
    }

    flush_markdown_section(&mut document, &current_path, &mut current_lines, now);
    document
}

pub fn title_to_path(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('.');
            last_was_separator = true;
        }
    }

    while slug.ends_with('.') {
        slug.pop();
    }

    if slug.is_empty() {
        "document".to_string()
    } else {
        slug
    }
}

pub fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

fn flush_markdown_section(
    document: &mut RbmemDocument,
    path: &str,
    lines: &mut Vec<String>,
    now: chrono::DateTime<Utc>,
) {
    let content = lines.join("\n").trim().to_string();
    if !content.is_empty() {
        document.upsert_section(path, SectionType::Text, content, now);
    }
    lines.clear();
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let after_hashes = trimmed.get(hashes..)?;
    if !after_hashes.starts_with(' ') {
        return None;
    }

    let title = after_hashes.trim();
    (!title.is_empty()).then_some((hashes, title))
}
