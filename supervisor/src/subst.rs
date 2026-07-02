//! Placeholder substitution + tokenizing for the config's command templates.

use std::collections::BTreeMap;

pub type Vars = BTreeMap<&'static str, String>;

/// Replace every `{key}` in `template` with its value.
pub fn substitute(template: &str, vars: &Vars) -> String {
    let mut s = template.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Split a command line into argv on whitespace (values must not contain spaces).
pub fn tokenize(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}
