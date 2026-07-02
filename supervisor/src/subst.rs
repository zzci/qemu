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

/// Split a command line into argv on whitespace. Single or double quotes group a value that
/// contains spaces (no escapes, no expansion); an unterminated quote runs to the end of the line.
pub fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    in_token = true; // "" is a valid (empty) token
                }
                c if c.is_whitespace() => {
                    if in_token {
                        out.push(std::mem::take(&mut cur));
                        in_token = false;
                    }
                }
                c => {
                    cur.push(c);
                    in_token = true;
                }
            },
        }
    }
    if in_token {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{substitute, tokenize, Vars};

    #[test]
    fn substitutes_placeholders() {
        let mut vars = Vars::new();
        vars.insert("disk", "/vms/a.qcow2".into());
        vars.insert("ram", "2G".into());
        assert_eq!(substitute("-m {ram} -drive file={disk}", &vars), "-m 2G -drive file=/vms/a.qcow2");
        assert_eq!(substitute("no placeholders", &vars), "no placeholders");
    }

    #[test]
    fn tokenizes_on_whitespace() {
        assert_eq!(tokenize("qemu -m 2G"), vec!["qemu", "-m", "2G"]);
        assert_eq!(tokenize("  spaced\t out  "), vec!["spaced", "out"]);
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn quotes_group_values_with_spaces() {
        assert_eq!(
            tokenize(r#"qemu -drive "file=/vms/my disk.qcow2" -name 'win 11'"#),
            vec!["qemu", "-drive", "file=/vms/my disk.qcow2", "-name", "win 11"]
        );
        // quotes may appear mid-token, and "" is a real empty argument
        assert_eq!(tokenize(r#"-append root="/dev/sda 1" """#), vec!["-append", "root=/dev/sda 1", ""]);
    }

    #[test]
    fn unterminated_quote_runs_to_end() {
        assert_eq!(tokenize(r#"echo "a b"#), vec!["echo", "a b"]);
    }
}
