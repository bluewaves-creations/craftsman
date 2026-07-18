//! Parsers for the security-gate scanners: gitleaks, semgrep,
//! osv-scanner — each mapping one scanner's JSON report into normalized
//! findings (secret values never copied into messages).

use super::super::{Finding, GateError, Severity, fnv_hex};

use super::{finding, json_value};

/// `gitleaks git --report-format json`: an array of leaks. Secrets never
/// enter the finding message — only the rule's description; the secret's
/// content contributes (hashed) to the baseline fingerprint via `message`?
/// No: the fingerprint hashes this message, so the message carries a hash
/// of the secret, not the secret.
pub(super) fn parse_gitleaks(report: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("gitleaks", report)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "gitleaks",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let secret_hash = fnv_hex(v["Secret"].as_str().unwrap_or_default());
            finding(
                "security",
                "gitleaks",
                v["RuleID"].as_str().unwrap_or("gitleaks"),
                v["File"].as_str().unwrap_or_default(),
                v["StartLine"].as_u64(),
                format!(
                    "{} [secret fnv:{secret_hash}]",
                    v["Description"].as_str().unwrap_or("secret detected")
                ),
                Severity::Critical,
            )
        })
        .collect())
}

/// `semgrep scan --json`: `{"results": [...]}`.
pub(super) fn parse_semgrep(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("semgrep", stdout)?;
    let results = doc["results"].as_array().ok_or_else(|| GateError::Parse {
        tool: "semgrep",
        detail: "expected a `results` array".to_owned(),
    })?;
    Ok(results
        .iter()
        .map(|v| {
            let severity = match v["extra"]["severity"].as_str().unwrap_or_default() {
                "ERROR" => Severity::High,
                "WARNING" => Severity::Medium,
                _ => Severity::Info,
            };
            finding(
                "security",
                "semgrep",
                v["check_id"].as_str().unwrap_or("semgrep"),
                v["path"].as_str().unwrap_or_default(),
                v["start"]["line"].as_u64(),
                v["extra"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `osv-scanner scan source --format json`: `{"results": [{source,
/// packages: [{package, vulnerabilities}]}]}`. Severity comes from
/// `database_specific.severity` when present; unknown = High (conservative:
/// an unrated vulnerability is not a pass).
pub(super) fn parse_osv(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("osv-scanner", stdout)?;
    let results = match doc["results"].as_array() {
        Some(r) => r,
        // A clean scan may emit `{"results":[]}` or omit results entirely.
        None if doc.is_object() => return Ok(Vec::new()),
        None => {
            return Err(GateError::Parse {
                tool: "osv-scanner",
                detail: "expected a `results` array".to_owned(),
            });
        }
    };
    let mut findings = Vec::new();
    for source in results {
        let file = source["source"]["path"].as_str().unwrap_or_default();
        for pkg in source["packages"].as_array().unwrap_or(&Vec::new()) {
            let name = pkg["package"]["name"].as_str().unwrap_or("?");
            let version = pkg["package"]["version"].as_str().unwrap_or("?");
            for vuln in pkg["vulnerabilities"].as_array().unwrap_or(&Vec::new()) {
                let id = vuln["id"].as_str().unwrap_or("OSV");
                let severity = match vuln["database_specific"]["severity"]
                    .as_str()
                    .unwrap_or_default()
                    .to_ascii_uppercase()
                    .as_str()
                {
                    "CRITICAL" => Severity::Critical,
                    "MODERATE" | "MEDIUM" => Severity::Medium,
                    "LOW" => Severity::Low,
                    // "HIGH" and anything unrated: an unknown severity is
                    // not a pass.
                    _ => Severity::High,
                };
                findings.push(finding(
                    "security",
                    "osv-scanner",
                    id,
                    file,
                    None,
                    format!(
                        "{name}@{version}: {}",
                        vuln["summary"].as_str().unwrap_or("known vulnerability")
                    ),
                    severity,
                ));
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::super::super::Severity;
    use super::*;

    #[test]
    fn security_reports_parse_and_hide_secrets() {
        let leaks = r#"[{"RuleID":"aws-access-key","File":"config/prod.env","StartLine":2,"Description":"AWS Access Key","Secret":"AKIA123","Commit":"abc"}]"#;
        let f = parse_gitleaks(leaks).expect("parses");
        assert_eq!(f[0].severity, Severity::Critical);
        assert!(!f[0].message.contains("AKIA123"), "secret must not leak");
        assert!(
            f[0].message.contains("fnv:"),
            "hash anchors the fingerprint"
        );

        let sg = r#"{"results":[{"check_id":"rust.lang.security.unsafe","path":"src/x.rs","start":{"line":9},"extra":{"message":"unsafe block\nmore","severity":"ERROR"}}],"errors":[]}"#;
        let f = parse_semgrep(sg).expect("parses");
        assert_eq!(f[0].severity, Severity::High);
        assert_eq!(f[0].message, "unsafe block");

        let osv = r#"{"results":[{"source":{"path":"/r/Cargo.lock","type":"lockfile"},"packages":[{"package":{"name":"time","version":"0.1.0","ecosystem":"crates.io"},"vulnerabilities":[{"id":"RUSTSEC-2020-0071","summary":"Segfault in time","database_specific":{"severity":"MODERATE"}}]}]}]}"#;
        let f = parse_osv(osv).expect("parses");
        assert_eq!(f[0].rule, "RUSTSEC-2020-0071");
        assert_eq!(f[0].severity, Severity::Medium);
        assert!(f[0].message.contains("time@0.1.0"));
        assert!(parse_osv("{}").expect("clean scan").is_empty());
    }
}
