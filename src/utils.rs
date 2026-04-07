#![allow(clippy::disallowed_types)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::builder::{PathBufValueParser, TypedValueParser, ValueParserFactory};
use eyre::{Result, eyre};
use serde::{Deserialize, Serialize};

// TODO: switch to Cow?

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct AbsolutePath(PathBuf);

impl AbsolutePath {
    pub fn new(p: impl Into<PathBuf>) -> Result<AbsolutePath> {
        let p = p.into();

        match p.is_absolute() {
            true => Ok(AbsolutePath(p)),
            false => Err(eyre!("{p:?} isn't absolute")),
        }
    }

    pub fn abs(&self) -> &Path {
        &self.0
    }
}

impl ValueParserFactory for AbsolutePath {
    type Parser = AbsolutePathParser;

    fn value_parser() -> Self::Parser {
        AbsolutePathParser
    }
}

#[derive(Clone, Copy)]
pub struct AbsolutePathParser;

impl TypedValueParser for AbsolutePathParser {
    type Value = AbsolutePath;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> std::result::Result<Self::Value, clap::Error> {
        AbsolutePath::new(fs::canonicalize(
            PathBufValueParser::new().parse_ref(cmd, arg, value)?,
        )?)
        .map_err(|e| clap::Error::raw(clap::error::ErrorKind::ValueValidation, e))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct RelativePath(PathBuf);

impl RelativePath {
    pub fn new(p: impl Into<PathBuf>) -> Result<RelativePath> {
        let p = p.into();

        match p.is_absolute() {
            false => Ok(RelativePath(p)),
            true => Err(eyre!("{p:?} isn't relative")),
        }
    }

    pub fn rel(&self) -> &Path {
        &self.0
    }
}
