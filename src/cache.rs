use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{AbsolutePath, RelativePath, eval, impurity::Impurity, watchman};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Cache {
    pub drv: String,
    impurities: Vec<Impurity>,
    clock: watchman_client::pdu::Clock,
}

#[tracing::instrument]
pub fn gen_cache(
    root: &AbsolutePath,
    nix_file: &RelativePath,
    attr: &str,
    clock: Option<watchman_client::pdu::Clock>,
) -> Result<Cache> {
    let (drv, impurities) = eval(
        &AbsolutePath::new(root.abs().join(nix_file.rel()))?,
        attr,
        false,
    )
    .wrap_err("couldn't eval")?;

    Ok(Cache {
        drv,
        impurities: impurities
            .into_iter()
            .filter(|p| p.path().is_none_or(|p| p.abs().starts_with(root.abs())))
            .collect(),
        clock: clock.map(Result::Ok).unwrap_or_else(|| {
            Ok::<_, eyre::Error>(watchman_client::pdu::Clock::Spec(
                watchman::get_current_clock(root).wrap_err("couldn't get current clock")?,
            ))
        })?,
    })
}

#[derive(Debug)]
pub enum CacheStatus {
    Valid,
    Invalid(Option<watchman_client::pdu::Clock>),
}

impl Cache {
    #[tracing::instrument(skip(self))]
    pub fn status(
        &self,
        root: &AbsolutePath,
        nix_file: &RelativePath,
        attr: &str,
    ) -> Result<CacheStatus> {
        let watchman_res = watchman::query_watchman(root, self.clock.clone())?;

        debug!(?watchman_res);

        if watchman_res.is_fresh_instance {
            debug!("fresh instance, invalidating cache");
            return Ok(CacheStatus::Invalid(None));
        }

        if watchman_res.files.is_empty() && self.impurities.iter().all(|i| i.path().is_some()) {
            return Ok(CacheStatus::Valid);
        }

        if watchman_res.files.iter().any(|e| e.rel() == nix_file.rel()) {
            debug!(path = ?nix_file, "nix file changed");
            return Ok(CacheStatus::Invalid(Some(watchman_res.clock)));
        }

        match self
            .impurities
            .iter()
            .find(|i| i.has_changed(&watchman_res.files, root))
        {
            Some(impurity) => {
                debug!(?impurity, "invalidating cache");
                Ok(CacheStatus::Invalid(Some(watchman_res.clock)))
            }
            None => Ok(CacheStatus::Valid),
        }
    }
}
