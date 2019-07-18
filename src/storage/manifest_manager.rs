use super::database_log::DatabaseLog;
use super::sstable::SSTable;
use crate::log::{JudgeReal, LogManager};
use memmap::{MmapMut, MmapOptions};
use std::collections::BTreeMap;

pub struct SSTableId {
    level: usize,
    id: usize,
}

pub enum ManifestLogEntry {
    ADD(SSTableId),
    RemoveEntry(SSTableId),
}

#[repr(packed)]
#[derive(Clone)]
pub struct RawManifestLogEntry {
    real_flag: u8,
    add_flag: u8,
    level: u8,
    id: u8,
}

impl JudgeReal for RawManifestLogEntry {
    fn is_real(&self) -> bool {
        self.real_flag == 1
    }

    fn set_real(&mut self, real: bool) {
        if real {
            self.real_flag = 1;
        } else {
            self.real_flag = 0;
        }
    }
}

pub struct ManifestLog {
    log_manager: LogManager<RawManifestLogEntry>,
}

impl ManifestLog {}

pub struct ManifestManager {
    sstables: BTreeMap<usize, BTreeMap<usize, SSTable>>,
}

impl ManifestManager {}
