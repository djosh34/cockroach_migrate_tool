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
            "summary": {
                "tables_verified": 1,
                "tables_with_data": 1,
                "has_mismatches": false
            },
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
            "findings": [],
            "mismatch_summary": {
                "has_mismatches": false,
                "affected_tables": [],
                "counts_by_kind": {}
            }
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
            "summary": {
                "tables_verified": 1,
                "tables_with_data": 1,
                "has_mismatches": true
            },
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
            "findings": [{
                "kind": "mismatching_table_definition",
                "schema": "public",
                "table": "accounts",
                "message": "primary key mismatch"
            }],
            "mismatch_summary": {
                "has_mismatches": true,
                "affected_tables": [{
                    "schema": "public",
                    "table": "accounts"
                }],
                "counts_by_kind": {
                    "mismatching_table_definition": 1
                }
            }
        }
    }))
    .expect("verify mismatch job response should deserialize");

    let audit = VerifyCorrectnessAudit::new(vec!["public.accounts".to_string()], response);

    assert!(audit.selected_tables_mismatch());
}
