#![allow(clippy::disallowed_types)]

use std::{fs, path::Path};

use assert_cmd::{Command, assert::Assert, cargo::cargo_bin_cmd};
use predicates::prelude::predicate;
use tempfile::tempdir;

use crate::util::{Fixture, FixtureFactory};

mod util;

const EXPR_PATH: &str = "expr.nix";
const ATTR_NAME: &str = "attr";
const MEOW: &str = "/nix/store/nym9s689an41amsi3nflg4gigxs9wq5x-meow.drv\n";
const BARK: &str = "/nix/store/a6bb5ndq3r8g36smp4k59w1ja9znd0my-bark.drv\n";

fn fluke_cmd(p: &Path) -> Command {
    let mut cmd = cargo_bin_cmd!();

    cmd.env_clear()
        .current_dir(p)
        .env("HOME", p)
        .env("PATH", env!("WATCHMAN_PATH"))
        .env("RUST_LOG", "fluke=debug")
        .env("NO_COLOR", "1")
        .arg("--root")
        .arg(p)
        .args([EXPR_PATH, ATTR_NAME]);

    cmd
}

enum CacheStatus {
    Valid,
    Invalid,
}

trait AssertExt {
    fn cache_status(self, status: CacheStatus) -> Self;
}

impl AssertExt for Assert {
    fn cache_status(self, status: CacheStatus) -> Self {
        self.stderr(predicate::str::contains(match status {
            CacheStatus::Valid => "status: Valid",
            CacheStatus::Invalid => "status: Invalid(",
        }))
    }
}

#[test]
fn trivial() {
    let dir = tempdir().unwrap();
    let fixture = FixtureFactory::new(dir.path());

    fixture
        .create_fixtures([Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = "meow"; system = builtins.currentSystem; builder = "/bin/sh"; }; }"#)])
        .unwrap();

    fluke_cmd(dir.path()).assert().success().stdout(MEOW);

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Valid);

    fixture
        .create_fixtures([Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = "bark"; system = builtins.currentSystem; builder = "/bin/sh"; }; }"#)])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(BARK)
        .cache_status(CacheStatus::Invalid)
        .stderr(predicate::str::contains(
            "nix file changed, path: RelativePath(\"expr.nix\")",
        ));
}

#[test]
fn import() {
    let dir = tempdir().unwrap();
    let fixture = FixtureFactory::new(dir.path());

    fixture
        .create_fixtures([Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = import ./name; system = builtins.currentSystem; builder = "/bin/sh"; }; }"#), Fixture::File("name", "\"meow\"")])
        .unwrap();

    fluke_cmd(dir.path()).assert().success().stdout(MEOW);

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Valid);

    fixture
        .create_fixtures([Fixture::File("name", "\"bark\"")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(BARK)
        .cache_status(CacheStatus::Invalid);
}

#[test]
fn read_dir() {
    const MEOW: &str = "/nix/store/v87f5wjr3f48fdwmrbw71ccblsqqnmrg-meow.drv\n";

    let dir = tempdir().unwrap();
    let fixture = FixtureFactory::new(dir.path());

    fixture
        .create_fixtures([
            Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = "meow"; system = builtins.currentSystem; builder = "/bin/sh"; args = [
                ((builtins.readDir ./dir).file)
            ]; }; }"#),
            Fixture::Directory("dir"),
            Fixture::File("dir/file", ""),
        ])
        .unwrap();

    fluke_cmd(dir.path()).assert().success().stdout(MEOW);

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Valid);

    // Creating a new file within the directory that was readDir'd invalidates the cache.
    fixture
        .create_fixtures([Fixture::File("dir/file2", "\"bark\"")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Invalid);

    // Creating a new directory invalidates the cache.
    fixture
        .create_fixtures([Fixture::Directory("dir/nested")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Invalid);

    // Creating a nested file invalidates the cache, as a new inode was added to the nested directory.
    // TODO: Consider making this case not invalidate the cache by querying Watchman for more data. For
    // now, though, let's lean on the side of being cautious.
    fixture
        .create_fixtures([Fixture::File("dir/nested/meow", "meow")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Invalid);

    // Modifying the nested file's content does not invalidate the cache.
    fixture
        .create_fixtures([Fixture::File("dir/nested/meow", "meow2")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Valid);
}

#[test]
fn paths() {
    let dir = tempdir().unwrap();
    let fixture = FixtureFactory::new(dir.path());

    fixture
        .create_fixtures([
            Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = "meow"; system = builtins.currentSystem; builder = "/bin/sh"; dir = ./dir; }; }"#),
            Fixture::Directory("dir"),
            Fixture::File("dir/file", ""),
        ])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/zi2qdbx4j56flqha571bsaiw9vcm9439-meow.drv\n");

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/zi2qdbx4j56flqha571bsaiw9vcm9439-meow.drv\n")
        .cache_status(CacheStatus::Valid);

    // Modifying an existing file within the directory that was read invalidates the cache.
    fixture
        .create_fixtures([Fixture::File("dir/file", "bark")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/kvpkpm97sf4ia351h4kf6rhykfgh0vr8-meow.drv\n")
        .cache_status(CacheStatus::Invalid);

    // Creating a new file within the directory that was read invalidates the cache.
    fixture
        .create_fixtures([Fixture::File("dir/file2", "bark")])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/9mayvqp45g9m4mfdlna48vv92bjqrsgi-meow.drv\n")
        .cache_status(CacheStatus::Invalid);

    // Deleting a file invalidates the cache.
    fs::remove_file(dir.path().join("dir/file2")).unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/kvpkpm97sf4ia351h4kf6rhykfgh0vr8-meow.drv\n")
        .cache_status(CacheStatus::Invalid);
}

#[test]
fn get_env() {
    let dir = tempdir().unwrap();
    let fixture = FixtureFactory::new(dir.path());

    fixture
        .create_fixtures([Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = builtins.getEnv "DRV_NAME"; system = builtins.currentSystem; builder = "/bin/sh"; }; }"#)])
        .unwrap();

    fluke_cmd(dir.path())
        .env("DRV_NAME", "meow")
        .assert()
        .success()
        .stdout(MEOW);

    fluke_cmd(dir.path())
        .env("DRV_NAME", "meow")
        .assert()
        .success()
        .stdout(MEOW)
        .cache_status(CacheStatus::Valid);

    fluke_cmd(dir.path())
        .env("DRV_NAME", "bark")
        .assert()
        .success()
        .stdout(BARK)
        .cache_status(CacheStatus::Invalid);

    // Unset environment variables get handled as expected.
    fixture
        .create_fixtures([Fixture::File(EXPR_PATH, r#"{ attr = builtins.derivation { name = "meow"; system = builtins.currentSystem; builder = "/bin/sh"; var = builtins.getEnv "VAR"; }; }"#)])
        .unwrap();

    fluke_cmd(dir.path())
        .assert()
        .success()
        .stdout("/nix/store/iqrhyyrvv570w70ahbk0lp45bb2b5lwc-meow.drv\n")
        .cache_status(CacheStatus::Invalid);

    fluke_cmd(dir.path())
        .env("VAR", "meowmeowmeow")
        .assert()
        .success()
        .stdout("/nix/store/nbzz2xdiz5w9v6bdcxlgcs15igyfz8xa-meow.drv\n")
        .cache_status(CacheStatus::Invalid);
}
