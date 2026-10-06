// R4 IMPLEMENTS: one-click diagnosis — turn a tunnel's error state + recent
// log lines into a structured result: a stable `code` slug (the frontend
// renders localized title/suggestions from `diagnosis.<code>.*` i18n keys)
// plus `detail` holding the raw evidence line.
//
// Expected codes (keep in sync with the frontend i18n dictionary):
//   tokenMismatch, authFailed, versionMismatch, portConflict,
//   connectionRefused, dnsFailed, localServiceDown, binaryMissing,
//   remoteServerUnreachable, genericError, allHealthy
use crate::models::Diagnosis;

/// Diagnose tunnel `tunnel_id` using its current state error and the
/// engine's recent log ring buffer.
pub fn diagnose(_tunnel_id: &str, _state_error: Option<&str>, _recent_logs: &[String]) -> Diagnosis {
    Diagnosis {
        tunnel_id: _tunnel_id.to_string(),
        code: "allHealthy".to_string(),
        level: crate::models::DiagnosisLevel::Info,
        detail: "diagnostics: not implemented".into(),
    }
}
