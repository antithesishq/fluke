use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    process::Command,
};

use clap::{CommandFactory, Parser};
use eyre::{Context, OptionExt, Result};
use sha1::{Digest, Sha1};

use fluke::{
    AbsolutePath, RelativePath,
    cache::{Cache, CacheStatus, gen_cache},
};
use tracing::{debug, trace};
use tracing_subscriber::EnvFilter;

/// Nix evaluation caching for the rest of us
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The root of your project. Usually, the default (inferred from your source control repository) is sufficient.
    #[arg(long, default_value = default_root())]
    root: AbsolutePath,
    /// The file within your project to evaluate. This must return an attribute set of derivations.
    file: AbsolutePath,
    /// The attribute to evaluate.
    attr: String,
}

fn default_root() -> String {
    for (cmd, args) in [
        ("git", &["rev-parse", "--show-toplevel"][..]),
        ("jj", &["root"]),
        ("hg", &["root"]),
    ] {
        if let Ok(res) = Command::new(cmd).args(args).output() {
            if !res.status.success() {
                continue;
            }

            return String::from_utf8(res.stdout).unwrap().trim().to_string();
        }
    }

    env::current_dir().unwrap().to_str().unwrap().to_string()
}

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .pretty()
        .init();

    let xdg_dirs = xdg::BaseDirectories::with_prefix("fluke");
    let cache_dir = xdg_dirs
        .get_cache_home()
        .ok_or_eyre("couldn't find where to place cache")?;
    fs::create_dir_all(&cache_dir)?;

    let args = Cli::parse();

    let nix_file = match args.file.abs().strip_prefix(args.root.abs()) {
        Ok(rel) => RelativePath::new(rel)?,
        Err(_) => Cli::command()
            .error(
                clap::error::ErrorKind::ValueValidation,
                format!(
                    "{:?} must be a descendant of {:?}",
                    args.file.abs(),
                    args.root.abs()
                ),
            )
            .exit(),
    };

    let cache_file = cache_dir.join({
        let mut h = Sha1::new();
        h.update(args.root.abs().as_os_str().as_encoded_bytes());
        h.update(nix_file.rel().as_os_str().as_encoded_bytes());
        h.update(&args.attr);
        hex::encode(h.finalize())
    });

    // TODO: remove this explicit type
    let (cache, _flock): (Option<Cache>, _) = {
        let mut f = File::options()
            .write(true)
            .read(true)
            .create(true)
            .truncate(false)
            .open(&cache_file)?;

        trace!(file = ?cache_file, "waiting for flock");
        f.lock().wrap_err("couldn't get flock")?;
        trace!(file = ?cache_file, "got flock");

        let metadata = f.metadata()?;

        if metadata.len() == 0 {
            (None, f)
        } else {
            let mut buf = Vec::with_capacity(metadata.len() as usize);
            f.read_to_end(&mut buf)?;

            (
                Some(serde_json::from_slice(&buf).wrap_err("couldn't parse cache file")?),
                f,
            )
        }
    };

    // debug!(?cache, path = ?cache_file, "read cache");

    let new_cache = match cache {
        Some(cache) => {
            let status = cache.status(&args.root, &nix_file, &args.attr)?;

            debug!(?status);

            match status {
                CacheStatus::Valid => cache,
                CacheStatus::Invalid(clock) => gen_cache(&args.root, &nix_file, &args.attr, clock)?,
            }
        }
        None => gen_cache(&args.root, &nix_file, &args.attr, None)?,
    };

    fs::write(
        cache_file,
        serde_json::to_vec(&new_cache).wrap_err("couldn't serialize cache")?,
    )?;

    println!("{}", new_cache.drv);

    Ok(())
}
