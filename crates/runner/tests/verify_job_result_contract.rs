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
        "result": {
            "table_summaries": [{
                "schema": "public",
                "table": "accounts",
                "num_verified": 7,
                "num_success": 7,
                "num_missing": 0,
                "num_mismatch": 0,
                "num_column_mismatch": 0,
                "num_extraneous": 0,
                "num_live_retry": 0
            }],
            "mismatch_tables": [],
            "table_definition_mismatches": []
        }
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
        "failure": {
            "category": "mismatch",
            "code": "mismatch_detected",
            "message": "verify detected mismatches in 1 table"
        },
        "result": {
            "table_summaries": [{
                "schema": "public",
                "table": "accounts",
                "num_verified": 7,
                "num_success": 6,
                "num_missing": 0,
                "num_mismatch": 1,
                "num_column_mismatch": 0,
                "num_extraneous": 0,
                "num_live_retry": 0
            }],
            "mismatch_tables": [{
                "schema": "public",
                "table": "accounts"
            }],
            "table_definition_mismatches": []
        }
    }))
    .expect("verify mismatch job response should deserialize");

    let audit = VerifyCorrectnessAudit::new(vec!["public.accounts".to_string()], response);

    assert!(audit.selected_tables_mismatch());
}
