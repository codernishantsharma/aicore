use aicore_storage::Storage;
use std::sync::Arc;

pub struct PermissionManager {
    _storage: Arc<Storage>,
}

impl PermissionManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { _storage: storage }
    }
}
