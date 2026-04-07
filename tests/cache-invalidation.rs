#![allow(clippy::disallowed_types)]

use std::{env, fs, path::PathBuf};

use fluke::{
    AbsolutePath, RelativePath,
    cache::{Cache, CacheStatus, gen_cache},
};
use proptest::{prelude::*, test_runner::Config};
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};
use tempfile::{TempDir, tempdir};
use test_log::test;

use crate::util::{Fixture, FixtureFactory};

mod util;

#[derive(Clone, Debug)]
struct RefState {
    /// Whether any cache-invalidating actions have been performed since the last Check.
    invalidated: bool,
}

#[derive(Clone, Debug)]
enum Transition {
    Check,
    ModifyNix,
    ModifyFile,
    ModifyDir,
    ModifyEnvVar,
}

prop_state_machine! {
    #![proptest_config(Config {
        verbose: 1,
        ..Config::default()
    })]

    #[test]
    fn cache_invalidation(sequential 1..20 => Test);
}

impl ReferenceStateMachine for RefState {
    type State = RefState;
    type Transition = Transition;

    fn init_state() -> BoxedStrategy<Self::State> {
        Just(RefState { invalidated: false }).boxed()
    }

    fn transitions(_state: &Self::State) -> BoxedStrategy<Self::Transition> {
        use Transition::*;

        prop_oneof![
            2 => Just(Check),
            1 => Just(ModifyNix),
            3 => Just(ModifyFile),
            3 => Just(ModifyDir),
            3 => Just(ModifyEnvVar),
        ]
        .boxed()
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            Transition::Check => {}
            Transition::ModifyNix
            | Transition::ModifyFile
            | Transition::ModifyDir
            | Transition::ModifyEnvVar => state.invalidated = true,
        }

        state
    }
}

struct Test {
    dir: TempDir,
    f: FixtureFactory,
    nix_file_at_root: RelativePath,
    cache: Cache,
}

impl StateMachineTest for Test {
    type SystemUnderTest = Self;
    type Reference = RefState;

    fn init_test(
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) -> Self::SystemUnderTest {
        let dir = tempdir().expect("tempdir to be created");

        let nix_file = dir.path().join("expr.nix");
        let nix_file_at_root = RelativePath::new(PathBuf::from("expr.nix")).unwrap();
        fs::write(&nix_file, include_str!("expr.nix")).expect("expr to be written");

        let fixture_factory = FixtureFactory::new(dir.path());

        fixture_factory
            .create_fixtures([Fixture::File("meowmeow", "a"), Fixture::Directory("dir")])
            .expect("fixtures to be created successfully");

        let cache = gen_cache(
            &AbsolutePath::new(dir.path()).unwrap(),
            &nix_file_at_root,
            "foo",
            None,
        )
        .expect("first cache to be generated successfully");

        Test {
            dir,
            f: fixture_factory,
            nix_file_at_root,
            cache,
        }
    }

    fn apply(
        state: Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        transition: <Self::Reference as ReferenceStateMachine>::Transition,
    ) -> Self::SystemUnderTest {
        match transition {
            Transition::Check => {
                let status = state
                    .cache
                    .status(
                        &AbsolutePath::new(state.dir.path()).unwrap(),
                        &state.nix_file_at_root,
                        "foo",
                    )
                    .expect("cache status to be calculated successfully");

                if ref_state.invalidated {
                    assert!(
                        matches!(status, CacheStatus::Invalid(Some(_))),
                        "cache should be invalid\ncache: {:?}",
                        state.cache
                    );
                } else {
                    assert!(
                        matches!(status, CacheStatus::Valid),
                        "cache should be valid\ncache: {:?}",
                        state.cache
                    );
                }
            }
            Transition::ModifyNix => {
                state
                    .f
                    .create_fixtures([Fixture::Append(
                        &state.nix_file_at_root.rel(),
                        "\n# comment\n",
                    )])
                    .expect("file to be modified");
            }
            Transition::ModifyFile => {
                state
                    .f
                    .create_fixtures([Fixture::Append("meowmeow", "\n# comment\n")])
                    .expect("file to be modified");
            }
            Transition::ModifyDir => {
                let path = state.dir.path().join("dir/foo");
                if path.exists() {
                    fs::remove_file(path).expect("for the file to be deleted");
                } else {
                    state
                        .f
                        .create_fixtures([Fixture::File(path, "")])
                        .expect("file to be created")
                }
            }
            Transition::ModifyEnvVar => {
                let v = if let Ok("meow") = env::var("MEOWMEOW").as_deref() {
                    "bark"
                } else {
                    "meow"
                };

                // SAFETY: We are running within one single-threaded process.
                unsafe {
                    env::set_var("MEOWMEOW", v);
                }
            }
        }

        state
    }

    fn teardown(
        _state: Self::SystemUnderTest,
        _ref_state: <Self::Reference as ReferenceStateMachine>::State,
    ) {
        // SAFETY: We are running within one single-threaded process.
        unsafe {
            env::remove_var("MEOWMEOW");
        }
    }
}
