use crate::{Result, model::Header};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub sender_patterns: Vec<String>,
    pub snippet_patterns: Vec<String>,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            sender_patterns: [
                "(?i)no-?reply@",
                "(?i)do-?not-?reply@",
                "(?i)notifications?@",
                "(?i)mailer-daemon@",
                "(?i)postmaster@",
            ]
            .map(String::from)
            .into(),
            snippet_patterns: Vec::new(),
        }
    }
}
#[derive(Debug)]
pub struct Classification {
    pub is_automated: bool,
    pub reason: String,
}

pub struct Classifier {
    senders: Vec<Regex>,
    snippets: Vec<Regex>,
}
impl Classifier {
    pub fn new(rules: Rules) -> Result<Self> {
        let compile = |patterns: Vec<String>| -> Result<Vec<Regex>> {
            patterns.into_iter().map(|p| Ok(Regex::new(&p)?)).collect()
        };
        Ok(Self {
            senders: compile(rules.sender_patterns)?,
            snippets: compile(rules.snippet_patterns)?,
        })
    }
    pub fn load(path: &Path) -> Result<Self> {
        let mut rules = Rules::default();
        match std::fs::read(path) {
            Ok(bytes) => {
                #[derive(Deserialize, Default)]
                #[serde(default)]
                struct Additional {
                    sender_patterns: Vec<String>,
                    snippet_patterns: Vec<String>,
                }
                let extra: Additional = serde_json::from_slice(&bytes)?;
                rules.sender_patterns.extend(extra.sender_patterns);
                rules.snippet_patterns.extend(extra.snippet_patterns);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Self::new(rules)
    }
    pub fn classify(&self, from: &str, snippet: &str, headers: &[Header]) -> Classification {
        let automated = |reason| Classification {
            is_automated: true,
            reason,
        };
        for header in headers {
            if header.name.eq_ignore_ascii_case("Auto-Submitted") {
                let keyword = header.value.split(';').next().unwrap_or_default().trim();
                if !keyword.is_empty() && !keyword.eq_ignore_ascii_case("no") {
                    return automated(format!("header:Auto-Submitted={}", header.value));
                }
            }
        }
        if headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("List-Unsubscribe"))
        {
            return automated("header:List-Unsubscribe".into());
        }
        for (patterns, value, prefix) in [
            (&self.senders, from, "sender"),
            (&self.snippets, snippet, "snippet"),
        ] {
            if let Some(pattern) = patterns.iter().find(|p| p.is_match(value)) {
                return automated(format!("{prefix}:{pattern}"));
            }
        }
        Classification {
            is_automated: false,
            reason: "human".into(),
        }
    }
}
