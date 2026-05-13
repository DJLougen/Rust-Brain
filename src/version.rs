pub const RBMEM_FORMAT_VERSION: &str = "1.4.0";
pub const RBMEM_LEGACY_FORMAT_VERSION: &str = "1.3";
pub const RBMEM_FORMAT_LABEL: &str = "RBMEM v1.4.0";

pub fn is_supported_format_version(version: &str) -> bool {
    matches!(
        version.trim(),
        RBMEM_FORMAT_VERSION | "1.4" | RBMEM_LEGACY_FORMAT_VERSION | "1.3.0"
    )
}
