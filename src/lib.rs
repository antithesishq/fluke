use std::{fs, process::Command};

use eyre::{Context, Result, bail};
use tempfile::tempdir;
use tracing::trace;

use crate::impurity::Impurity;

pub mod cache;
pub mod impurity;
mod utils;
pub mod watchman;

pub use utils::{AbsolutePath, RelativePath};

#[tracing::instrument]
pub fn eval(path: &AbsolutePath, attr: &str, eval: bool) -> Result<(String, Vec<Impurity>)> {
    let impurity_log = tempdir().wrap_err("couldn't create temporary directory")?;
    let impurity_log = impurity_log.path().join("l");

    fs::write(&impurity_log, Vec::new())?;

    let out = {
        let mut cmd = Command::new(option_env!("LIX_PATH").unwrap_or("nix-instantiate"));

        cmd.args(["--extra-deprecated-features", "url-literals"])
            .arg(path.abs())
            .args(["-A", attr])
            .arg("--impurity-sock")
            .arg(&impurity_log);

        if eval {
            cmd.arg("--eval");
        }

        cmd.output().wrap_err("failed to execute nix")?
    };

    if !out.status.success() {
        bail!(
            "nix failed with {}:\n{}",
            out.status,
            String::from_utf8(out.stderr)?
        );
    }

    let impurities = match impurity_log.exists() {
        true => fs::read_to_string(impurity_log)
            .wrap_err("couldn't read impurity log file")?
            .lines()
            .inspect(|l| trace!(impurity = l))
            .map(serde_json::from_str::<Impurity>)
            .collect::<Result<Vec<_>, _>>()
            .wrap_err("couldn't parse impurity log")?,
        false => Vec::new(),
    };

    Ok((
        String::from_utf8(out.stdout)?.trim().to_string(),
        impurities,
    ))
}

#[cfg(test)]
#[allow(clippy::disallowed_types)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::Path,
    };

    use proptest::prelude::*;
    use tempfile::tempdir;
    use test_log::test;

    use crate::{AbsolutePath, eval, impurity::Impurity};

    #[derive(Clone, Debug)]
    enum NixExpr {
        Bool(bool),
        Int(u8),
        String(String),
        Array(Vec<NixExpr>),
        AttrSet(HashMap<String, NixExpr>),
        Import(String, Box<NixExpr>),
    }

    #[derive(Debug, Default)]
    struct State {
        files: HashMap<String, String>,
    }

    impl NixExpr {
        fn realize(&self, state: &mut State, root: bool) -> String {
            match self {
                NixExpr::Bool(b) => b.to_string(),
                NixExpr::Int(n) => n.to_string(),
                NixExpr::String(s) => format!(r#""{s}""#),
                NixExpr::Array(a) => {
                    let mut s = "[".to_string();

                    for i in a {
                        s.push_str(&i.realize(state, root));
                        s.push(' ');
                    }

                    s.push(']');
                    s
                }
                NixExpr::AttrSet(set) => {
                    let mut s = "{ ".to_string();

                    for (k, v) in set {
                        s.push_str(" \"");
                        s.push_str(k);
                        s.push_str("\" = ");
                        s.push_str(&v.realize(state, root));
                        s.push(';');
                    }

                    s.push('}');
                    s
                }
                NixExpr::Import(path, expr) => {
                    let r = expr.realize(state, false);
                    state.files.insert(path.clone(), r);

                    format!("(import ./{}{path}.nix)", if root { "t/" } else { "" })
                }
            }
        }

        fn impurities(&self, root: &Path) -> HashSet<Impurity> {
            let mut set = HashSet::new();

            match self {
                NixExpr::Bool(_) | NixExpr::Int(_) | NixExpr::String(_) => {}
                NixExpr::Array(a) => {
                    for e in a {
                        for i in e.impurities(root) {
                            set.insert(i);
                        }
                    }
                }
                NixExpr::AttrSet(s) => {
                    for e in s.values() {
                        for i in e.impurities(root) {
                            set.insert(i);
                        }
                    }
                }
                NixExpr::Import(p, e) => {
                    for i in e.impurities(root) {
                        set.insert(i);
                    }

                    set.insert(Impurity::Import {
                        path: AbsolutePath::new(root.join("t").join(format!("{p}.nix"))).unwrap(),
                    });
                }
            }

            set
        }
    }

    fn arb_nix() -> impl Strategy<Value = NixExpr> {
        let leaf = prop_oneof![
            any::<bool>().prop_map(NixExpr::Bool),
            any::<u8>().prop_map(NixExpr::Int),
            "[a-zA-Z]{1,32}".prop_map(NixExpr::String),
        ];

        leaf.prop_recursive(4, 32, 10, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..10).prop_map(NixExpr::Array),
                prop::collection::hash_map("[a-z]{1,8}", inner.clone(), 0..10)
                    .prop_map(NixExpr::AttrSet),
                ("[a-z]{8}", inner).prop_map(|(p, e)| NixExpr::Import(p, Box::new(e)))
            ]
        })
    }

    proptest! {
        #[test]
        fn pbt(expr in arb_nix()) {
            let dir = tempdir().unwrap();
            fs::create_dir(dir.path().join("t")).unwrap();

            let mut state = State::default();
            let realized = expr.realize(&mut state, true);

            eprintln!("realized = {realized}");

            for (path, content) in state.files {
                fs::write(dir.path().join("t").join(format!("{path}.nix")), content).unwrap();
            }

            fs::write(dir.path().join("expr.nix"), format!("{{ foo = builtins.toJSON ({realized}); }}")).unwrap();

            assert_eq!(eval(&AbsolutePath::new(dir.path().join("expr.nix")).unwrap(), "foo", true).unwrap().1.into_iter().collect::<HashSet<_>>(), expr.impurities(dir.path()));
        }
    }
}
