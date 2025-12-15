use async_trait::async_trait;
use raur::{Package, Raur, SearchBy};

/// a stub implementation of the raur trait
/// it does absolutely nothing, lookups for packages are always empty
pub struct StubAur;

#[async_trait]
impl Raur for StubAur {
    type Err = raur::Error;

    async fn search_by<S: AsRef<str> + Send + Sync>(
        &self,
        _query: S,
        _by: SearchBy,
    ) -> Result<Vec<Package>, Self::Err> {
        Ok(vec![])
    }

    async fn raw_info<S: AsRef<str> + Send + Sync>(
        &self,
        _pkg_names: &[S],
    ) -> Result<Vec<Package>, Self::Err> {
        Ok(vec![])
    }
}
