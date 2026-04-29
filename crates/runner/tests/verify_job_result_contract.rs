#![allow(dead_code)]

#[path = "support/e2e_integrity.rs"]
mod e2e_integrity;

use e2e_integrity::{VerifyCorrectnessAudit, VerifyJobResponse};
use serde_json::json;

#[test]
fn verify_correctness_audit_uses_structured_job_results_without_logs() {
    let response = serde_json::from_value::<VerifyJobResponse>(json!({
        "job_id": "job-000001",
        "status": "succeeded",
        "databases": [{
            "name": "app",
            "status": "succeeded",
            "schemas": ["public"],
            "tables": ["accounts"],
            "rows_checked": 7,
            "findings": []
        }]
    }))
    .expect("verify job response should deserialize");

    let audit = VerifyCorrectnessAudit::new(vec!["public.accounts".to_string()], response);

    assert!(audit.selected_tables_match());
}

#[test]
fn verify_correctness_audit_accepts_mismatch_failures_with_results() {
    let response = serde_json::from_value::<VerifyJobResponse>(json!({
        "job_id": "job-000002",
        "status": "failed",
        "databases": [{
            "name": "app",
            "status": "failed",
            "schemas": ["public"],
            "tables": ["accounts"],
            "rows_checked": 7,
            "error": {
                "category": "mismatch",
                "code": "mismatch_detected",
                "message": "verify detected mismatches in 1 table"
            },
            "findings": [{
                "kind": "mismatching_table_definition",
                "schema": "public",
                "table": "accounts",
                "message": "primary key mismatch"
            }]
        }]
    }))
    .expect("verify mismatch job response should deserialize");

    let audit = VerifyCorrectnessAudit::new(vec!["public.accounts".to_string()], response);

    assert!(audit.selected_tables_mismatch());
}
