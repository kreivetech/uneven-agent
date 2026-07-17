use serde::{Deserialize, Serialize};
use crate::domain::common::{Severity, Logger, SecurityContext};
use std::borrow::Cow;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortDef {
    pub port: u16,
    pub service: Cow<'static, str>,
    pub dangerous: bool,
    pub severity: Severity,
    pub cvss: f32,
    pub message: Cow<'static, str>,
    pub recommendation: Cow<'static, str>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequiredHeader {
    pub header: Cow<'static, str>,
    pub key: Cow<'static, str>,
    pub severity: Severity,
    pub cvss: f32,
    pub static_keyword: Option<Cow<'static, str>>,
    pub message_template: Option<Cow<'static, str>>,
    pub recommendation: Cow<'static, str>,
}

impl RequiredHeader {
    pub fn build_message(&self, host: &str) -> String {
        if let Some(ref template) = self.message_template {
            template.replace("{host}", host).replace("{header}", &self.header)
        } else {
            format!("Missing security header '{}' on host '{}'", self.header, host)
        }
    }
}

pub trait ActiveCtx: Send + Sync {
    fn logger(&self) -> &dyn Logger;
    fn context(&self) -> &SecurityContext;
    fn audit_log(&self) -> &[String];
    fn audit(&mut self, msg: &str);
    fn flush_audit_log(&mut self) -> futures::future::BoxFuture<'_, Result<(), crate::domain::error::UnevenError>>;
}

pub static PORTS: &[PortDef] = &[
    PortDef { port: 21,    service: Cow::Borrowed("FTP"),               dangerous: true,  severity: Severity::High,     cvss: 7.5,  message: Cow::Borrowed("FTP (port 21) is open — transmits credentials in plain text"), recommendation: Cow::Borrowed("Disable FTP. Use SFTP (port 22) instead.") },
    PortDef { port: 22,    service: Cow::Borrowed("SSH"),               dangerous: false, severity: Severity::Info,     cvss: 0.0,  message: Cow::Borrowed("SSH (port 22) open"), recommendation: Cow::Borrowed("Ensure SSH uses key authentication and has fail2ban/rate limiting.") },
    PortDef { port: 23,    service: Cow::Borrowed("Telnet"),            dangerous: true,  severity: Severity::Critical, cvss: 9.8,  message: Cow::Borrowed("Telnet (port 23) is open — unencrypted remote access"), recommendation: Cow::Borrowed("Disable Telnet immediately. Use SSH.") },
    PortDef { port: 25,    service: Cow::Borrowed("SMTP"),              dangerous: false, severity: Severity::Info,     cvss: 0.0,  message: Cow::Borrowed("SMTP (port 25) open"), recommendation: Cow::Borrowed("Ensure SMTP relay is restricted. Enable STARTTLS.") },
    PortDef { port: 80,    service: Cow::Borrowed("HTTP"),              dangerous: false, severity: Severity::Info,     cvss: 0.0,  message: Cow::Borrowed("HTTP (port 80) open"), recommendation: Cow::Borrowed("Redirect all HTTP to HTTPS.") },
    PortDef { port: 443,   service: Cow::Borrowed("HTTPS"),             dangerous: false, severity: Severity::Info,     cvss: 0.0,  message: Cow::Borrowed("HTTPS (port 443) open"), recommendation: Cow::Borrowed("Verify TLS configuration.") },
    PortDef { port: 8080,  service: Cow::Borrowed("HTTP-alt"),          dangerous: false, severity: Severity::Low,      cvss: 3.1,  message: Cow::Borrowed("Alternate HTTP port 8080 open — may expose admin panels or dev servers"), recommendation: Cow::Borrowed("Verify what service is running. Restrict if not needed publicly.") },
    PortDef { port: 8443,  service: Cow::Borrowed("HTTPS-alt"),         dangerous: false, severity: Severity::Info,     cvss: 0.0,  message: Cow::Borrowed("Alternate HTTPS port 8443 open"), recommendation: Cow::Borrowed("Verify what service is running.") },
    PortDef { port: 1433,  service: Cow::Borrowed("MSSQL"),             dangerous: true,  severity: Severity::Critical, cvss: 9.8,  message: Cow::Borrowed("MSSQL (port 1433) exposed to network"), recommendation: Cow::Borrowed("Restrict MSSQL to internal network only. Enable Windows Authentication.") },
    PortDef { port: 3306,  service: Cow::Borrowed("MySQL"),             dangerous: true,  severity: Severity::Critical, cvss: 9.8,  message: Cow::Borrowed("MySQL (port 3306) exposed to network — database should not be publicly accessible"), recommendation: Cow::Borrowed("Bind MySQL to 127.0.0.1. Use a firewall to block external access.") },
    PortDef { port: 5432,  service: Cow::Borrowed("PostgreSQL"),        dangerous: true,  severity: Severity::Critical, cvss: 9.8,  message: Cow::Borrowed("PostgreSQL (port 5432) exposed to network"), recommendation: Cow::Borrowed("Bind PostgreSQL to 127.0.0.1. Use a firewall.") },
    PortDef { port: 5984,  service: Cow::Borrowed("CouchDB"),           dangerous: true,  severity: Severity::Critical, cvss: 9.8,  message: Cow::Borrowed("CouchDB (port 5984) exposed — web-accessible database"), recommendation: Cow::Borrowed("Restrict CouchDB to localhost. Enable authentication.") },
    PortDef { port: 6379,  service: Cow::Borrowed("Redis"),             dangerous: true,  severity: Severity::Critical, cvss: 10.0, message: Cow::Borrowed("Redis (port 6379) exposed — often has no authentication by default"), recommendation: Cow::Borrowed("Bind to 127.0.0.1, enable requirepass, use TLS.") },
    PortDef { port: 9200,  service: Cow::Borrowed("Elasticsearch"),     dangerous: true,  severity: Severity::Critical, cvss: 10.0, message: Cow::Borrowed("Elasticsearch (port 9200) exposed — no authentication by default in older versions"), recommendation: Cow::Borrowed("Enable X-Pack security, restrict access with firewall.") },
    PortDef { port: 27017, service: Cow::Borrowed("MongoDB"),           dangerous: true,  severity: Severity::Critical, cvss: 10.0, message: Cow::Borrowed("MongoDB (port 27017) exposed — may allow unauthenticated access"), recommendation: Cow::Borrowed("Enable authentication, bind to localhost, restrict with firewall.") },
    PortDef { port: 2375,  service: Cow::Borrowed("Docker"),            dangerous: true,  severity: Severity::Critical, cvss: 10.0, message: Cow::Borrowed("Docker API (port 2375) exposed without TLS — full container control possible"), recommendation: Cow::Borrowed("Never expose Docker API without TLS. Use unix socket instead.") },
    PortDef { port: 4444,  service: Cow::Borrowed("Metasploit"),        dangerous: true,  severity: Severity::Critical, cvss: 10.0, message: Cow::Borrowed("Port 4444 open — common Metasploit handler / backdoor port"), recommendation: Cow::Borrowed("Investigate immediately. Close if unauthorized.") },
    PortDef { port: 9000,  service: Cow::Borrowed("Portainer/PHP-FPM"), dangerous: true,  severity: Severity::High,     cvss: 8.6,  message: Cow::Borrowed("Port 9000 open — Portainer admin or PHP-FPM may be exposed"), recommendation: Cow::Borrowed("Restrict to internal network. Enable authentication.") },
];

