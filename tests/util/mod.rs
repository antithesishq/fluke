#![allow(clippy::disallowed_types)]

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use eyre::Result;

pub struct FixtureFactory {
    root: PathBuf,
}

/// A file or directory created under a certain root.
#[allow(unused)]
pub enum Fixture<P: AsRef<Path>> {
    File(P, &'static str),
    Directory(P),
    Append(P, &'static str),
}

impl FixtureFactory {
    pub fn new(root: &Path) -> FixtureFactory {
        FixtureFactory {
            root: root.to_path_buf(),
        }
    }

    pub fn create_fixtures<P: AsRef<Path>>(
        &self,
        fixtures: impl IntoIterator<Item = Fixture<P>>,
    ) -> Result<()> {
        for f in fixtures.into_iter() {
            let path = f.path(&self.root);

            match f {
                Fixture::File(_, content) => {
                    fs::create_dir_all(path.parent().unwrap())?;
                    fs::write(path, content)?;
                }
                Fixture::Directory(_) => {
                    fs::create_dir_all(path)?;
                }
                Fixture::Append(_, content) => {
                    write!(OpenOptions::new().append(true).open(path)?, "{content}")?;
                }
            }
        }

        Ok(())
    }
}

impl<P: AsRef<Path>> Fixture<P> {
    fn path(&self, root: &Path) -> PathBuf {
        match self {
            Fixture::File(path, _) | Fixture::Directory(path) | Fixture::Append(path, _) => {
                let path = path.as_ref();

                if path.is_absolute() && path.strip_prefix(root).is_ok() {
                    path.to_path_buf()
                } else {
                    assert!(
                        !path.is_absolute(),
                        "absolute paths must be descendents of root"
                    );

                    root.join(path)
                }
            }
        }
    }
}
