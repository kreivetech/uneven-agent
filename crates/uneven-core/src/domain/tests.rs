use crate::domain::*;
use serde_json::json;
use std::borrow::Cow;

#[test]
fn test_config_deserialization_camel_case() {
    let json_data = json!({
        "projectRoot": "/path/to/project",
        "brain": {
            "provider": "openai",
            "model": "gpt-4",
            "apiKey": "sk-12345",
            "temperature": 0.5,
            "maxTokens": 500
        },
        "watch": {
            "terminal": "npm start",
            "autoFix": true,
            "confirmBeforeFix": false
        },
        "security": {
            "blockedColumns": ["password", "token"],
            "blockedTables": ["users"]
        }
    });

    let config: UnevenConfig = serde_json::from_value(json_data).unwrap();
    assert_eq!(config.project_root, "/path/to/project");
    assert_eq!(config.brain.provider, "openai");
    assert_eq!(config.brain.model, "gpt-4");
    assert_eq!(config.brain.api_key.unwrap(), "sk-12345");
    assert_eq!(config.brain.temperature, 0.5);
    assert_eq!(config.brain.max_tokens, 500);
    assert_eq!(
        config.watch.as_ref().unwrap().terminal.as_ref().unwrap(),
        "npm start"
    );
    assert!(config.watch.as_ref().unwrap().auto_fix.unwrap());
    assert!(!config.watch.as_ref().unwrap().confirm_before_fix.unwrap());
    assert_eq!(config.security.blocked_columns, vec!["password", "token"]);
    assert_eq!(config.security.blocked_tables, vec!["users"]);
}

#[test]
fn test_config_deserialization_snake_case_alias() {
    let json_data = json!({
        "project_root": "/path/to/project",
        "brain": {
            "provider": "local",
            "model": "llama3",
            "api_key": "local-key",
            "temperature": 0.0,
            "max_tokens": 100
        },
        "watch": {
            "terminal": "npm run dev",
            "auto_fix": false,
            "confirm_before_fix": true
        },
        "security": {
            "blocked_columns": ["salt"],
            "blocked_tables": ["keys"]
        }
    });

    let config: UnevenConfig = serde_json::from_value(json_data).unwrap();
    assert_eq!(config.project_root, "/path/to/project");
    assert_eq!(config.brain.provider, "local");
    assert_eq!(config.brain.model, "llama3");
    assert_eq!(config.brain.api_key.unwrap(), "local-key");
    assert_eq!(config.brain.temperature, 0.0);
    assert_eq!(config.brain.max_tokens, 100);
    assert_eq!(
        config.watch.as_ref().unwrap().terminal.as_ref().unwrap(),
        "npm run dev"
    );
    assert!(!config.watch.as_ref().unwrap().auto_fix.unwrap());
    assert!(config.watch.as_ref().unwrap().confirm_before_fix.unwrap());
    assert_eq!(config.security.blocked_columns, vec!["salt"]);
    assert_eq!(config.security.blocked_tables, vec!["keys"]);
}

#[test]
fn test_scope_check_deserialization() {
    // Violation case (blocked: true)
    let violation_json = json!({
        "blocked": "true",
        "target": "10.0.0.1",
        "reason": "Out of scope",
        "suggestion": "Use approved target"
    });
    let check: ScopeCheck = serde_json::from_value(violation_json).unwrap();
    match check {
        ScopeCheck::Violation(v) => {
            assert_eq!(v.target, "10.0.0.1");
            assert_eq!(v.reason, "Out of scope");
        }
        _ => panic!("Expected ScopeCheck::Violation"),
    }

    // Pass case (blocked: false)
    let pass_json = json!({
        "blocked": "false",
        "target": "192.168.1.1"
    });
    let check: ScopeCheck = serde_json::from_value(pass_json).unwrap();
    match check {
        ScopeCheck::Pass(p) => {
            assert_eq!(p.target, "192.168.1.1");
        }
        _ => panic!("Expected ScopeCheck::Pass"),
    }
}

#[test]
fn test_fix_available_deserialization() {
    // Boolean case
    let boolean_json = json!(true);
    let fix: FixAvailable = serde_json::from_value(boolean_json).unwrap();
    assert_eq!(fix, FixAvailable::Boolean(true));

    // Details case
    let details_json = json!({
        "name": "lodash",
        "version": "4.17.21",
        "isSemVerMajor": true
    });
    let fix: FixAvailable = serde_json::from_value(details_json).unwrap();
    match fix {
        FixAvailable::Details(d) => {
            assert_eq!(d.name, "lodash");
            assert_eq!(d.version, "4.17.21");
            assert!(d.is_sem_ver_major);
        }
        _ => panic!("Expected FixAvailable::Details"),
    }
}

