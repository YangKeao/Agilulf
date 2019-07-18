use super::log_manager::LogManager;
use super::sstable::SSTable;
use std::collections::BTreeMap;

pub struct ManifestManager {
    sstables: BTreeMap<usize, Vec<SSTable>>,
}

impl ManifestManager {}
