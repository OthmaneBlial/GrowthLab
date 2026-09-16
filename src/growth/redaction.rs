//! Common credential redaction. Defense in depth cannot recognize every secret
//! format or every kind of private data; permission boundaries remain required.
use regex::Regex;
use std::sync::OnceLock;

fn patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| [
        r"(?s)-----BEGIN (?:[A-Z ]+ )?PRIVATE KEY-----.*?-----END (?:[A-Z ]+ )?PRIVATE KEY-----",
        r"(?s)-----BEGIN (?:[A-Z ]+ )?PRIVATE KEY-----.*",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-(?:proj-|ant-)?[A-Za-z0-9_-]{16,}|AKIA[A-Z0-9]{16}|xox[baprs]-[A-Za-z0-9-]{10,})\b",
        r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}",
        r#"(?i)\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|_?auth[_-]?token|token|password|passwd|client[_-]?secret|secret|aws_secret_access_key|database_url)\b["']?\s*[:=]\s*(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s"',;}]+)"#,
        r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b",
        r"(?i)(?:https?|postgres(?:ql)?|mysql|redis|mongodb(?:\+srv)?)://[^/\s:@]+:[^/\s@]+@",
    ].into_iter().map(|value| Regex::new(value).expect("static credential pattern")).collect())
}

pub fn redact(value: &str) -> String {
    patterns().iter().fold(value.to_string(), |value, pattern| {
        pattern.replace_all(&value, "[REDACTED]").into_owned()
    })
}

pub fn contains_secret(value: &str) -> bool {
    patterns().iter().any(|pattern| pattern.is_match(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_common_credentials_in_json_shell_headers_urls_and_pem() {
        for value in [
            "ghp_abcdefghijklmnopqrstuvwxyz123456",
            "github_pat_abcdefghijklmnopqrstuvwxyz123456",
            "sk-proj-abcdefghijklmnopqrstuvwxyz123456",
            "AKIA1234567890ABCDEF",
            "xoxb-1234567890-secret",
            "Authorization: Bearer abcdefghijklmnop",
            "Authorization: Basic abcdefghijklmnop",
            "API_KEY=fixture-private-value",
            "{\"password\": \"fixture-private-value\"}",
            "{\"client_secret\":\"fixture-private-value\"}",
            "PASSWORD='fixture-private-value with spaces'",
            "{\"password\":\"fixture-private-value with \\\"quotes\\\"\"}",
            "AWS_SECRET_ACCESS_KEY=fixture-private-value",
            "_authToken=fixture-private-value",
            "postgres://person:fixture-private-value@example.invalid/db",
            "eyJabcdefghijk.abcdefghijklmnop.abcdefghijklmnop",
            "https://person:fixture-private-value@example.invalid/file",
            "-----BEGIN RSA PRIVATE KEY-----\nfixture-private-value\n-----END RSA PRIVATE KEY-----",
            "-----BEGIN PRIVATE KEY-----\nfixture-private-value",
        ] {
            assert!(contains_secret(value), "fixture format was not detected");
            let output = redact(value);
            assert!(output.contains("[REDACTED]"));
            assert!(!output.contains("fixture-private-value"));
            assert_eq!(redact(&output), output, "redaction is idempotent");
        }
        let ordinary = "Conversion is untested. node website/check.mjs succeeded. Tokens used: 42.";
        assert!(!contains_secret(ordinary));
        assert_eq!(redact(ordinary), ordinary);
    }
}