#[test]
fn test_required_header_message_building() {
    let header = RequiredHeader {
        header: Cow::Borrowed("Content-Security-Policy"),
        key: Cow::Borrowed("csp"),
        severity: Severity::High,
        cvss: 7.5,
        static_keyword: None,
        message_template: Some(Cow::Borrowed("Header {header} is missing on {host}")),
        recommendation: Cow::Borrowed("Add CSP header"),
    };
    let msg = header.build_message("localhost");
    assert_eq!(
        msg,
        "Header Content-Security-Policy is missing on localhost"
    );
}

#[test]
fn test_static_rules_regex_compilation() {
    assert!(!STATIC_RULES.is_empty());
    for rule in STATIC_RULES {
        let compiled = rule.compile_regex();
        assert!(
            compiled.is_ok(),
            "Failed to compile regex pattern '{}' for category '{:?}': {:?}",
            rule.pattern,
            rule.category,
            compiled.err()
        );
    }
}

#[test]
fn test_ports_and_required_headers_exposed() {
    assert!(!PORTS.is_empty());
    assert!(!REQUIRED_HEADERS.is_empty());

    // Check specific port definition
    let ftp = PORTS.iter().find(|p| p.port == 21).unwrap();
    assert_eq!(ftp.service, "FTP");
    assert!(ftp.dangerous);
    assert_eq!(ftp.severity, Severity::High);

    // Check specific required header
    let hsts = REQUIRED_HEADERS
        .iter()
        .find(|h| h.key == "strict-transport-security")
        .unwrap();
    assert_eq!(hsts.header, "Strict-Transport-Security");
    assert_eq!(hsts.severity, Severity::High);
}

#[test]
fn test_security_default_policy() {
    let policy = SecurityPolicy::default_policy();
    assert_eq!(policy.max_rows_per_analysis, Some(1));
    assert!(policy
        .blocked_columns
        .as_ref()
        .unwrap()
        .contains(&"password".to_string()));
    assert!(policy
        .blocked_tables
        .as_ref()
        .unwrap()
        .contains(&"sessions".to_string()));
    assert_eq!(SKIP_DIRS.len(), 12);
    assert_eq!(TEXT_EXTENSIONS.len(), 20);
}

#[test]
fn test_prompts_generation() {
    let analysis_prompt = build_analysis_prompt("table users");
    assert!(analysis_prompt.contains("table users"));
    assert!(analysis_prompt.contains("<suggestions>"));

    let sql_gen = build_sql_gen_prompt(Some("postgresql"), "schema", "get users", "rules");
    assert!(sql_gen.contains("postgresql"));
    assert!(sql_gen.contains("schema"));
    assert!(sql_gen.contains("get users"));
    assert!(sql_gen.contains("rules"));

    let style =
        build_style_extraction_prompt("style info", &["id".to_string(), "name".to_string()]);
    assert!(style.contains("style info"));
    assert!(style.contains("id, name"));

    let malware = build_malware_eval_prompt("eval(payload)", "rule description", "app.js");
    assert!(malware.contains("eval(payload)"));
    assert!(malware.contains("rule description"));
    assert!(malware.contains("app.js"));
}

#[test]
fn test_templates_constants() {
    assert!(!DASHBOARD_CSS.is_empty());
    assert!(!DASHBOARD_SCRIPTS.is_empty());
    assert_eq!(
        CHART_JS_CDN,
        "https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"
    );

    let viewer = build_report_viewer_script("<html></html>", "My Dashboard");
    assert!(viewer.contains("My Dashboard"));
    assert!(viewer.contains("<html></html>"));

    let security_report = build_security_html_report(
        "my-project",
        "127.0.0.1",
        "2026-05-23",
        "findings",
        1,
        2,
        3,
        4,
        5,
        "remediation",
        "CRITICAL",
        "red",
    );
    assert!(security_report.contains("Security Report — my-project"));
    assert!(security_report.contains("127.0.0.1"));
    assert!(security_report.contains("2026-05-23"));
    assert!(security_report.contains("findings"));
    assert!(security_report.contains("remediation"));
}
