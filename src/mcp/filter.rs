//! Runtime filter for the MCP tool list.
//!
//! `RawFilter` holds unparsed CLI/env values. `Filter::resolve` normalizes,
//! validates, and applies the filter pipeline, returning either a `Filter`
//! whose `allows()` is the tool-name gate used by `tool_list()` and
//! `call_tool`, or a `FilterError` to surface at startup.

use std::collections::HashSet;

use crate::mcp::classify::{classify, Class, ToolMeta, ALL_GROUPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    All,
    Read,
    Safe,
}

impl Profile {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "all" => Some(Profile::All),
            "read" => Some(Profile::Read),
            "safe" => Some(Profile::Safe),
            _ => None,
        }
    }

    fn allows_class(self, class: Class) -> bool {
        match (self, class) {
            (Profile::All, _) => true,
            (Profile::Read, Class::Read) => true,
            (Profile::Read, _) => false,
            (Profile::Safe, Class::Destructive) => false,
            (Profile::Safe, _) => true,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Profile::All => "all",
            Profile::Read => "read",
            Profile::Safe => "safe",
        }
    }
}

/// Raw filter inputs before validation.
#[derive(Debug, Default, Clone)]
pub struct RawFilter {
    pub profile: Option<String>,
    pub read_only: bool,
    pub groups: Option<Vec<String>>,
    pub exclude_groups: Option<Vec<String>>,
    pub tools: Option<Vec<String>>,
    pub exclude_tools: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum FilterError {
    UnknownProfile {
        name: String,
    },
    UnknownGroup {
        name: String,
        valid: Vec<&'static str>,
    },
    UnknownTool {
        name: String,
        suggestion: Option<String>,
    },
    ConflictingProfile {
        profile: String,
    },
    ToolExcludedByProfile {
        tool: String,
        profile: String,
    },
    EmptyFilter,
}

impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterError::UnknownProfile { name } => {
                write!(f, "unknown --profile: {} (valid: all, read, safe)", name)
            }
            FilterError::UnknownGroup { name, valid } => {
                write!(f, "unknown group: {} (valid: {})", name, valid.join(", "))
            }
            FilterError::UnknownTool { name, suggestion } => match suggestion {
                Some(s) => write!(f, "unknown tool: {} (did you mean {}?)", name, s),
                None => write!(f, "unknown tool: {}", name),
            },
            FilterError::ConflictingProfile { profile } => {
                write!(
                    f,
                    "conflicting profile flags: --read-only and --profile {}",
                    profile
                )
            }
            FilterError::ToolExcludedByProfile { tool, profile } => write!(
                f,
                "tool {} is excluded by profile={}; drop --profile or remove {} from --tools",
                tool, profile, tool
            ),
            FilterError::EmptyFilter => {
                write!(
                    f,
                    "filter pipeline produced an empty tool set; nothing to expose"
                )
            }
        }
    }
}

impl std::error::Error for FilterError {}

#[derive(Debug)]
pub struct Filter {
    pub profile: Profile,
    pub groups: Option<Vec<String>>,
    pub exclude_groups: Option<Vec<String>>,
    allowed: HashSet<String>,
}

impl Filter {
    pub fn allows(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }

    pub fn allowed_count(&self) -> usize {
        self.allowed.len()
    }

    pub fn resolve(raw: RawFilter) -> Result<Self, FilterError> {
        // 1. Resolve profile, reconciling --read-only with --profile.
        let profile = match (raw.profile.as_deref(), raw.read_only) {
            (None, false) => Profile::All,
            (None, true) => Profile::Read,
            (Some(p), false) => {
                Profile::parse(p).ok_or_else(|| FilterError::UnknownProfile { name: p.into() })?
            }
            (Some("read"), true) => Profile::Read,
            (Some(p), true) => return Err(FilterError::ConflictingProfile { profile: p.into() }),
        };
        let profile_label = profile.as_str();

        // 2. Validate group inputs.
        for groups in [&raw.groups, &raw.exclude_groups].into_iter().flatten() {
            for g in groups {
                if !ALL_GROUPS.contains(&g.as_str()) {
                    return Err(FilterError::UnknownGroup {
                        name: g.clone(),
                        valid: ALL_GROUPS.to_vec(),
                    });
                }
            }
        }

        // 3. Enumerate all tool names + their ToolMeta (via tool_list + classify).
        let all_names: Vec<(String, ToolMeta)> = crate::mcp::tool_list()
            .as_array()
            .expect("tool_list returns array")
            .iter()
            .filter_map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .and_then(|n| classify(n).map(|m| (n.to_string(), m)))
            })
            .collect();
        let known_names: HashSet<&str> = all_names.iter().map(|(n, _)| n.as_str()).collect();

        // 4. Validate --tools / --exclude-tools names against the full catalog.
        for tools in [&raw.tools, &raw.exclude_tools].into_iter().flatten() {
            for t in tools {
                if !known_names.contains(t.as_str()) {
                    return Err(FilterError::UnknownTool {
                        name: t.clone(),
                        suggestion: closest_name(t, &known_names),
                    });
                }
            }
        }

        // 5. Detect --tools entries that are excluded by the profile before pipelining.
        if let Some(tools) = &raw.tools {
            for t in tools {
                if let Some(meta) = all_names.iter().find(|(n, _)| n == t).map(|(_, m)| m) {
                    if !profile.allows_class(meta.class) {
                        return Err(FilterError::ToolExcludedByProfile {
                            tool: t.clone(),
                            profile: profile_label.into(),
                        });
                    }
                }
            }
        }

        // 6. Build the allowed set via the pipeline.
        let mut allowed: HashSet<String> = all_names
            .iter()
            .filter(|(_, m)| profile.allows_class(m.class))
            .map(|(n, _)| n.clone())
            .collect();

        if let Some(groups) = &raw.groups {
            allowed.retain(|n| {
                let g = all_names
                    .iter()
                    .find(|(name, _)| name == n)
                    .map(|(_, m)| m.group)
                    .unwrap_or("");
                groups.iter().any(|wanted| wanted == g)
            });
        }

        if let Some(excl) = &raw.exclude_groups {
            allowed.retain(|n| {
                let g = all_names
                    .iter()
                    .find(|(name, _)| name == n)
                    .map(|(_, m)| m.group)
                    .unwrap_or("");
                !excl.iter().any(|bad| bad == g)
            });
        }

        if let Some(tools) = &raw.tools {
            let wanted: HashSet<&str> = tools.iter().map(String::as_str).collect();
            allowed.retain(|n| wanted.contains(n.as_str()));
        }

        if let Some(excl) = &raw.exclude_tools {
            for t in excl {
                allowed.remove(t);
            }
        }

        if allowed.is_empty() {
            return Err(FilterError::EmptyFilter);
        }

        Ok(Filter {
            profile,
            groups: raw.groups,
            exclude_groups: raw.exclude_groups,
            allowed,
        })
    }
}

fn closest_name(needle: &str, haystack: &HashSet<&str>) -> Option<String> {
    haystack
        .iter()
        .map(|c| (c, levenshtein(needle, c)))
        .min_by_key(|&(_, d)| d)
        .filter(|&(_, d)| d <= 3)
        .map(|(s, _)| s.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
