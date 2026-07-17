use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TestFramework {
    Jest,
    Vitest,
    Mocha,
    Jasmine,
    Pytest,
    CargoTest,
    GoTest,
    Phpunit,
    Rspec,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub framework: TestFramework,
    pub passed: bool,
    pub total: u32,
    pub passing: u32,
    pub failing: u32,
    pub skipped: u32,
    pub duration: u64, // ms
    pub output: String,
    pub failed_tests: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestRunnerConfig {
    pub timeout_ms: Option<u64>,
    pub target_file: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

impl std::fmt::Display for TestFramework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TestFramework::Jest => "Jest",
            TestFramework::Vitest => "Vitest",
            TestFramework::Mocha => "Mocha",
            TestFramework::Jasmine => "Jasmine",
            TestFramework::Pytest => "Pytest",
            TestFramework::CargoTest => "Cargo Test",
            TestFramework::GoTest => "Go Test",
            TestFramework::Phpunit => "PHPUnit",
            TestFramework::Rspec => "RSpec",
            TestFramework::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

