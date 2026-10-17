use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub(crate) struct TabId(pub(super) u32);

impl TabId {
    pub(crate) const FIRST: TabId = TabId(1);

    pub(crate) fn next(self) -> TabId {
        TabId(self.0 + 1)
    }
}