pub static REQUIRED_HEADERS: &[RequiredHeader] = &[
    RequiredHeader {
        header: Cow::Borrowed("X-Content-Type-Options"),
        key: Cow::Borrowed("x-content-type-options"),
        static_keyword: Some(Cow::Borrowed("nosniff")),
        severity: Severity::Medium,
        cvss: 4.3,
        message_template: Some(Cow::Borrowed("Host '{host}' is missing X-Content-Type-Options: nosniff — browsers may MIME-sniff responses")),
        recommendation: Cow::Borrowed("Set response header: X-Content-Type-Options: nosniff"),
    },
    RequiredHeader {
        header: Cow::Borrowed("X-Frame-Options"),
        key: Cow::Borrowed("x-frame-options"),
        static_keyword: Some(Cow::Borrowed("SAMEORIGIN|DENY")),
        severity: Severity::Medium,
        cvss: 4.3,
        message_template: Some(Cow::Borrowed("Host '{host}' is missing X-Frame-Options — site may be vulnerable to clickjacking")),
        recommendation: Cow::Borrowed("Set response header: X-Frame-Options: SAMEORIGIN or DENY"),
    },
    RequiredHeader {
        header: Cow::Borrowed("Strict-Transport-Security"),
        key: Cow::Borrowed("strict-transport-security"),
        static_keyword: Some(Cow::Borrowed("max-age")),
        severity: Severity::High,
        cvss: 6.5,
        message_template: Some(Cow::Borrowed("Host '{host}' is missing HSTS header — users may be downgraded to unencrypted HTTP")),
        recommendation: Cow::Borrowed("Set header: Strict-Transport-Security: max-age=31536000; includeSubDomains"),
    },
    RequiredHeader {
        header: Cow::Borrowed("Content-Security-Policy"),
        key: Cow::Borrowed("content-security-policy"),
        static_keyword: Some(Cow::Borrowed("default-src|script-src|content-security-policy")),
        severity: Severity::High,
        cvss: 6.1,
        message_template: Some(Cow::Borrowed("Host '{host}' is missing Content-Security-Policy — no defense-in-depth against XSS")),
        recommendation: Cow::Borrowed("Define a CSP appropriate for your app. Start with: default-src 'self'"),
    },
    RequiredHeader {
        header: Cow::Borrowed("X-Powered-By removal"),
        key: Cow::Borrowed("x-powered-by"),
        static_keyword: Some(Cow::Borrowed("disable.*x-powered-by|removeHeader.*x-powered-by|helmet")),
        severity: Severity::Low,
        cvss: 2.6,
        message_template: Some(Cow::Borrowed("Host '{host}' exposes X-Powered-By header — reveals server technology stack")),
        recommendation: Cow::Borrowed("Remove the X-Powered-By header from responses."),
    },
];
