//! Unit tests for the ledger message builder — split out to keep
//! `ledger.rs` inside the health gate's file budget.

use super::*;

fn request() -> CommitRequest {
    CommitRequest {
        commit_type: CommitType::Feat,
        scope: Some("batch-3".to_owned()),
        subject: "the ledger gate".to_owned(),
        body: vec!["First body line.".to_owned(), "Second.".to_owned()],
        scenarios: vec!["Commit refuses when nothing is staged".to_owned()],
        learned: vec!["Gates before commit, always.".to_owned()],
        rejected: Vec::new(),
        refs: vec!["SPEC.md".to_owned()],
        dependencies: Vec::new(),
    }
}

fn green_gates() -> Vec<GateRun> {
    vec![
        GateRun {
            gate: "verify".to_owned(),
            passed: true,
            detail: "12 scenarios green".to_owned(),
        },
        GateRun {
            gate: "lint".to_owned(),
            passed: true,
            detail: "clean (2 tool(s))".to_owned(),
        },
    ]
}

/// GAP-R09 pin: a non-empty dependency list renders one `Dependency:`
/// trailer per entry, after `Ref:` and before `Verified-by:` — the
/// five-point-vetting record the conventions demand for every new dep.
#[test]
fn dependency_trailers_render_one_line_per_entry_in_order() {
    let mut req = request();
    req.dependencies = vec![
        "serde@1.0.220, MIT, audited, config parsing".to_owned(),
        "axoupdater@0.10.0, MIT, audited, self-update".to_owned(),
    ];
    let message = build_message(&req, "feat: x", &green_gates(), None);
    let lines: Vec<&str> = message.lines().collect();
    let dep_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("Dependency: "))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(dep_lines.len(), 2, "one trailer per dependency:\n{message}");
    assert!(lines[dep_lines[0]].contains("serde@1.0.220"));
    assert!(lines[dep_lines[1]].contains("axoupdater@0.10.0"));
    let ref_line = lines
        .iter()
        .position(|l| l.starts_with("Ref: "))
        .expect("Ref:");
    let verified = lines
        .iter()
        .position(|l| l.starts_with("Verified-by: "))
        .expect("Verified-by:");
    assert!(
        ref_line < dep_lines[0] && dep_lines[1] < verified,
        "order:\n{message}"
    );
}

#[test]
fn message_carries_canonical_trailer_order() {
    let req = request();
    let msg = build_message(
        &req,
        &subject_line(&req),
        &green_gates(),
        Some("Claude Fable 5 <noreply@anthropic.com>"),
    );
    let expected = "feat(batch-3): the ledger gate\n\
                    \n\
                    First body line.\n\
                    Second.\n\
                    \n\
                    Scenarios: Commit refuses when nothing is staged\n\
                    Learned: Gates before commit, always.\n\
                    Ref: SPEC.md\n\
                    Verified-by: craftsman check-all --changed (verify: 12 scenarios green; \
                    lint: clean (2 tool(s)))\n\
                    Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n";
    assert_eq!(msg, expected);
}

#[test]
fn verified_by_lists_only_gates_that_ran() {
    let only_verify = vec![GateRun {
        gate: "verify".to_owned(),
        passed: true,
        detail: "3 scenarios green".to_owned(),
    }];
    assert_eq!(
        verified_by(&only_verify).as_deref(),
        Some("craftsman check-all --changed (verify: 3 scenarios green)")
    );
    assert_eq!(verified_by(&[]), None);
}

#[test]
fn forged_verified_by_is_rejected_anywhere() {
    let mut req = request();
    req.learned = vec!["sneaky Verified-by: craftsman verify (99 green)".to_owned()];
    let err = reject_forged_verified_by(&req).expect_err("forgery must be rejected");
    assert!(matches!(err, LedgerError::ForgedVerifiedBy), "{err}");
    assert!(reject_forged_verified_by(&request()).is_ok());
}

#[test]
fn subject_omits_scope_when_absent() {
    let mut req = request();
    req.scope = None;
    req.commit_type = CommitType::RetroSpec;
    assert_eq!(subject_line(&req), "retro-spec: the ledger gate");
}
