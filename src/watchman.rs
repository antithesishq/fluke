use eyre::{Context, Result};
use watchman_client::prelude::*;

use crate::{AbsolutePath, RelativePath};

async fn get_client_and_resolve_root(root: &AbsolutePath) -> Result<(Client, ResolvedRoot)> {
    let client = Connector::new()
        .connect()
        .await
        .wrap_err("couldn't connect to watchman")?;

    let resolved = client
        .resolve_root(CanonicalPath::with_canonicalized_path(
            root.abs().to_path_buf(),
        ))
        .await
        .wrap_err("couldn't resolve root")?;

    Ok((client, resolved))
}

#[derive(Debug)]
pub struct WatchmanRes {
    pub files: Vec<RelativePath>,
    pub clock: Clock,
    pub is_fresh_instance: bool,
}

#[tokio::main(flavor = "current_thread")]
pub async fn query_watchman(
    root: &AbsolutePath,
    since: watchman_client::pdu::Clock,
) -> Result<WatchmanRes> {
    let (client, resolved) = get_client_and_resolve_root(root).await?;

    let res: QueryResult<NameOnly> = client
        .query(
            &resolved,
            QueryRequestCommon {
                since: Some(since),
                ..QueryRequestCommon::default()
            },
        )
        .await
        .wrap_err("couldn't query watchman")?;

    let files = res
        .files
        .map(|mut f| {
            f.drain(..)
                .map(|p| RelativePath::new(&*p.name))
                .collect::<Result<_>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(WatchmanRes {
        files,
        clock: res.clock,
        is_fresh_instance: res.is_fresh_instance,
    })
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_current_clock(root: &AbsolutePath) -> Result<ClockSpec> {
    let (client, resolved) = get_client_and_resolve_root(root).await?;

    Ok(client.clock(&resolved, SyncTimeout::Default).await?)
}
