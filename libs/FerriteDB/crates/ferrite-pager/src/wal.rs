use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use ferrite_vfs::{FileHandle, LockMode, OpenOptions, Vfs, VfsError};
use thiserror::Error;

use crate::{PAGE_SIZE, Page, PageNumber};

const WAL_MAGIC: &[u8; 8] = b"FDBWAL01";
const WAL_VERSION: u32 = 1;
const WAL_HEADER_SIZE: u64 = 32;
const FRAME_HEADER_SIZE: u64 = 24;
const FRAME_SIZE: u64 = FRAME_HEADER_SIZE + PAGE_SIZE as u64;
const SECOND_CHECKSUM_SEED: u32 = 0xa5a5_5a5a;

const fn crc32c_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut checksum = index as u32;
        let mut bit = 0;
        while bit < 8 {
            let mask = 0_u32.wrapping_sub(checksum & 1);
            checksum = (checksum >> 1) ^ (0x82f6_3b78 & mask);
            bit += 1;
        }
        table[index] = checksum;
        index += 1;
    }
    table
}

const CRC32C_TABLE: [u32; 256] = crc32c_table();

#[derive(Debug, Error)]
pub enum WalError {
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),
    #[error("WAL header is invalid: {0}")]
    InvalidHeader(&'static str),
    #[error("WAL frame is corrupt: {0}")]
    CorruptFrame(&'static str),
    #[error("WAL checksum mismatch")]
    ChecksumMismatch,
    #[error("WAL file ended before the requested bytes were read")]
    UnexpectedEof,
    #[error("WAL write made no progress")]
    WriteZero,
    #[error("WAL offset exceeds the supported range")]
    OffsetOverflow,
    #[error("WAL memory allocation failed")]
    Allocation,
    #[error("a WAL commit must contain at least one page")]
    EmptyCommit,
    #[error("a WAL transaction contains the same page more than once")]
    DuplicatePage,
    #[error("WAL commit page count must include page 1")]
    InvalidPageCount,
    #[error("WAL snapshot does not belong to this log or is not committed")]
    InvalidSnapshot,
    #[error("WAL is in an uncertain state after an I/O failure; reopen it before use")]
    NeedsRecovery,
    #[error("writer snapshot is stale; reopen the pager before retrying the write")]
    StaleWriterSnapshot,
    #[error("WAL commit is durable, but releasing its writer lock failed: {0}")]
    CommitDurable(String),
    #[error("WAL commit status is uncertain; reopen before deciding whether it committed: {0}")]
    CommitUncertain(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Checksums {
    first: u32,
    second: u32,
}

#[derive(Clone, Copy, Debug)]
struct WalHeader {
    salt1: u32,
    salt2: u32,
    checksums: Checksums,
}

impl WalHeader {
    fn new(vfs: &dyn Vfs) -> Result<Self, WalError> {
        let mut salt = [0; 8];
        vfs.fill_random(&mut salt)?;
        let salt1 = u32::from_be_bytes([salt[0], salt[1], salt[2], salt[3]]);
        let salt2 = u32::from_be_bytes([salt[4], salt[5], salt[6], salt[7]]);
        let mut header = Self {
            salt1,
            salt2,
            checksums: Checksums::default(),
        };
        let encoded = header.encode();
        header.checksums =
            Checksums {
                first: u32::from_be_bytes(encoded[24..28].try_into().map_err(|_| {
                    WalError::InvalidHeader("checksum field has an invalid length")
                })?),
                second: u32::from_be_bytes(encoded[28..32].try_into().map_err(|_| {
                    WalError::InvalidHeader("checksum field has an invalid length")
                })?),
            };
        Ok(header)
    }

    fn encode(self) -> [u8; WAL_HEADER_SIZE as usize] {
        let mut bytes = [0; WAL_HEADER_SIZE as usize];
        bytes[..8].copy_from_slice(WAL_MAGIC);
        bytes[8..12].copy_from_slice(&WAL_VERSION.to_be_bytes());
        bytes[12..16].copy_from_slice(&(PAGE_SIZE as u32).to_be_bytes());
        bytes[16..20].copy_from_slice(&self.salt1.to_be_bytes());
        bytes[20..24].copy_from_slice(&self.salt2.to_be_bytes());
        let checksums = header_checksums(&bytes[..24]);
        bytes[24..28].copy_from_slice(&checksums.first.to_be_bytes());
        bytes[28..32].copy_from_slice(&checksums.second.to_be_bytes());
        bytes
    }

    fn decode(bytes: &[u8; WAL_HEADER_SIZE as usize]) -> Result<Self, WalError> {
        if &bytes[..8] != WAL_MAGIC {
            return Err(WalError::InvalidHeader("magic does not match"));
        }
        let version = read_u32(bytes, 8);
        if version != WAL_VERSION {
            return Err(WalError::InvalidHeader("unsupported version"));
        }
        if read_u32(bytes, 12) != PAGE_SIZE as u32 {
            return Err(WalError::InvalidHeader("page size does not match"));
        }
        let expected = header_checksums(&bytes[..24]);
        let actual = Checksums {
            first: read_u32(bytes, 24),
            second: read_u32(bytes, 28),
        };
        if actual != expected {
            return Err(WalError::ChecksumMismatch);
        }
        Ok(Self {
            salt1: read_u32(bytes, 16),
            salt2: read_u32(bytes, 20),
            checksums: actual,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalSnapshot {
    end_frame: u64,
    database_size: u32,
    salt1: u32,
    salt2: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WalStats {
    pub bytes: u64,
    pub committed_frames: u64,
}

impl WalSnapshot {
    pub const fn end_frame(self) -> u64 {
        self.end_frame
    }

    pub const fn database_size(self) -> u32 {
        self.database_size
    }

    pub(crate) const fn same_log(self, other: Self) -> bool {
        self.salt1 == other.salt1 && self.salt2 == other.salt2
    }
}

#[derive(Clone, Copy)]
struct FrameIndex {
    page_number: PageNumber,
    frame_number: u64,
    offset: u64,
}

pub struct Wal {
    file: Box<dyn FileHandle>,
    header: WalHeader,
    frames: Vec<FrameIndex>,
    index: HashMap<PageNumber, Vec<FrameIndex>>,
    commit_sizes: Vec<(u64, u32)>,
    committed_frame: u64,
    database_size: u32,
    checksums: Checksums,
    needs_recovery: bool,
}

impl Wal {
    pub fn path_for_database(database_path: &Path) -> PathBuf {
        let mut path = OsString::from(database_path.as_os_str());
        path.push("-wal");
        PathBuf::from(path)
    }

    pub fn open_for_database(vfs: &dyn Vfs, database_path: &Path) -> Result<Self, WalError> {
        Self::open(vfs, &Self::path_for_database(database_path))
    }

    pub fn create_for_database(vfs: &dyn Vfs, database_path: &Path) -> Result<Self, WalError> {
        Self::create(vfs, &Self::path_for_database(database_path))
    }

    pub fn create(vfs: &dyn Vfs, wal_path: &Path) -> Result<Self, WalError> {
        let mut file = vfs.open(
            wal_path,
            OpenOptions {
                read: true,
                write: true,
                create: true,
                truncate: true,
            },
        )?;
        let header = WalHeader::new(vfs)?;
        write_all_at(&mut *file, 0, &header.encode())?;
        file.sync_all()?;
        Ok(Self {
            file,
            header,
            frames: Vec::new(),
            index: HashMap::new(),
            commit_sizes: Vec::new(),
            committed_frame: 0,
            database_size: 0,
            checksums: header.checksums,
            needs_recovery: false,
        })
    }

    pub fn open(vfs: &dyn Vfs, wal_path: &Path) -> Result<Self, WalError> {
        let mut file = vfs.open(
            wal_path,
            OpenOptions {
                read: true,
                write: true,
                create: true,
                truncate: false,
            },
        )?;
        file.lock(LockMode::Exclusive)?;
        let initial_file_length = file.len()?;
        let header = if initial_file_length < WAL_HEADER_SIZE {
            file.set_len(0)?;
            let header = WalHeader::new(vfs)?;
            write_all_at(&mut *file, 0, &header.encode())?;
            file.sync_all()?;
            header
        } else {
            let mut encoded = [0; WAL_HEADER_SIZE as usize];
            read_exact_at(&mut *file, 0, &mut encoded)?;
            WalHeader::decode(&encoded)?
        };
        let file_length = file.len()?;
        let mut wal = Self {
            file,
            header,
            frames: Vec::new(),
            index: HashMap::new(),
            commit_sizes: Vec::new(),
            committed_frame: 0,
            database_size: 0,
            checksums: header.checksums,
            needs_recovery: false,
        };
        wal.recover_index(file_length)?;
        wal.file.unlock()?;
        Ok(wal)
    }

    pub fn frame_count(&self) -> u64 {
        self.committed_frame
    }

    pub fn stats(&mut self) -> Result<WalStats, WalError> {
        self.ensure_usable()?;
        self.with_writer_lock(false, |wal| {
            wal.reload_index()?;
            Ok(WalStats {
                bytes: wal.file.len()?,
                committed_frames: wal.committed_frame,
            })
        })
    }

    pub fn snapshot(&self, base_database_size: u32) -> Result<WalSnapshot, WalError> {
        if self.needs_recovery {
            return Err(WalError::NeedsRecovery);
        }
        let database_size = if self.committed_frame == 0 {
            base_database_size
        } else {
            self.database_size
        };
        if database_size == 0 {
            return Err(WalError::InvalidPageCount);
        }
        Ok(WalSnapshot {
            end_frame: self.committed_frame,
            database_size,
            salt1: self.header.salt1,
            salt2: self.header.salt2,
        })
    }

    pub fn read_page(
        &mut self,
        page_number: PageNumber,
        snapshot: WalSnapshot,
    ) -> Result<Option<Page>, WalError> {
        self.ensure_usable()?;
        if snapshot.salt1 != self.header.salt1
            || snapshot.salt2 != self.header.salt2
            || snapshot.end_frame > self.committed_frame
            || page_number.get() > snapshot.database_size
        {
            return Err(WalError::InvalidSnapshot);
        }
        let Some(versions) = self.index.get(&page_number) else {
            return Ok(None);
        };
        let index = versions.partition_point(|frame| frame.frame_number <= snapshot.end_frame);
        let Some(frame) = index.checked_sub(1).and_then(|index| versions.get(index)) else {
            return Ok(None);
        };
        let data_offset = frame
            .offset
            .checked_add(FRAME_HEADER_SIZE)
            .ok_or(WalError::OffsetOverflow)?;
        let mut page = Page::zeroed();
        read_exact_at(&mut *self.file, data_offset, page.as_mut_bytes())?;
        Ok(Some(page))
    }

    pub fn commit_pages(
        &mut self,
        pages: &[(PageNumber, &Page)],
        database_size: u32,
    ) -> Result<WalSnapshot, WalError> {
        self.ensure_usable()?;
        self.with_writer_lock(true, |wal| {
            wal.reload_index()?;
            wal.commit_pages_locked(pages, database_size)
        })
    }

    pub(crate) fn commit_pages_from_snapshot(
        &mut self,
        expected_snapshot: WalSnapshot,
        pages: &[(PageNumber, &Page)],
        database_size: u32,
    ) -> Result<WalSnapshot, WalError> {
        self.ensure_usable()?;
        self.with_writer_lock(true, |wal| {
            wal.reload_index()?;
            if wal.committed_frame != expected_snapshot.end_frame()
                || !wal.snapshot(database_size)?.same_log(expected_snapshot)
            {
                return Err(WalError::StaleWriterSnapshot);
            }
            wal.commit_pages_locked(pages, database_size)
        })
    }

    fn commit_pages_locked(
        &mut self,
        pages: &[(PageNumber, &Page)],
        database_size: u32,
    ) -> Result<WalSnapshot, WalError> {
        self.ensure_usable()?;
        if pages.is_empty() {
            return Err(WalError::EmptyCommit);
        }
        if database_size == 0 || pages.iter().any(|(page, _)| page.get() > database_size) {
            return Err(WalError::InvalidPageCount);
        }
        let mut unique_pages = HashSet::new();
        unique_pages
            .try_reserve(pages.len())
            .map_err(|_| WalError::Allocation)?;
        for (page_number, _) in pages {
            if !unique_pages.insert(*page_number) {
                return Err(WalError::DuplicatePage);
            }
        }
        self.frames
            .try_reserve(pages.len())
            .map_err(|_| WalError::Allocation)?;
        self.commit_sizes
            .try_reserve(1)
            .map_err(|_| WalError::Allocation)?;
        self.index
            .try_reserve(pages.len())
            .map_err(|_| WalError::Allocation)?;
        for page_number in &unique_pages {
            self.index
                .entry(*page_number)
                .or_default()
                .try_reserve(1)
                .map_err(|_| WalError::Allocation)?;
        }
        let mut appended = Vec::new();
        appended
            .try_reserve_exact(pages.len())
            .map_err(|_| WalError::Allocation)?;

        let committed_length = WAL_HEADER_SIZE
            .checked_add(
                self.committed_frame
                    .checked_mul(FRAME_SIZE)
                    .ok_or(WalError::OffsetOverflow)?,
            )
            .ok_or(WalError::OffsetOverflow)?;
        self.file.set_len(committed_length)?;
        let mut checksums = self.checksums;
        let mut frame_number = self.committed_frame;
        self.needs_recovery = true;
        let append_result = (|| {
            for (index, (page_number, page)) in pages.iter().enumerate() {
                frame_number = frame_number
                    .checked_add(1)
                    .ok_or(WalError::OffsetOverflow)?;
                let database_size_marker = if index + 1 == pages.len() {
                    database_size
                } else {
                    0
                };
                let first_eight = frame_prefix(page_number.get(), database_size_marker);
                checksums = frame_checksums(checksums, &first_eight, page.as_bytes());
                let header = encode_frame_header(
                    first_eight,
                    self.header.salt1,
                    self.header.salt2,
                    checksums,
                );
                let offset = frame_offset(frame_number)?;
                write_all_at(&mut *self.file, offset, &header)?;
                write_all_at(
                    &mut *self.file,
                    offset
                        .checked_add(FRAME_HEADER_SIZE)
                        .ok_or(WalError::OffsetOverflow)?,
                    page.as_bytes(),
                )?;
                appended.push(FrameIndex {
                    page_number: *page_number,
                    frame_number,
                    offset,
                });
            }
            self.file.sync_all()?;
            Ok::<(), WalError>(())
        })();
        if let Err(error) = append_result {
            return Err(WalError::CommitUncertain(error.to_string()));
        }

        for frame in appended {
            self.index.entry(frame.page_number).or_default().push(frame);
            self.frames.push(frame);
        }
        self.committed_frame = frame_number;
        self.database_size = database_size;
        self.checksums = checksums;
        self.commit_sizes.push((frame_number, database_size));
        self.needs_recovery = false;
        self.snapshot(database_size)
    }

    pub fn checkpoint(
        &mut self,
        database_file: &mut dyn FileHandle,
        vfs: &dyn Vfs,
    ) -> Result<(), WalError> {
        self.ensure_usable()?;
        self.with_writer_lock(false, |wal| {
            wal.reload_index()?;
            let committed_frame = wal.committed_frame;
            let _ = wal.checkpoint_through_locked(database_file, vfs, committed_frame, true)?;
            Ok(())
        })
    }

    pub(crate) fn backup_to(
        &mut self,
        database_file: &mut dyn FileHandle,
        vfs: &dyn Vfs,
        backup_database_path: &Path,
    ) -> Result<(), WalError> {
        self.ensure_usable()?;
        self.with_writer_lock(false, |wal| {
            wal.reload_index()?;
            let backup_wal_path = Self::path_for_database(backup_database_path);
            let mut backup_database = vfs.open(
                backup_database_path,
                OpenOptions {
                    read: false,
                    write: true,
                    create: true,
                    truncate: true,
                },
            )?;
            let mut backup_wal = vfs.open(
                &backup_wal_path,
                OpenOptions {
                    read: false,
                    write: true,
                    create: true,
                    truncate: true,
                },
            )?;
            copy_file_contents(database_file, &mut *backup_database)?;
            copy_file_contents(&mut *wal.file, &mut *backup_wal)?;
            Ok(())
        })
    }

    pub(crate) fn with_checkpoint_lock<R, E>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<WalError>,
    {
        self.ensure_usable().map_err(E::from)?;
        self.file
            .lock(LockMode::Exclusive)
            .map_err(WalError::from)
            .map_err(E::from)?;
        let result = (|| {
            self.reload_index().map_err(E::from)?;
            operation(self)
        })();
        let unlock_result = self.file.unlock().map_err(WalError::from).map_err(E::from);
        match result {
            Err(error) => Err(error),
            Ok(value) => unlock_result.map(|()| value),
        }
    }

    pub(crate) fn checkpoint_through_locked(
        &mut self,
        database_file: &mut dyn FileHandle,
        vfs: &dyn Vfs,
        max_frame: u64,
        restart_when_complete: bool,
    ) -> Result<Option<WalSnapshot>, WalError> {
        self.ensure_usable()?;
        if max_frame > self.committed_frame {
            return Err(WalError::InvalidSnapshot);
        }
        if max_frame == 0 {
            return Ok(None);
        }
        let database_size = self
            .commit_sizes
            .iter()
            .rev()
            .find_map(|(frame, size)| (*frame <= max_frame).then_some(*size))
            .ok_or(WalError::InvalidSnapshot)?;
        let database_length = u64::from(database_size)
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(WalError::OffsetOverflow)?;
        let mut latest = Vec::new();
        latest
            .try_reserve_exact(self.index.len())
            .map_err(|_| WalError::Allocation)?;
        for versions in self.index.values() {
            let visible = versions.partition_point(|frame| frame.frame_number <= max_frame);
            if let Some(frame) = visible.checked_sub(1).and_then(|index| versions.get(index)) {
                latest.push(*frame);
            }
        }
        latest.sort_unstable_by_key(|frame| frame.page_number.get());

        let target_length = if restart_when_complete && max_frame == self.committed_frame {
            database_length
        } else {
            database_file.len()?.max(database_length)
        };
        database_file.set_len(target_length)?;
        for frame in latest {
            let mut page = Page::zeroed();
            read_exact_at(
                &mut *self.file,
                frame
                    .offset
                    .checked_add(FRAME_HEADER_SIZE)
                    .ok_or(WalError::OffsetOverflow)?,
                page.as_mut_bytes(),
            )?;
            let database_offset = u64::from(frame.page_number.get() - 1)
                .checked_mul(PAGE_SIZE as u64)
                .ok_or(WalError::OffsetOverflow)?;
            write_all_at(database_file, database_offset, page.as_bytes())?;
        }
        database_file.sync_all()?;

        if !restart_when_complete || max_frame != self.committed_frame {
            return Ok(None);
        }

        self.needs_recovery = true;
        self.file.set_len(0)?;
        let header = WalHeader::new(vfs)?;
        write_all_at(&mut *self.file, 0, &header.encode())?;
        self.file.sync_all()?;
        self.header = header;
        self.frames.clear();
        self.index.clear();
        self.commit_sizes.clear();
        self.committed_frame = 0;
        self.database_size = database_size;
        self.checksums = header.checksums;
        self.needs_recovery = false;
        Ok(Some(self.snapshot(database_size)?))
    }

    fn with_writer_lock<R>(
        &mut self,
        commit_durable_on_unlock_error: bool,
        operation: impl FnOnce(&mut Self) -> Result<R, WalError>,
    ) -> Result<R, WalError> {
        self.file.lock(LockMode::Exclusive)?;
        let result = operation(self);
        let unlock_result = self.file.unlock();
        match result {
            Err(error) => Err(error),
            Ok(value) => match unlock_result {
                Ok(()) => Ok(value),
                Err(error) if commit_durable_on_unlock_error => {
                    Err(WalError::CommitDurable(error.to_string()))
                }
                Err(error) => Err(WalError::Vfs(error)),
            },
        }
    }

    fn reload_index(&mut self) -> Result<(), WalError> {
        let file_length = self.file.len()?;
        if file_length < WAL_HEADER_SIZE {
            return Err(WalError::InvalidHeader("header is truncated"));
        }
        let mut encoded = [0; WAL_HEADER_SIZE as usize];
        read_exact_at(&mut *self.file, 0, &mut encoded)?;
        self.header = WalHeader::decode(&encoded)?;
        self.frames.clear();
        self.index.clear();
        self.commit_sizes.clear();
        self.committed_frame = 0;
        self.database_size = 0;
        self.checksums = self.header.checksums;
        self.needs_recovery = false;
        self.recover_index(file_length)
    }

    fn recover_index(&mut self, file_length: u64) -> Result<(), WalError> {
        if file_length < WAL_HEADER_SIZE {
            return Err(WalError::InvalidHeader("header is truncated"));
        }
        let complete_frames = (file_length - WAL_HEADER_SIZE) / FRAME_SIZE;
        let capacity = usize::try_from(complete_frames).map_err(|_| WalError::Allocation)?;
        self.frames
            .try_reserve_exact(capacity)
            .map_err(|_| WalError::Allocation)?;
        self.commit_sizes
            .try_reserve_exact(capacity)
            .map_err(|_| WalError::Allocation)?;
        let mut checksums = self.header.checksums;
        let mut committed_count = 0_usize;
        let mut committed_checksums = checksums;
        let mut committed_frame = 0_u64;
        let mut committed_size = 0_u32;
        let mut uncommitted_start = 0_usize;

        for frame_index in 0..complete_frames {
            let frame_number = frame_index.checked_add(1).ok_or(WalError::OffsetOverflow)?;
            let offset = frame_offset(frame_number)?;
            let mut frame = [0; FRAME_SIZE as usize];
            read_exact_at(&mut *self.file, offset, &mut frame)?;
            let frame_header = &frame[..FRAME_HEADER_SIZE as usize];
            let page = &frame[FRAME_HEADER_SIZE as usize..];
            let page_number = PageNumber::new(read_u32(frame_header, 0))
                .map_err(|_| WalError::CorruptFrame("page number is zero"))?;
            let database_size = read_u32(frame_header, 4);
            if read_u32(frame_header, 8) != self.header.salt1
                || read_u32(frame_header, 12) != self.header.salt2
            {
                return Err(WalError::CorruptFrame("salt does not match the WAL header"));
            }
            let first_eight = frame_prefix(page_number.get(), database_size);
            let computed = frame_checksums(checksums, &first_eight, page);
            let stored = Checksums {
                first: read_u32(frame_header, 16),
                second: read_u32(frame_header, 20),
            };
            if computed != stored {
                return Err(WalError::ChecksumMismatch);
            }
            checksums = computed;
            self.frames.push(FrameIndex {
                page_number,
                frame_number,
                offset,
            });
            if database_size != 0 {
                if self.frames[uncommitted_start..]
                    .iter()
                    .any(|frame| frame.page_number.get() > database_size)
                {
                    return Err(WalError::CorruptFrame(
                        "commit database size excludes a transaction page",
                    ));
                }
                committed_count = self.frames.len();
                committed_frame = frame_number;
                committed_size = database_size;
                committed_checksums = checksums;
                self.commit_sizes.push((frame_number, database_size));
                uncommitted_start = committed_count;
            }
        }
        self.frames.truncate(committed_count);
        self.committed_frame = committed_frame;
        self.database_size = committed_size;
        self.checksums = committed_checksums;
        self.rebuild_index()?;
        Ok(())
    }

    fn rebuild_index(&mut self) -> Result<(), WalError> {
        self.index
            .try_reserve(self.frames.len())
            .map_err(|_| WalError::Allocation)?;
        for frame in &self.frames {
            self.index
                .entry(frame.page_number)
                .or_default()
                .try_reserve(1)
                .map_err(|_| WalError::Allocation)?;
        }
        for frame in &self.frames {
            self.index
                .entry(frame.page_number)
                .or_default()
                .push(*frame);
        }
        Ok(())
    }

    fn ensure_usable(&self) -> Result<(), WalError> {
        if self.needs_recovery {
            Err(WalError::NeedsRecovery)
        } else {
            Ok(())
        }
    }
}

fn header_checksums(bytes: &[u8]) -> Checksums {
    Checksums {
        first: crc32c_append(0, bytes),
        second: crc32c_append(SECOND_CHECKSUM_SEED, bytes),
    }
}

fn frame_checksums(previous: Checksums, header: &[u8; 8], page: &[u8]) -> Checksums {
    let mut first = !previous.first;
    let mut second = !previous.second;
    for byte in header.iter().chain(page) {
        first = (first >> 8) ^ CRC32C_TABLE[((first as u8) ^ *byte) as usize];
        second = (second >> 8) ^ CRC32C_TABLE[((second as u8) ^ *byte) as usize];
    }
    Checksums {
        first: !first,
        second: !second,
    }
}

fn crc32c_append(previous: u32, bytes: &[u8]) -> u32 {
    let mut checksum = !previous;
    for byte in bytes {
        checksum = (checksum >> 8) ^ CRC32C_TABLE[((checksum as u8) ^ *byte) as usize];
    }
    !checksum
}

fn frame_prefix(page_number: u32, database_size: u32) -> [u8; 8] {
    let mut prefix = [0; 8];
    prefix[..4].copy_from_slice(&page_number.to_be_bytes());
    prefix[4..].copy_from_slice(&database_size.to_be_bytes());
    prefix
}

fn encode_frame_header(
    first_eight: [u8; 8],
    salt1: u32,
    salt2: u32,
    checksums: Checksums,
) -> [u8; FRAME_HEADER_SIZE as usize] {
    let mut header = [0; FRAME_HEADER_SIZE as usize];
    header[..8].copy_from_slice(&first_eight);
    header[8..12].copy_from_slice(&salt1.to_be_bytes());
    header[12..16].copy_from_slice(&salt2.to_be_bytes());
    header[16..20].copy_from_slice(&checksums.first.to_be_bytes());
    header[20..24].copy_from_slice(&checksums.second.to_be_bytes());
    header
}

fn frame_offset(frame_number: u64) -> Result<u64, WalError> {
    if frame_number == 0 {
        return Err(WalError::OffsetOverflow);
    }
    WAL_HEADER_SIZE
        .checked_add(
            frame_number
                .checked_sub(1)
                .and_then(|frame| frame.checked_mul(FRAME_SIZE))
                .ok_or(WalError::OffsetOverflow)?,
        )
        .ok_or(WalError::OffsetOverflow)
}

pub(crate) fn copy_file_contents(
    source: &mut dyn FileHandle,
    destination: &mut dyn FileHandle,
) -> Result<(), WalError> {
    let length = source.len()?;
    destination.set_len(length)?;
    let mut buffer = [0; 8192];
    let mut offset = 0_u64;
    while offset < length {
        let chunk = usize::try_from((length - offset).min(buffer.len() as u64))
            .map_err(|_| WalError::OffsetOverflow)?;
        read_exact_at(source, offset, &mut buffer[..chunk])?;
        write_all_at(destination, offset, &buffer[..chunk])?;
        offset = offset
            .checked_add(u64::try_from(chunk).map_err(|_| WalError::OffsetOverflow)?)
            .ok_or(WalError::OffsetOverflow)?;
    }
    destination.sync_all()?;
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_exact_at(
    file: &mut dyn FileHandle,
    mut offset: u64,
    mut buffer: &mut [u8],
) -> Result<(), WalError> {
    while !buffer.is_empty() {
        let read = file.read_at(offset, buffer)?;
        if read == 0 {
            return Err(WalError::UnexpectedEof);
        }
        if read > buffer.len() {
            return Err(WalError::CorruptFrame(
                "VFS returned more bytes than requested",
            ));
        }
        offset = offset
            .checked_add(u64::try_from(read).map_err(|_| WalError::OffsetOverflow)?)
            .ok_or(WalError::OffsetOverflow)?;
        buffer = &mut buffer[read..];
    }
    Ok(())
}

fn write_all_at(
    file: &mut dyn FileHandle,
    mut offset: u64,
    mut buffer: &[u8],
) -> Result<(), WalError> {
    while !buffer.is_empty() {
        let written = file.write_at(offset, buffer)?;
        if written == 0 {
            return Err(WalError::WriteZero);
        }
        if written > buffer.len() {
            return Err(WalError::CorruptFrame(
                "VFS wrote more bytes than requested",
            ));
        }
        offset = offset
            .checked_add(u64::try_from(written).map_err(|_| WalError::OffsetOverflow)?)
            .ok_or(WalError::OffsetOverflow)?;
        buffer = &buffer[written..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ferrite_vfs::SimVfs;

    use super::*;

    fn page_number(value: u32) -> PageNumber {
        PageNumber::new(value).expect("valid page number")
    }

    fn page_with_byte(value: u8) -> Page {
        let mut page = Page::zeroed();
        page.as_mut_bytes().fill(value);
        page
    }

    fn crc32c_append_bitwise(previous: u32, bytes: &[u8]) -> u32 {
        let mut checksum = !previous;
        for byte in bytes {
            checksum ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(checksum & 1);
                checksum = (checksum >> 1) ^ (0x82f6_3b78 & mask);
            }
        }
        !checksum
    }

    #[test]
    fn wal_header_round_trips_and_crc32c_matches_the_standard_vector() {
        assert_eq!(crc32c_append(0, b"123456789"), 0xe306_9283);
        let vfs = SimVfs::new();
        let header = WalHeader::new(&vfs).expect("create WAL header");
        let encoded = header.encode();
        let decoded = WalHeader::decode(&encoded).expect("decode WAL header");

        assert_eq!(decoded.salt1, header.salt1);
        assert_eq!(decoded.salt2, header.salt2);
        assert_eq!(decoded.checksums, header.checksums);

        let mut corrupt = encoded;
        corrupt[20] ^= 0x80;
        assert!(matches!(
            WalHeader::decode(&corrupt),
            Err(WalError::ChecksumMismatch)
        ));
    }

    #[test]
    fn table_and_fused_crc32c_match_bitwise_reference_for_page_sized_input() {
        let bytes = (0..PAGE_SIZE + 8)
            .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
            .collect::<Vec<_>>();
        for seed in [0, SECOND_CHECKSUM_SEED, u32::MAX] {
            assert_eq!(
                crc32c_append(seed, &bytes),
                crc32c_append_bitwise(seed, &bytes)
            );
        }

        let header: [u8; 8] = bytes[..8].try_into().expect("8-byte frame prefix");
        let page = &bytes[8..];
        let previous = Checksums {
            first: 0x1234_5678,
            second: SECOND_CHECKSUM_SEED,
        };
        assert_eq!(
            frame_checksums(previous, &header, page),
            Checksums {
                first: crc32c_append_bitwise(crc32c_append_bitwise(previous.first, &header), page),
                second: crc32c_append_bitwise(
                    crc32c_append_bitwise(previous.second, &header),
                    page
                ),
            }
        );
    }

    #[test]
    fn wal_commits_pages_and_preserves_older_snapshot_reads() {
        let vfs = SimVfs::new();
        let database_path = Path::new("snapshot.fdb");
        let mut wal = Wal::open_for_database(&vfs, database_path).expect("open WAL");
        let original = page_with_byte(0x11);
        let replacement = page_with_byte(0x22);
        let second = page_with_byte(0x33);
        let initial_snapshot = wal.snapshot(1).expect("initial snapshot");
        assert!(
            wal.read_page(page_number(1), initial_snapshot)
                .expect("read before commit")
                .is_none()
        );

        let first_snapshot = wal
            .commit_pages(&[(page_number(1), &original)], 1)
            .expect("commit first page");
        let latest_snapshot = wal
            .commit_pages(
                &[(page_number(1), &replacement), (page_number(2), &second)],
                2,
            )
            .expect("commit two pages");

        assert_eq!(first_snapshot.end_frame(), 1);
        assert_eq!(latest_snapshot.end_frame(), 3);
        assert_eq!(
            wal.read_page(page_number(1), first_snapshot)
                .expect("read old snapshot")
                .expect("page in first snapshot")
                .as_bytes(),
            original.as_bytes()
        );
        assert_eq!(
            wal.read_page(page_number(1), latest_snapshot)
                .expect("read latest snapshot")
                .expect("page in latest snapshot")
                .as_bytes(),
            replacement.as_bytes()
        );

        drop(wal);
        let mut reopened = Wal::open_for_database(&vfs, database_path).expect("reopen WAL");
        assert_eq!(reopened.frame_count(), 3);
        let recovered = reopened.snapshot(1).expect("recovered snapshot");
        assert_eq!(recovered.database_size(), 2);
        assert_eq!(
            reopened
                .read_page(page_number(2), recovered)
                .expect("read recovered page")
                .expect("page survives reopen")
                .as_bytes(),
            second.as_bytes()
        );
    }

    #[test]
    fn recovery_discards_a_torn_uncommitted_transaction() {
        let vfs = SimVfs::new();
        let database_path = Path::new("torn-transaction.fdb");
        let mut wal = Wal::open_for_database(&vfs, database_path).expect("open WAL");
        let page1 = page_with_byte(0x44);
        let page2 = page_with_byte(0x55);
        vfs.inject_torn_write(2, 7)
            .expect("configure torn WAL frame");

        assert!(matches!(
            wal.commit_pages(&[(page_number(1), &page1), (page_number(2), &page2)], 2),
            Err(WalError::CommitUncertain(_))
        ));
        assert!(matches!(wal.snapshot(1), Err(WalError::NeedsRecovery)));
        drop(wal);
        vfs.clear_fault().expect("restore simulated storage");

        let mut recovered = Wal::open_for_database(&vfs, database_path).expect("recover WAL");
        assert_eq!(recovered.frame_count(), 0);
        let snapshot = recovered.snapshot(1).expect("snapshot after recovery");
        assert!(
            recovered
                .read_page(page_number(1), snapshot)
                .expect("read after recovery")
                .is_none()
        );
    }

    #[test]
    fn recovery_rejects_a_corrupted_committed_frame() {
        let vfs = SimVfs::new();
        let database_path = Path::new("corrupt-frame.fdb");
        let page = page_with_byte(0x66);
        let mut wal = Wal::open_for_database(&vfs, database_path).expect("open WAL");
        wal.commit_pages(&[(page_number(1), &page)], 1)
            .expect("commit page");
        drop(wal);

        let wal_path = Wal::path_for_database(database_path);
        let mut file = vfs
            .open(
                &wal_path,
                OpenOptions {
                    read: true,
                    write: true,
                    create: false,
                    truncate: false,
                },
            )
            .expect("open WAL for corruption");
        file.write_at(WAL_HEADER_SIZE + FRAME_HEADER_SIZE + 9, &[0xff])
            .expect("corrupt committed page");
        drop(file);

        assert!(matches!(
            Wal::open_for_database(&vfs, database_path),
            Err(WalError::ChecksumMismatch)
        ));
    }

    #[test]
    fn opening_wal_requires_the_writer_lock_for_recovery() {
        let vfs = SimVfs::new();
        let database_path = Path::new("locked-open.fdb");
        drop(Wal::open_for_database(&vfs, database_path).expect("create WAL"));

        let wal_path = Wal::path_for_database(database_path);
        let mut writer = vfs
            .open(
                &wal_path,
                OpenOptions {
                    read: true,
                    write: true,
                    create: false,
                    truncate: false,
                },
            )
            .expect("open WAL as writer");
        writer
            .lock(LockMode::Exclusive)
            .expect("lock WAL as writer");

        assert!(matches!(
            Wal::open_for_database(&vfs, database_path),
            Err(WalError::Vfs(VfsError::LockBusy))
        ));

        writer.unlock().expect("unlock WAL");
        assert!(Wal::open_for_database(&vfs, database_path).is_ok());
    }

    #[test]
    fn wal_rejects_empty_and_duplicate_page_commits() {
        let vfs = SimVfs::new();
        let mut wal =
            Wal::open_for_database(&vfs, Path::new("invalid-commit.fdb")).expect("open WAL");
        let page = page_with_byte(0x77);

        assert!(matches!(
            wal.commit_pages(&[], 1),
            Err(WalError::EmptyCommit)
        ));
        assert!(matches!(
            wal.commit_pages(&[(page_number(1), &page), (page_number(1), &page)], 1),
            Err(WalError::DuplicatePage)
        ));
        assert_eq!(wal.frame_count(), 0);
    }
}
