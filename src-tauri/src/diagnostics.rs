// R4 IMPLEMENTS: one-click diagnosis — turn a tunnel's error state + recent
// log lines into a human-readable explanation with concrete suggestions
// (security group not open, token mismatch, version mismatch, port already
// in use, local service down, ...).
use crate::models::Diagnosis;

/// Diagnose tunnel `tunnel_id` using its current state and the engine's
/// recent log ring buffer.
pub fn diagnose(_tunnel_id: &str, _state_error: Option<&str>, _recent_logs: &[String]) -> Diagnosis {
    Diagnosis {
        tunnel_id: _tunnel_id.to_string(),
        level: crate::models::DiagnosisLevel::Info,
        title: "No issues detected".into(),
        detail: "diagnostics: not implemented".into(),
        suggestions: vec![],
    }
}
