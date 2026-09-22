use aicore_storage::Storage;
use std::sync::Arc;

pub struct SessionManager {
    _storage: Arc<Storage>,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { _storage: storage }
    }
}
