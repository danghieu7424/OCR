use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use ferrite_vfs::{FileHandle, LockMode, OpenOptions, Vfs, VfsError};
use thiserror::Error;

mod wal;
pub use wal::{Wal, WalError, WalSnapshot, WalStats};

pub const PAGE_SIZE: usize = 4096;
pub const DATABASE_HEADER_SIZE: usize = 100;
pub const DATABASE_MAGIC: &[u8; 16] = b"FerriteDB fmt 1\0";
pub const FORMAT_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum PagerError {
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),
    #[error("database header error: {0}")]
    Header(#[from] HeaderError),
    #[error("page number zero is invalid")]
    InvalidPageNumber,
    #[error("page cache capacity must be nonzero")]
    ZeroCacheCapacity,
    #[error("page-cache capacity cannot be allocated")]
    CacheAllocation,
    #[error("database file is empty")]
    EmptyDatabase,
    #[error("database file length is not a multiple of {PAGE_SIZE}")]
    MisalignedFileLength,
    #[error("database contains more pages than the format can address")]
    TooManyPages,
    #[error(
        "database header page count {header_count} does not match file page count {file_count}"
    )]
    PageCountMismatch { header_count: u32, file_count: u32 },
    #[error("page {page_number} is outside database page range 1..={page_count}")]
    PageOutOfRange { page_number: u32, page_count: u32 },
    #[error("database file already contains data")]
    DatabaseAlreadyExists,
    #[error("database page read ended before all {PAGE_SIZE} bytes were read")]
    UnexpectedEof,
    #[error("database page write made no progress")]
    WriteZero,
    #[error("all page-cache frames are pinned")]
    CacheFullPinned,
    #[error("page-cache lock was poisoned")]
    CachePoisoned,
    #[error("page-cache pin counter overflow")]
    PinCountOverflow,
    #[error("page-cache frame generation exhausted")]
    FrameGenerationExhausted,
    #[error("page-cache frame handle is stale")]
    StaleFrame,
    #[error("cached pages are pinned and cannot be invalidated")]
    CachePagesPinned,
    #[error("page {page_number} is not present in the database or committed WAL")]
    MissingWalPage { page_number: u32 },
    #[error("WAL error: {0}")]
    Wal(#[from] WalError),
    #[error("page transaction allocation failed")]
    TransactionAllocation,
    #[error("checkpoint threshold must be greater than zero")]
    InvalidCheckpointThreshold,
    #[error("backup source and target paths must be different")]
    BackupPathsConflict,
    #[error("backup destination already exists")]
    BackupTargetExists,
    #[error("reader registry lock was poisoned")]
    ReaderRegistryPoisoned,
    #[error("reader registration identifier exhausted")]
    ReaderIdExhausted,
    #[error("reader registry allocation failed")]
    ReaderRegistryAllocation,
    #[error("WAL commit is durable, but post-commit pager coordination failed: {0}")]
    CommitDurable(String),
    #[error("WAL commit status is uncertain; reopen before deciding whether it committed: {0}")]
    CommitUncertain(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PageNumber(NonZeroU32);

impl PageNumber {
    pub fn new(value: u32) -> Result<Self, PagerError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(PagerError::InvalidPageNumber)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// A fixed-size page buffer with no interpretation of its payload bytes.
///
/// ```text
/// 0                                                       PAGE_SIZE
/// +-----------------------------------------------------------+
/// |                    4096 raw bytes                         |
/// +-----------------------------------------------------------+
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointOutcome {
    NotNeeded,
    Checkpointed,
    Deferred,
}

pub struct Page {
    bytes: Box<[u8; PAGE_SIZE]>,
}

impl Page {
    pub fn zeroed() -> Self {
        Self {
            bytes: Box::new([0; PAGE_SIZE]),
        }
    }

    pub fn as_bytes(&self) -> &[u8; PAGE_SIZE] {
        &self.bytes
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

struct Frame {
    page_number: Option<PageNumber>,
    page: Option<Page>,
    pin_count: u32,
    referenced: bool,
    generation: u64,
}

struct CacheInner {
    frames: Vec<Frame>,
    page_to_frame: HashMap<PageNumber, usize>,
    clock_hand: usize,
    stats: CacheStats,
}

pub struct PageCache {
    inner: Arc<Mutex<CacheInner>>,
}

#[derive(Clone, Copy)]
struct FrameId {
    index: usize,
    generation: u64,
    page_number: PageNumber,
}

pub struct PageGuard {
    inner: Arc<Mutex<CacheInner>>,
    frame_id: FrameId,
}

impl PageCache {
    pub fn new(capacity: usize) -> Result<Self, PagerError> {
        if capacity == 0 {
            return Err(PagerError::ZeroCacheCapacity);
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| PagerError::CacheAllocation)?;
        frames.resize_with(capacity, || Frame {
            page_number: None,
            page: None,
            pin_count: 0,
            referenced: false,
            generation: 0,
        });
        let mut page_to_frame = HashMap::new();
        page_to_frame
            .try_reserve(capacity)
            .map_err(|_| PagerError::CacheAllocation)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(CacheInner {
                frames,
                page_to_frame,
                clock_hand: 0,
                stats: CacheStats::default(),
            })),
        })
    }

    pub fn capacity(&self) -> Result<usize, PagerError> {
        let inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        Ok(inner.frames.len())
    }

    pub fn stats(&self) -> Result<CacheStats, PagerError> {
        let inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        Ok(inner.stats)
    }

    fn clear(&self) -> Result<(), PagerError> {
        let mut inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        if inner.frames.iter().any(|frame| frame.pin_count != 0) {
            return Err(PagerError::CachePagesPinned);
        }
        inner.page_to_frame.clear();
        for frame in &mut inner.frames {
            frame.page_number = None;
            frame.page = None;
            frame.referenced = false;
        }
        Ok(())
    }

    fn pin_if_cached(&self, page_number: PageNumber) -> Result<Option<PageGuard>, PagerError> {
        let mut inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        let Some(&index) = inner.page_to_frame.get(&page_number) else {
            inner.stats.misses = inner.stats.misses.saturating_add(1);
            return Ok(None);
        };
        let generation = {
            let frame = &mut inner.frames[index];
            frame.pin_count = frame
                .pin_count
                .checked_add(1)
                .ok_or(PagerError::PinCountOverflow)?;
            frame.referenced = true;
            frame.generation
        };
        inner.stats.hits = inner.stats.hits.saturating_add(1);
        Ok(Some(PageGuard {
            inner: Arc::clone(&self.inner),
            frame_id: FrameId {
                index,
                generation,
                page_number,
            },
        }))
    }

    fn insert(&self, page_number: PageNumber, page: Page) -> Result<PageGuard, PagerError> {
        let mut inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        if let Some(&index) = inner.page_to_frame.get(&page_number) {
            let frame = &mut inner.frames[index];
            frame.pin_count = frame
                .pin_count
                .checked_add(1)
                .ok_or(PagerError::PinCountOverflow)?;
            frame.referenced = true;
            return Ok(PageGuard {
                inner: Arc::clone(&self.inner),
                frame_id: FrameId {
                    index,
                    generation: frame.generation,
                    page_number,
                },
            });
        }

        let index = select_frame(&mut inner)?;
        let generation = inner.frames[index]
            .generation
            .checked_add(1)
            .ok_or(PagerError::FrameGenerationExhausted)?;
        if let Some(evicted_page) = inner.frames[index].page_number {
            inner.page_to_frame.remove(&evicted_page);
            inner.stats.evictions = inner.stats.evictions.saturating_add(1);
        }
        let frame = &mut inner.frames[index];
        frame.generation = generation;
        frame.page_number = Some(page_number);
        frame.page = Some(page);
        frame.pin_count = 1;
        frame.referenced = true;
        inner.page_to_frame.insert(page_number, index);
        Ok(PageGuard {
            inner: Arc::clone(&self.inner),
            frame_id: FrameId {
                index,
                generation,
                page_number,
            },
        })
    }
}

fn select_frame(inner: &mut CacheInner) -> Result<usize, PagerError> {
    let capacity = inner.frames.len();
    for _ in 0..capacity.saturating_mul(2) {
        let index = inner.clock_hand;
        inner.clock_hand = (inner.clock_hand + 1) % capacity;
        let frame = &mut inner.frames[index];
        if frame.pin_count > 0 {
            continue;
        }
        if frame.page_number.is_none() {
            return Ok(index);
        }
        if frame.referenced {
            frame.referenced = false;
            continue;
        }
        return Ok(index);
    }
    Err(PagerError::CacheFullPinned)
}

impl PageGuard {
    pub fn page_number(&self) -> PageNumber {
        self.frame_id.page_number
    }

    /// Runs while the frame is locked; the callback must not re-enter this cache.
    pub fn with_bytes<R>(&self, read: impl FnOnce(&[u8; PAGE_SIZE]) -> R) -> Result<R, PagerError> {
        let inner = self.inner.lock().map_err(|_| PagerError::CachePoisoned)?;
        let frame = inner
            .frames
            .get(self.frame_id.index)
            .filter(|frame| {
                frame.generation == self.frame_id.generation
                    && frame.page_number == Some(self.frame_id.page_number)
            })
            .ok_or(PagerError::StaleFrame)?;
        let page = frame.page.as_ref().ok_or(PagerError::StaleFrame)?;
        Ok(read(page.as_bytes()))
    }
}

impl Drop for PageGuard {
    fn drop(&mut self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let Some(frame) = inner.frames.get_mut(self.frame_id.index) else {
            return;
        };
        if frame.generation == self.frame_id.generation
            && frame.page_number == Some(self.frame_id.page_number)
        {
            frame.pin_count = frame.pin_count.saturating_sub(1);
        }
    }
}

struct ReaderRegistry {
    coordination_file: Mutex<Box<dyn FileHandle>>,
    readers: HashMap<u64, WalSnapshot>,
    latest_snapshot: Option<WalSnapshot>,
    next_reader_id: u64,
}

impl ReaderRegistry {
    fn new(coordination_file: Box<dyn FileHandle>) -> Self {
        Self {
            coordination_file: Mutex::new(coordination_file),
            readers: HashMap::new(),
            latest_snapshot: None,
            next_reader_id: 0,
        }
    }

    fn try_upgrade_lock(&mut self, checkpoint_file: &mut dyn FileHandle) -> Result<(), VfsError> {
        let mut coordination_file = self
            .coordination_file
            .lock()
            .map_err(|_| VfsError::Io(std::io::Error::other("reader lock was poisoned")))?;
        coordination_file.unlock()?;
        match checkpoint_file.lock(LockMode::Exclusive) {
            Ok(()) => Ok(()),
            Err(error) => {
                coordination_file.lock(LockMode::Shared)?;
                Err(error)
            }
        }
    }

    fn restore_shared_lock(
        &mut self,
        checkpoint_file: &mut dyn FileHandle,
    ) -> Result<(), VfsError> {
        let mut coordination_file = self
            .coordination_file
            .lock()
            .map_err(|_| VfsError::Io(std::io::Error::other("reader lock was poisoned")))?;
        checkpoint_file.unlock()?;
        coordination_file.lock(LockMode::Shared)
    }
}

impl Drop for ReaderRegistry {
    fn drop(&mut self) {
        if let Ok(coordination_file) = self.coordination_file.get_mut() {
            let _ = coordination_file.unlock();
        }
    }
}

type RegistryKey = (usize, std::path::PathBuf);
type RegistryMap = HashMap<RegistryKey, Weak<RwLock<ReaderRegistry>>>;

static READER_REGISTRIES: OnceLock<Mutex<RegistryMap>> = OnceLock::new();

fn reader_registry(vfs: &dyn Vfs, path: &Path) -> Result<Arc<RwLock<ReaderRegistry>>, PagerError> {
    let key = (vfs.identity(), path.to_path_buf());
    let registries = READER_REGISTRIES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registries = registries
        .lock()
        .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
    if let Some(registry) = registries.get(&key).and_then(Weak::upgrade) {
        return Ok(registry);
    }
    let mut coordination_file = vfs.open(
        path,
        OpenOptions {
            read: true,
            write: true,
            create: false,
            truncate: false,
        },
    )?;
    coordination_file.lock(LockMode::Shared)?;
    let registry = Arc::new(RwLock::new(ReaderRegistry::new(coordination_file)));
    registries.insert(key, Arc::downgrade(&registry));
    Ok(registry)
}

struct ReaderRegistration {
    id: u64,
    registry: Arc<RwLock<ReaderRegistry>>,
}

impl ReaderRegistration {
    fn register(
        registry: &Arc<RwLock<ReaderRegistry>>,
        snapshot: WalSnapshot,
    ) -> Result<Self, PagerError> {
        let mut state = registry
            .write()
            .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
        state
            .readers
            .try_reserve(1)
            .map_err(|_| PagerError::ReaderRegistryAllocation)?;
        let id = state
            .next_reader_id
            .checked_add(1)
            .ok_or(PagerError::ReaderIdExhausted)?;
        state.next_reader_id = id;
        state.readers.insert(id, snapshot);
        state.latest_snapshot = Some(snapshot);
        Ok(Self {
            id,
            registry: Arc::clone(registry),
        })
    }

    fn publish(&self, snapshot: WalSnapshot) -> Result<(), PagerError> {
        let mut state = self
            .registry
            .write()
            .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
        let reader = state
            .readers
            .get_mut(&self.id)
            .ok_or(PagerError::ReaderRegistryPoisoned)?;
        *reader = snapshot;
        state.latest_snapshot = Some(snapshot);
        Ok(())
    }
}

impl Drop for ReaderRegistration {
    fn drop(&mut self) {
        if let Ok(mut state) = self.registry.write() {
            state.readers.remove(&self.id);
        }
    }
}

pub struct Pager {
    file: Box<dyn FileHandle>,
    path: PathBuf,
    header: DatabaseHeader,
    database_page_count: u32,
    cache: PageCache,
    wal: Wal,
    snapshot: WalSnapshot,
    reader: ReaderRegistration,
}

impl Pager {
    pub fn create(vfs: &dyn Vfs, path: &Path, cache_capacity: usize) -> Result<Self, PagerError> {
        let cache = PageCache::new(cache_capacity)?;
        let mut file = vfs.open(
            path,
            OpenOptions {
                read: true,
                write: true,
                create: true,
                truncate: false,
            },
        )?;
        if file.len()? != 0 {
            return Err(PagerError::DatabaseAlreadyExists);
        }
        file.lock(ferrite_vfs::LockMode::Exclusive)?;
        if file.len()? != 0 {
            file.unlock()?;
            return Err(PagerError::DatabaseAlreadyExists);
        }

        let header = DatabaseHeader::new();
        let mut page = Page::zeroed();
        page.as_mut_bytes()[..DATABASE_HEADER_SIZE].copy_from_slice(&header.encode()?);
        write_all_at(&mut *file, 0, page.as_bytes())?;
        file.sync_all()?;
        let wal = Wal::create_for_database(vfs, path)?;
        file.unlock()?;
        let snapshot = wal.snapshot(header.page_count)?;
        let registry = reader_registry(vfs, path)?;
        let reader = ReaderRegistration::register(&registry, snapshot)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            header,
            database_page_count: 1,
            cache,
            wal,
            snapshot,
            reader,
        })
    }

    pub fn open(vfs: &dyn Vfs, path: &Path, cache_capacity: usize) -> Result<Self, PagerError> {
        let cache = PageCache::new(cache_capacity)?;
        let mut file = vfs.open(
            path,
            OpenOptions {
                read: true,
                write: true,
                create: false,
                truncate: false,
            },
        )?;
        let registry = reader_registry(vfs, path)?;
        let mut registry_state = registry
            .write()
            .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
        let file_length = file.len()?;
        if file_length == 0 {
            return Err(PagerError::EmptyDatabase);
        }
        if file_length % PAGE_SIZE as u64 != 0 {
            return Err(PagerError::MisalignedFileLength);
        }
        let file_page_count =
            u32::try_from(file_length / PAGE_SIZE as u64).map_err(|_| PagerError::TooManyPages)?;
        let mut first_page = Page::zeroed();
        read_exact_at(&mut *file, 0, first_page.as_mut_bytes())?;
        let main_header = DatabaseHeader::decode(&first_page.as_bytes()[..DATABASE_HEADER_SIZE]);
        let mut wal = Wal::open_for_database(vfs, path)?;
        let base_page_count = main_header
            .as_ref()
            .map(|header| header.page_count)
            .unwrap_or(file_page_count);
        let recovered_snapshot = wal.snapshot(base_page_count)?;
        let snapshot = registry_state
            .latest_snapshot
            .filter(|published| {
                published.same_log(recovered_snapshot)
                    && published.end_frame() <= recovered_snapshot.end_frame()
            })
            .unwrap_or(recovered_snapshot);
        if registry_state
            .latest_snapshot
            .is_none_or(|published| !published.same_log(recovered_snapshot))
        {
            registry_state.latest_snapshot = Some(recovered_snapshot);
        }
        if file_page_count > snapshot.database_size() {
            return Err(PagerError::PageCountMismatch {
                header_count: snapshot.database_size(),
                file_count: file_page_count,
            });
        }
        let root = PageNumber::new(1)?;
        let mut header = if let Some(page) = wal.read_page(root, snapshot)? {
            let wal_header = DatabaseHeader::decode(&page.as_bytes()[..DATABASE_HEADER_SIZE])?;
            if wal_header.page_count != snapshot.database_size() {
                return Err(PagerError::PageCountMismatch {
                    header_count: wal_header.page_count,
                    file_count: snapshot.database_size(),
                });
            }
            wal_header
        } else {
            let mut header = main_header?;
            if wal.frame_count() == 0 && header.page_count != file_page_count {
                return Err(PagerError::PageCountMismatch {
                    header_count: header.page_count,
                    file_count: file_page_count,
                });
            }
            header.page_count = snapshot.database_size();
            header
        };
        header.page_count = snapshot.database_size();
        registry_state
            .readers
            .try_reserve(1)
            .map_err(|_| PagerError::ReaderRegistryAllocation)?;
        let reader_id = registry_state
            .next_reader_id
            .checked_add(1)
            .ok_or(PagerError::ReaderIdExhausted)?;
        registry_state.next_reader_id = reader_id;
        registry_state.readers.insert(reader_id, snapshot);
        drop(registry_state);
        let reader = ReaderRegistration {
            id: reader_id,
            registry,
        };
        Ok(Self {
            file,
            path: path.to_path_buf(),
            header,
            database_page_count: file_page_count,
            cache,
            wal,
            snapshot,
            reader,
        })
    }

    pub fn header(&self) -> &DatabaseHeader {
        &self.header
    }

    pub fn page_count(&self) -> u32 {
        self.header.page_count
    }

    pub fn cache_stats(&self) -> Result<CacheStats, PagerError> {
        self.cache.stats()
    }

    pub fn wal_stats(&mut self) -> Result<WalStats, PagerError> {
        Ok(self.wal.stats()?)
    }

    pub fn backup_to(&mut self, vfs: &dyn Vfs, backup_path: &Path) -> Result<(), PagerError> {
        let source_wal_path = Wal::path_for_database(&self.path);
        let backup_wal_path = Wal::path_for_database(backup_path);
        if backup_path == self.path.as_path()
            || backup_path == source_wal_path.as_path()
            || backup_wal_path.as_path() == self.path.as_path()
            || backup_wal_path == source_wal_path
        {
            return Err(PagerError::BackupPathsConflict);
        }
        ensure_path_missing(vfs, backup_path)?;
        ensure_path_missing(vfs, &backup_wal_path)?;
        self.wal.backup_to(&mut *self.file, vfs, backup_path)?;
        Ok(())
    }

    pub fn restore_from(
        vfs: &dyn Vfs,
        backup_path: &Path,
        database_path: &Path,
        cache_capacity: usize,
    ) -> Result<(), PagerError> {
        let backup_wal_path = Wal::path_for_database(backup_path);
        let database_wal_path = Wal::path_for_database(database_path);
        if backup_path == database_path
            || backup_path == database_wal_path.as_path()
            || backup_wal_path.as_path() == database_path
            || backup_wal_path == database_wal_path
        {
            return Err(PagerError::BackupPathsConflict);
        }
        ensure_path_missing(vfs, database_path)?;
        ensure_path_missing(vfs, &database_wal_path)?;

        let mut backup_database = vfs.open(
            backup_path,
            OpenOptions {
                read: true,
                write: false,
                create: false,
                truncate: false,
            },
        )?;
        let mut backup_wal = vfs.open(
            &backup_wal_path,
            OpenOptions {
                read: true,
                write: false,
                create: false,
                truncate: false,
            },
        )?;
        let mut database = vfs.open(
            database_path,
            OpenOptions {
                read: false,
                write: true,
                create: true,
                truncate: true,
            },
        )?;
        let mut wal = vfs.open(
            &database_wal_path,
            OpenOptions {
                read: false,
                write: true,
                create: true,
                truncate: true,
            },
        )?;
        wal::copy_file_contents(&mut *backup_database, &mut *database)?;
        wal::copy_file_contents(&mut *backup_wal, &mut *wal)?;
        drop(wal);
        drop(database);
        drop(backup_wal);
        drop(backup_database);

        let restored = Self::open(vfs, database_path, cache_capacity)?;
        drop(restored);
        Ok(())
    }

    pub fn begin_write(&mut self) -> PagerTransaction<'_> {
        let database_size = self.header.page_count;
        PagerTransaction {
            pager: self,
            database_size,
            pages: HashMap::new(),
        }
    }

    pub fn checkpoint(&mut self, vfs: &dyn Vfs) -> Result<(), PagerError> {
        let registry_arc = Arc::clone(&self.reader.registry);
        let mut registry = registry_arc
            .write()
            .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
        let reader_id = self.reader.id;
        self.wal.with_checkpoint_lock(|wal| {
            let min_reader_frame = registry
                .readers
                .values()
                .map(|snapshot| snapshot.end_frame())
                .min()
                .unwrap_or_else(|| wal.frame_count());
            if min_reader_frame == 0 {
                return Ok(());
            }

            registry.try_upgrade_lock(&mut *self.file)?;
            let checkpoint_result = (|| {
                self.cache.clear()?;
                let reset_wal =
                    registry.readers.len() == 1 && self.snapshot.end_frame() == wal.frame_count();
                if let Some(snapshot) = wal.checkpoint_through_locked(
                    &mut *self.file,
                    vfs,
                    min_reader_frame,
                    reset_wal,
                )? {
                    let mut first_page = Page::zeroed();
                    read_exact_at(&mut *self.file, 0, first_page.as_mut_bytes())?;
                    self.header =
                        DatabaseHeader::decode(&first_page.as_bytes()[..DATABASE_HEADER_SIZE])?;
                    self.database_page_count = self.header.page_count;
                    self.snapshot = snapshot;
                    registry.readers.insert(reader_id, snapshot);
                    registry.latest_snapshot = Some(snapshot);
                }
                Ok(())
            })();
            let restore_result = registry.restore_shared_lock(&mut *self.file);
            match (checkpoint_result, restore_result) {
                (Err(error), _) => Err(error),
                (Ok(()), Err(error)) => Err(PagerError::Vfs(error)),
                (Ok(()), Ok(())) => Ok(()),
            }
        })
    }

    pub fn checkpoint_if_needed(
        &mut self,
        vfs: &dyn Vfs,
        threshold_bytes: u64,
    ) -> Result<CheckpointOutcome, PagerError> {
        if threshold_bytes == 0 {
            return Err(PagerError::InvalidCheckpointThreshold);
        }
        let stats = match self.wal_stats() {
            Ok(stats) => stats,
            Err(error) if is_lock_busy(&error) => return Ok(CheckpointOutcome::Deferred),
            Err(error) => return Err(error),
        };
        if stats.committed_frames == 0 || stats.bytes < threshold_bytes {
            return Ok(CheckpointOutcome::NotNeeded);
        }
        match self.checkpoint(vfs) {
            Ok(()) => {}
            Err(error) if is_lock_busy(&error) => return Ok(CheckpointOutcome::Deferred),
            Err(error) => return Err(error),
        }
        let stats = match self.wal_stats() {
            Ok(stats) => stats,
            Err(error) if is_lock_busy(&error) => return Ok(CheckpointOutcome::Deferred),
            Err(error) => return Err(error),
        };
        if stats.committed_frames == 0 {
            Ok(CheckpointOutcome::Checkpointed)
        } else {
            Ok(CheckpointOutcome::Deferred)
        }
    }

    pub fn read_page(&mut self, page_number: PageNumber) -> Result<PageGuard, PagerError> {
        let _registry = self
            .reader
            .registry
            .read()
            .map_err(|_| PagerError::ReaderRegistryPoisoned)?;
        if page_number.get() > self.header.page_count {
            return Err(PagerError::PageOutOfRange {
                page_number: page_number.get(),
                page_count: self.header.page_count,
            });
        }
        if let Some(page) = self.cache.pin_if_cached(page_number)? {
            return Ok(page);
        }

        if let Some(page) = self.wal.read_page(page_number, self.snapshot)? {
            return self.cache.insert(page_number, page);
        }
        if page_number.get() > self.database_page_count {
            return Err(PagerError::MissingWalPage {
                page_number: page_number.get(),
            });
        }
        let offset = u64::from(page_number.get() - 1)
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(PagerError::MisalignedFileLength)?;
        let mut page = Page::zeroed();
        read_exact_at(&mut *self.file, offset, page.as_mut_bytes())?;
        self.cache.insert(page_number, page)
    }
}

fn is_lock_busy(error: &PagerError) -> bool {
    matches!(
        error,
        PagerError::Vfs(VfsError::LockBusy) | PagerError::Wal(WalError::Vfs(VfsError::LockBusy))
    )
}

pub struct PagerTransaction<'a> {
    pager: &'a mut Pager,
    database_size: u32,
    pages: HashMap<PageNumber, Page>,
}

impl PagerTransaction<'_> {
    pub fn database_size(&self) -> u32 {
        self.database_size
    }

    pub fn allocate_page(&mut self) -> Result<PageNumber, PagerError> {
        let next = self
            .database_size
            .checked_add(1)
            .ok_or(PagerError::TooManyPages)?;
        let page_number = PageNumber::new(next)?;
        self.pages
            .try_reserve(1)
            .map_err(|_| PagerError::TransactionAllocation)?;
        self.pages.insert(page_number, Page::zeroed());
        self.database_size = next;
        Ok(page_number)
    }

    pub fn read_page(&mut self, page_number: PageNumber) -> Result<Page, PagerError> {
        if page_number.get() > self.database_size {
            return Err(PagerError::PageOutOfRange {
                page_number: page_number.get(),
                page_count: self.database_size,
            });
        }
        if let Some(page) = self.pages.get(&page_number) {
            return Ok(copy_page(page));
        }
        let guard = self.pager.read_page(page_number)?;
        let mut page = Page::zeroed();
        guard.with_bytes(|bytes| page.as_mut_bytes().copy_from_slice(bytes))?;
        Ok(page)
    }

    pub fn write_page(&mut self, page_number: PageNumber, page: &Page) -> Result<(), PagerError> {
        if page_number.get() > self.database_size {
            return Err(PagerError::PageOutOfRange {
                page_number: page_number.get(),
                page_count: self.database_size,
            });
        }
        self.pages
            .try_reserve(1)
            .map_err(|_| PagerError::TransactionAllocation)?;
        self.pages.insert(page_number, copy_page(page));
        Ok(())
    }

    pub fn commit(mut self) -> Result<(), PagerError> {
        if self.pages.is_empty() {
            return Ok(());
        }
        let root = PageNumber::new(1)?;
        let mut root_page = if let Some(page) = self.pages.remove(&root) {
            page
        } else {
            self.read_page(root)?
        };
        let mut header = self.pager.header.clone();
        header.page_count = self.database_size;
        header.change_counter = header.change_counter.wrapping_add(1);
        root_page.as_mut_bytes()[..DATABASE_HEADER_SIZE].copy_from_slice(&header.encode()?);
        self.pages.insert(root, root_page);

        let mut page_refs = Vec::new();
        page_refs
            .try_reserve_exact(self.pages.len())
            .map_err(|_| PagerError::TransactionAllocation)?;
        for (page_number, page) in &self.pages {
            page_refs.push((*page_number, page));
        }
        self.pager.cache.clear()?;
        let database_size = self.database_size;
        let snapshot = match self.pager.wal.commit_pages_from_snapshot(
            self.pager.snapshot,
            &page_refs,
            database_size,
        ) {
            Ok(snapshot) => snapshot,
            Err(WalError::CommitDurable(error)) => {
                return Err(PagerError::CommitDurable(error));
            }
            Err(WalError::CommitUncertain(error)) => {
                return Err(PagerError::CommitUncertain(error));
            }
            Err(error) => return Err(PagerError::Wal(error)),
        };
        self.pager.header = header;
        self.pager.snapshot = snapshot;
        self.pager
            .reader
            .publish(snapshot)
            .map_err(|error| PagerError::CommitDurable(error.to_string()))
    }
}

fn copy_page(page: &Page) -> Page {
    let mut copy = Page::zeroed();
    copy.as_mut_bytes().copy_from_slice(page.as_bytes());
    copy
}

fn ensure_path_missing(vfs: &dyn Vfs, path: &Path) -> Result<(), PagerError> {
    match vfs.open(
        path,
        OpenOptions {
            read: true,
            write: false,
            create: false,
            truncate: false,
        },
    ) {
        Ok(_) => Err(PagerError::BackupTargetExists),
        Err(VfsError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PagerError::Vfs(error)),
    }
}

fn read_exact_at(
    file: &mut dyn FileHandle,
    mut offset: u64,
    mut buffer: &mut [u8],
) -> Result<(), PagerError> {
    while !buffer.is_empty() {
        let read = file.read_at(offset, buffer)?;
        if read == 0 {
            return Err(PagerError::UnexpectedEof);
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or(PagerError::MisalignedFileLength)?;
        buffer = &mut buffer[read..];
    }
    Ok(())
}

fn write_all_at(
    file: &mut dyn FileHandle,
    mut offset: u64,
    mut buffer: &[u8],
) -> Result<(), PagerError> {
    while !buffer.is_empty() {
        let written = file.write_at(offset, buffer)?;
        if written == 0 {
            return Err(PagerError::WriteZero);
        }
        offset = offset
            .checked_add(written as u64)
            .ok_or(PagerError::MisalignedFileLength)?;
        buffer = &buffer[written..];
    }
    Ok(())
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum HeaderError {
    #[error("database header is shorter than {DATABASE_HEADER_SIZE} bytes")]
    TooShort,
    #[error("database magic does not match")]
    InvalidMagic,
    #[error("unsupported database format version: {0}")]
    UnsupportedVersion(u8),
    #[error("unsupported page size: {0}")]
    InvalidPageSize(u16),
    #[error("unsupported header flags: {0:#04x}")]
    UnsupportedFlags(u8),
    #[error("reserved header bytes must be zero")]
    NonZeroReservedBytes,
    #[error("database page count must include page 1")]
    InvalidPageCount,
    #[error("freelist head and count are inconsistent")]
    InvalidFreelist,
}

/// The first 100 bytes of page 1; all multibyte integers are big-endian.
///
/// ```text
///  0             16 18 19 20    24    28    32    36    40    44         100
///  +--------------+--+--+--+-----+-----+-----+-----+-----+-----+------------+
///  | Magic (16 B) |PS|V |F | Rsv | Cnt | Pg# | FL# | FLn | Sch | Reserved  |
///  +--------------+--+--+--+-----+-----+-----+-----+-----+-----+------------+
///  PS: page size; V: format version; F: flags; Cnt: change counter;
///  Pg#: database page count; FL#: freelist head; FLn: freelist count;
///  Sch: schema cookie.
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseHeader {
    pub page_size: u16,
    pub format_version: u8,
    pub flags: u8,
    pub change_counter: u32,
    pub page_count: u32,
    pub freelist_head: u32,
    pub freelist_count: u32,
    pub schema_cookie: u32,
}

impl DatabaseHeader {
    pub const fn new() -> Self {
        Self {
            page_size: PAGE_SIZE as u16,
            format_version: FORMAT_VERSION,
            flags: 0,
            change_counter: 0,
            page_count: 1,
            freelist_head: 0,
            freelist_count: 0,
            schema_cookie: 0,
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < DATABASE_HEADER_SIZE {
            return Err(HeaderError::TooShort);
        }
        if &bytes[0..16] != DATABASE_MAGIC {
            return Err(HeaderError::InvalidMagic);
        }
        let page_size = read_u16(bytes, 16);
        if usize::from(page_size) != PAGE_SIZE {
            return Err(HeaderError::InvalidPageSize(page_size));
        }
        if bytes[18] != FORMAT_VERSION {
            return Err(HeaderError::UnsupportedVersion(bytes[18]));
        }
        if bytes[19] != 0 {
            return Err(HeaderError::UnsupportedFlags(bytes[19]));
        }
        if bytes[20..24].iter().any(|byte| *byte != 0)
            || bytes[44..DATABASE_HEADER_SIZE]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(HeaderError::NonZeroReservedBytes);
        }

        let header = Self {
            page_size,
            format_version: bytes[18],
            flags: bytes[19],
            change_counter: read_u32(bytes, 24),
            page_count: read_u32(bytes, 28),
            freelist_head: read_u32(bytes, 32),
            freelist_count: read_u32(bytes, 36),
            schema_cookie: read_u32(bytes, 40),
        };
        header.validate()?;
        Ok(header)
    }

    pub fn encode(&self) -> Result<[u8; DATABASE_HEADER_SIZE], HeaderError> {
        self.validate()?;
        let mut bytes = [0; DATABASE_HEADER_SIZE];
        bytes[0..16].copy_from_slice(DATABASE_MAGIC);
        bytes[16..18].copy_from_slice(&self.page_size.to_be_bytes());
        bytes[18] = self.format_version;
        bytes[19] = self.flags;
        bytes[24..28].copy_from_slice(&self.change_counter.to_be_bytes());
        bytes[28..32].copy_from_slice(&self.page_count.to_be_bytes());
        bytes[32..36].copy_from_slice(&self.freelist_head.to_be_bytes());
        bytes[36..40].copy_from_slice(&self.freelist_count.to_be_bytes());
        bytes[40..44].copy_from_slice(&self.schema_cookie.to_be_bytes());
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), HeaderError> {
        if usize::from(self.page_size) != PAGE_SIZE {
            return Err(HeaderError::InvalidPageSize(self.page_size));
        }
        if self.format_version != FORMAT_VERSION {
            return Err(HeaderError::UnsupportedVersion(self.format_version));
        }
        if self.flags != 0 {
            return Err(HeaderError::UnsupportedFlags(self.flags));
        }
        if self.page_count == 0 {
            return Err(HeaderError::InvalidPageCount);
        }
        if (self.freelist_head == 0) != (self.freelist_count == 0)
            || self.freelist_head == 1
            || self.freelist_head > self.page_count
            || self.freelist_count > self.page_count.saturating_sub(1)
        {
            return Err(HeaderError::InvalidFreelist);
        }
        Ok(())
    }
}

impl Default for DatabaseHeader {
    fn default() -> Self {
        Self::new()
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_vfs::{OsVfs, SimVfs};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct ChildProcess(Child);

    impl Drop for ChildProcess {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    struct TemporaryDatabase(PathBuf);

    impl TemporaryDatabase {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after Unix epoch")
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "ferritedb-pager-{}-{nonce}.fdb",
                std::process::id()
            )))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(Wal::path_for_database(&self.0));
        }
    }

    #[test]
    fn header_round_trips_big_endian_fields() {
        let header = DatabaseHeader {
            page_size: PAGE_SIZE as u16,
            format_version: FORMAT_VERSION,
            flags: 0,
            change_counter: 0x0102_0304,
            page_count: 12,
            freelist_head: 3,
            freelist_count: 2,
            schema_cookie: 0xa0b0_c0d0,
        };
        let encoded = header.encode().expect("encode valid header");

        assert_eq!(&encoded[24..28], &[1, 2, 3, 4]);
        assert_eq!(
            DatabaseHeader::decode(&encoded).expect("decode header"),
            header
        );
    }

    #[test]
    fn rejects_short_header_and_bad_magic() {
        assert_eq!(DatabaseHeader::decode(&[0; 99]), Err(HeaderError::TooShort));

        let mut bytes = DatabaseHeader::new().encode().expect("encode header");
        bytes[0] = 0;
        assert_eq!(
            DatabaseHeader::decode(&bytes),
            Err(HeaderError::InvalidMagic)
        );
    }

    #[test]
    fn rejects_unsupported_version_page_size_and_flags() {
        let valid = DatabaseHeader::new().encode().expect("encode header");

        let mut bytes = valid;
        bytes[18] = 2;
        assert_eq!(
            DatabaseHeader::decode(&bytes),
            Err(HeaderError::UnsupportedVersion(2))
        );

        let mut bytes = valid;
        bytes[17] = 1;
        assert_eq!(
            DatabaseHeader::decode(&bytes),
            Err(HeaderError::InvalidPageSize(4097))
        );

        let mut bytes = valid;
        bytes[19] = 1;
        assert_eq!(
            DatabaseHeader::decode(&bytes),
            Err(HeaderError::UnsupportedFlags(1))
        );
    }

    #[test]
    fn rejects_nonzero_reserved_bytes_and_inconsistent_freelist() {
        let mut bytes = DatabaseHeader::new().encode().expect("encode header");
        bytes[99] = 1;
        assert_eq!(
            DatabaseHeader::decode(&bytes),
            Err(HeaderError::NonZeroReservedBytes)
        );

        let mut header = DatabaseHeader::new();
        header.freelist_count = 1;
        assert_eq!(header.encode(), Err(HeaderError::InvalidFreelist));

        header.freelist_head = 2;
        assert_eq!(header.encode(), Err(HeaderError::InvalidFreelist));
    }

    #[test]
    fn rejects_freelist_outside_page_range_and_empty_database() {
        let mut header = DatabaseHeader::new();
        header.freelist_head = 2;
        header.freelist_count = 1;
        assert_eq!(header.encode(), Err(HeaderError::InvalidFreelist));

        header.page_count = 0;
        header.freelist_head = 0;
        header.freelist_count = 0;
        assert_eq!(header.encode(), Err(HeaderError::InvalidPageCount));
    }

    #[test]
    fn creates_and_reopens_a_database_page() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-create.fdb");
        let mut pager = Pager::create(&vfs, path, 2).expect("create database");

        assert_eq!(pager.page_count(), 1);
        let page = pager
            .read_page(PageNumber::new(1).expect("valid page number"))
            .expect("read page 1");
        assert!(
            page.with_bytes(|bytes| &bytes[..DATABASE_MAGIC.len()] == DATABASE_MAGIC)
                .expect("read cached bytes")
        );
        drop(page);
        drop(pager);

        pager = Pager::open(&vfs, path, 2).expect("reopen database");
        assert_eq!(pager.header(), &DatabaseHeader::new());
    }

    #[test]
    fn concurrent_reader_keeps_its_snapshot_while_writer_commits() {
        let vfs = Arc::new(SimVfs::new());
        let path = Path::new("pager-concurrent-snapshot.fdb");
        let mut setup = Pager::create(vfs.as_ref(), path, 4).expect("create database");
        let page_number = {
            let mut transaction = setup.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0x11);
            transaction
                .write_page(page_number, &page)
                .expect("stage initial version");
            transaction.commit().expect("commit initial version");
            page_number
        };
        drop(setup);

        let mut reader = Pager::open(vfs.as_ref(), path, 4).expect("open snapshot reader");
        let start = Arc::new(Barrier::new(2));
        let writer_vfs = Arc::new(vfs.as_ref().clone());
        let writer_start = Arc::clone(&start);
        thread::scope(|scope| {
            scope.spawn(move || {
                let mut writer =
                    Pager::open(writer_vfs.as_ref(), path, 4).expect("open writer handle");
                writer_start.wait();
                let mut transaction = writer.begin_write();
                let mut page = Page::zeroed();
                page.as_mut_bytes().fill(0x22);
                transaction
                    .write_page(page_number, &page)
                    .expect("stage newer version");
                transaction.commit().expect("commit with reader open");
                assert_eq!(writer.page_count(), 2);
            });
            start.wait();
            let old_version = reader.read_page(page_number).expect("read old snapshot");
            assert!(
                old_version
                    .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x11))
                    .expect("inspect old snapshot")
            );
        });

        let old_version = reader.read_page(page_number).expect("read snapshot again");
        assert!(
            old_version
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x11))
                .expect("inspect stable snapshot")
        );
    }

    #[test]
    fn stale_writer_cannot_overwrite_a_newer_commit() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-stale-writer.fdb");
        let setup = Pager::create(&vfs, path, 4).expect("create database");
        drop(setup);

        let mut first_writer = Pager::open(&vfs, path, 4).expect("open first writer");
        let mut stale_writer = Pager::open(&vfs, path, 4).expect("open stale writer");
        let mut first_transaction = first_writer.begin_write();
        let page_number = first_transaction
            .allocate_page()
            .expect("allocate page for first writer");
        let mut first_page = Page::zeroed();
        first_page.as_mut_bytes().fill(0x71);
        first_transaction
            .write_page(page_number, &first_page)
            .expect("stage first writer page");
        first_transaction.commit().expect("commit first writer");

        let mut stale_transaction = stale_writer.begin_write();
        let stale_page_number = stale_transaction
            .allocate_page()
            .expect("allocate stale page");
        let mut stale_page = Page::zeroed();
        stale_page.as_mut_bytes().fill(0x82);
        stale_transaction
            .write_page(stale_page_number, &stale_page)
            .expect("stage stale writer page");
        assert!(matches!(
            stale_transaction.commit(),
            Err(PagerError::Wal(WalError::StaleWriterSnapshot))
        ));

        let committed = first_writer
            .read_page(page_number)
            .expect("read committed writer page");
        assert!(
            committed
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x71))
                .expect("inspect committed writer page")
        );
    }

    #[test]
    fn checkpoint_never_overwrites_a_page_beyond_min_reader_frame() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-bounded-checkpoint.fdb");
        let mut setup = Pager::create(&vfs, path, 4).expect("create database");
        let page_number = {
            let mut transaction = setup.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0x31);
            transaction
                .write_page(page_number, &page)
                .expect("stage first version");
            transaction.commit().expect("commit first version");
            page_number
        };
        drop(setup);

        let mut reader = Pager::open(&vfs, path, 4).expect("open old reader");
        let mut writer = Pager::open(&vfs, path, 4).expect("open writer");
        let mut transaction = writer.begin_write();
        let mut latest = Page::zeroed();
        latest.as_mut_bytes().fill(0x42);
        transaction
            .write_page(page_number, &latest)
            .expect("stage second version");
        transaction.commit().expect("commit second version");
        writer
            .checkpoint(&vfs)
            .expect("checkpoint through old reader");

        let mut main_file = vfs
            .open(
                path,
                OpenOptions {
                    read: true,
                    write: false,
                    create: false,
                    truncate: false,
                },
            )
            .expect("open main database file");
        let mut main_page = Page::zeroed();
        read_exact_at(
            &mut *main_file,
            u64::from(page_number.get() - 1) * PAGE_SIZE as u64,
            main_page.as_mut_bytes(),
        )
        .expect("read checkpointed main page");
        assert!(main_page.as_bytes().iter().all(|byte| *byte == 0x31));

        let old = reader
            .read_page(page_number)
            .expect("read pinned old version");
        assert!(
            old.with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x31))
                .expect("inspect pinned version")
        );
        drop(old);
        drop(reader);
        writer
            .checkpoint(&vfs)
            .expect("checkpoint after old reader closes");
        let latest = writer.read_page(page_number).expect("read latest version");
        assert!(
            latest
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x42))
                .expect("inspect latest version")
        );
    }

    #[test]
    fn checkpoint_policy_reports_wal_metrics_and_defers_for_old_readers() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-checkpoint-policy.fdb");
        let mut setup = Pager::create(&vfs, path, 4).expect("create database");
        let page_number = {
            let mut transaction = setup.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0x31);
            transaction
                .write_page(page_number, &page)
                .expect("stage initial version");
            transaction.commit().expect("commit initial version");
            page_number
        };
        let initial_stats = setup.wal_stats().expect("read WAL metrics");
        assert!(initial_stats.bytes > 32);
        assert!(initial_stats.committed_frames > 0);
        assert_eq!(
            setup
                .checkpoint_if_needed(&vfs, u64::MAX)
                .expect("skip checkpoint below threshold"),
            CheckpointOutcome::NotNeeded
        );
        drop(setup);

        let mut reader = Pager::open(&vfs, path, 4).expect("open old snapshot");
        let mut writer = Pager::open(&vfs, path, 4).expect("open writer");
        let mut transaction = writer.begin_write();
        let mut latest = Page::zeroed();
        latest.as_mut_bytes().fill(0x42);
        transaction
            .write_page(page_number, &latest)
            .expect("stage latest version");
        transaction.commit().expect("commit latest version");

        assert_eq!(
            writer
                .checkpoint_if_needed(&vfs, 1)
                .expect("defer checkpoint while old reader is open"),
            CheckpointOutcome::Deferred
        );
        let old_page = reader.read_page(page_number).expect("read old snapshot");
        assert!(
            old_page
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x31))
                .expect("inspect old snapshot")
        );
        drop(old_page);
        drop(reader);

        assert_eq!(
            writer
                .checkpoint_if_needed(&vfs, 1)
                .expect("finish checkpoint after reader closes"),
            CheckpointOutcome::Checkpointed
        );
        let final_stats = writer.wal_stats().expect("read final WAL metrics");
        assert_eq!(final_stats.committed_frames, 0);
        assert_eq!(final_stats.bytes, 32);
        let latest_page = writer.read_page(page_number).expect("read latest version");
        assert!(
            latest_page
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x42))
                .expect("inspect latest version")
        );
    }

    #[test]
    fn checkpoint_defers_for_reader_in_another_process() {
        const CHILD_PATH_ENV: &str = "FERRITEDB_PAGER_READER_CHILD_PATH";
        const CHILD_READY: &str = "FERRITEDB_READER_READY";

        if let Some(path) = std::env::var_os(CHILD_PATH_ENV) {
            let _reader = Pager::open(&OsVfs, Path::new(&path), 4).expect("open child reader");
            println!("{CHILD_READY}");
            std::io::stdout().flush().expect("flush readiness");
            let mut release = [0];
            std::io::stdin()
                .read_exact(&mut release)
                .expect("wait for parent release");
            return;
        }

        let database = TemporaryDatabase::new();
        let vfs = OsVfs;
        let mut writer = Pager::create(&vfs, database.path(), 4).expect("create database");
        let page_number = {
            let mut transaction = writer.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0x31);
            transaction
                .write_page(page_number, &page)
                .expect("stage initial version");
            transaction.commit().expect("commit initial version");
            page_number
        };

        let executable = std::env::current_exe().expect("locate test executable");
        let mut child = ChildProcess(
            Command::new(executable)
                .arg("--exact")
                .arg("tests::checkpoint_defers_for_reader_in_another_process")
                .arg("--nocapture")
                .env(CHILD_PATH_ENV, database.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("spawn child reader"),
        );
        let mut output = BufReader::new(child.0.stdout.take().expect("capture child output"));
        let mut line = String::new();
        loop {
            line.clear();
            assert_ne!(
                output.read_line(&mut line).expect("read child readiness"),
                0,
                "child exited before opening its reader"
            );
            if line.trim() == CHILD_READY {
                break;
            }
        }

        let mut transaction = writer.begin_write();
        let mut latest = Page::zeroed();
        latest.as_mut_bytes().fill(0x42);
        transaction
            .write_page(page_number, &latest)
            .expect("stage latest version");
        transaction.commit().expect("commit latest version");
        let checkpoint_result = writer.checkpoint(&vfs);
        assert!(
            matches!(checkpoint_result, Err(PagerError::Vfs(VfsError::LockBusy))),
            "expected the child reader to block checkpoint, got {checkpoint_result:?}"
        );
        assert_eq!(
            writer
                .checkpoint_if_needed(&vfs, 1)
                .expect("defer while child reader is active"),
            CheckpointOutcome::Deferred
        );

        child
            .0
            .stdin
            .as_mut()
            .expect("open child input")
            .write_all(&[1])
            .expect("release child reader");
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.0.try_wait().expect("check child status") {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "child reader did not exit after release"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(status.success(), "child test process failed: {status}");

        assert_eq!(
            writer
                .checkpoint_if_needed(&vfs, 1)
                .expect("checkpoint after child reader exits"),
            CheckpointOutcome::Checkpointed
        );
        let latest_page = writer.read_page(page_number).expect("read latest page");
        assert!(
            latest_page
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x42))
                .expect("inspect latest page")
        );
    }

    #[test]
    fn os_vfs_instances_share_the_process_reader_registry() {
        let database = TemporaryDatabase::new();
        let first_vfs = OsVfs;
        let second_vfs = OsVfs;
        let writer = Pager::create(&first_vfs, database.path(), 2).expect("create database");
        let reader = Pager::open(&second_vfs, database.path(), 2).expect("open second handle");

        assert!(Arc::ptr_eq(
            &writer.reader.registry,
            &reader.reader.registry
        ));
    }

    #[test]
    fn crash_during_bounded_checkpoint_recovers_latest_committed_snapshot() {
        use ferrite_vfs::SimFaultKind;

        let vfs = SimVfs::new();
        let path = Path::new("pager-concurrent-checkpoint-crash.fdb");
        let mut setup = Pager::create(&vfs, path, 4).expect("create database");
        let page_number = {
            let mut transaction = setup.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0x53);
            transaction
                .write_page(page_number, &page)
                .expect("stage first version");
            transaction.commit().expect("commit first version");
            page_number
        };
        drop(setup);

        let reader = Pager::open(&vfs, path, 4).expect("open old snapshot");
        let mut writer = Pager::open(&vfs, path, 4).expect("open writer");
        let mut transaction = writer.begin_write();
        let mut latest = Page::zeroed();
        latest.as_mut_bytes().fill(0x64);
        transaction
            .write_page(page_number, &latest)
            .expect("stage latest version");
        transaction.commit().expect("commit latest version");

        vfs.inject_torn_write(1, 13)
            .expect("configure crash during checkpoint");
        assert!(matches!(
            writer.checkpoint(&vfs),
            Err(PagerError::Wal(WalError::Vfs(VfsError::InjectedFault(
                SimFaultKind::PowerLoss
            ))))
        ));
        drop(reader);
        drop(writer);
        vfs.clear_fault().expect("clear simulated power loss");

        let mut recovered = Pager::open(&vfs, path, 4).expect("recover from WAL after crash");
        let page = recovered
            .read_page(page_number)
            .expect("read durable version");
        assert!(
            page.with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x64))
                .expect("inspect recovered version")
        );
    }

    #[test]
    fn pager_transaction_commits_recovers_and_checkpoints_new_pages() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-wal.fdb");
        let mut pager = Pager::create(&vfs, path, 4).expect("create database");
        let mut transaction = pager.begin_write();
        let page_number = transaction.allocate_page().expect("allocate page");
        let mut page = Page::zeroed();
        page.as_mut_bytes().fill(0x5a);
        transaction
            .write_page(page_number, &page)
            .expect("stage new page");
        transaction.commit().expect("commit WAL transaction");
        assert_eq!(pager.page_count(), 2);

        let committed = pager.read_page(page_number).expect("read committed page");
        assert!(
            committed
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x5a))
                .expect("inspect committed page")
        );
        drop(committed);
        drop(pager);

        let mut pager = Pager::open(&vfs, path, 4).expect("recover database from WAL");
        assert_eq!(pager.page_count(), 2);
        let recovered = pager.read_page(page_number).expect("read recovered page");
        assert!(
            recovered
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x5a))
                .expect("inspect recovered page")
        );
        drop(recovered);

        pager.checkpoint(&vfs).expect("checkpoint WAL");
        assert_eq!(pager.page_count(), 2);
        drop(pager);

        let mut pager = Pager::open(&vfs, path, 4).expect("reopen checkpointed database");
        let checkpointed = pager
            .read_page(page_number)
            .expect("read checkpointed page");
        assert!(
            checkpointed
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x5a))
                .expect("inspect checkpointed page")
        );
    }

    #[test]
    fn interrupted_checkpoint_recovers_from_the_still_committed_wal() {
        use ferrite_vfs::SimFaultKind;

        let vfs = SimVfs::new();
        let path = Path::new("pager-checkpoint-crash.fdb");
        let mut pager = Pager::create(&vfs, path, 2).expect("create database");
        let mut transaction = pager.begin_write();
        let page_number = transaction.allocate_page().expect("allocate page");
        let mut page = Page::zeroed();
        page.as_mut_bytes().fill(0x9a);
        transaction
            .write_page(page_number, &page)
            .expect("stage page");
        transaction.commit().expect("commit WAL transaction");
        vfs.inject_torn_write(0, 29)
            .expect("configure torn checkpoint write");

        let checkpoint = pager.checkpoint(&vfs);
        assert!(
            matches!(
                &checkpoint,
                Err(PagerError::Wal(WalError::Vfs(VfsError::InjectedFault(
                    SimFaultKind::PowerLoss
                ))))
            ),
            "unexpected checkpoint result: {checkpoint:?}"
        );
        drop(pager);
        vfs.clear_fault().expect("restore simulated storage");

        let mut reopened = Pager::open(&vfs, path, 2).expect("recover interrupted checkpoint");
        assert_eq!(reopened.page_count(), 2);
        let recovered = reopened
            .read_page(page_number)
            .expect("read page from committed WAL");
        assert!(
            recovered
                .with_bytes(|bytes| bytes.iter().all(|byte| *byte == 0x9a))
                .expect("inspect recovered page")
        );
        drop(recovered);
        reopened
            .checkpoint(&vfs)
            .expect("complete checkpoint after recovery");
    }

    #[test]
    fn dropping_a_pager_transaction_rolls_back_staged_pages() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-rollback.fdb");
        let mut pager = Pager::create(&vfs, path, 2).expect("create database");
        {
            let mut transaction = pager.begin_write();
            let page_number = transaction.allocate_page().expect("allocate page");
            let mut page = Page::zeroed();
            page.as_mut_bytes().fill(0xee);
            transaction
                .write_page(page_number, &page)
                .expect("stage page");
        }
        assert_eq!(pager.page_count(), 1);
        drop(pager);

        let mut reopened = Pager::open(&vfs, path, 2).expect("reopen database");
        assert_eq!(reopened.page_count(), 1);
        assert!(matches!(
            reopened.read_page(PageNumber::new(2).expect("valid page number")),
            Err(PagerError::PageOutOfRange {
                page_number: 2,
                page_count: 1
            })
        ));
    }

    #[test]
    fn torn_wal_commit_is_discarded_on_reopen() {
        let vfs = SimVfs::new();
        let path = Path::new("pager-torn-commit.fdb");
        let mut pager = Pager::create(&vfs, path, 2).expect("create database");
        let mut transaction = pager.begin_write();
        let page_number = transaction.allocate_page().expect("allocate page");
        let page = Page::zeroed();
        transaction
            .write_page(page_number, &page)
            .expect("stage page");
        vfs.inject_torn_write(2, 7)
            .expect("configure torn WAL write");

        let commit = transaction.commit();
        assert!(
            matches!(&commit, Err(PagerError::CommitUncertain(_))),
            "unexpected commit result: {commit:?}"
        );
        drop(pager);
        vfs.clear_fault().expect("restore simulated storage");

        let mut reopened = Pager::open(&vfs, path, 2).expect("recover torn transaction");
        assert_eq!(reopened.page_count(), 1);
        assert!(matches!(
            reopened.read_page(page_number),
            Err(PagerError::PageOutOfRange {
                page_number: 2,
                page_count: 1
            })
        ));
    }

    #[test]
    fn pager_rejects_commits_while_cached_pages_are_pinned() {
        let vfs = SimVfs::new();
        let mut pager =
            Pager::create(&vfs, Path::new("pinned-commit.fdb"), 2).expect("create database");
        let pinned = pager
            .read_page(PageNumber::new(1).expect("valid page number"))
            .expect("pin root page");
        let mut transaction = pager.begin_write();
        let page_number = transaction.allocate_page().expect("allocate page");
        let page = Page::zeroed();
        transaction
            .write_page(page_number, &page)
            .expect("stage page");

        assert!(matches!(
            transaction.commit(),
            Err(PagerError::CachePagesPinned)
        ));
        assert_eq!(pager.page_count(), 1);
        drop(pinned);
    }

    #[test]
    fn rejects_page_zero_and_out_of_range_reads() {
        assert!(matches!(
            PageNumber::new(0),
            Err(PagerError::InvalidPageNumber)
        ));

        let vfs = SimVfs::new();
        let mut pager =
            Pager::create(&vfs, Path::new("page-range.fdb"), 1).expect("create database");
        assert!(matches!(
            pager.read_page(PageNumber::new(2).expect("valid page number")),
            Err(PagerError::PageOutOfRange {
                page_number: 2,
                page_count: 1
            })
        ));
    }

    #[test]
    fn rejects_nonempty_and_misaligned_database_files() {
        let vfs = SimVfs::new();
        let path = Path::new("already-created.fdb");
        let pager = Pager::create(&vfs, path, 1).expect("create database");
        drop(pager);
        assert!(matches!(
            Pager::create(&vfs, path, 1),
            Err(PagerError::DatabaseAlreadyExists)
        ));

        let path = Path::new("misaligned.fdb");
        let mut file = vfs
            .open(
                path,
                OpenOptions {
                    read: true,
                    write: true,
                    create: true,
                    truncate: false,
                },
            )
            .expect("create malformed file");
        file.write_at(0, &[1]).expect("write malformed byte");
        assert!(matches!(
            Pager::open(&vfs, path, 1),
            Err(PagerError::MisalignedFileLength)
        ));
    }

    #[test]
    fn rejects_page_count_that_disagrees_with_file_length() {
        let vfs = SimVfs::new();
        let path = Path::new("page-count-mismatch.fdb");
        let pager = Pager::create(&vfs, path, 1).expect("create database");
        drop(pager);

        let mut file = vfs
            .open(
                path,
                OpenOptions {
                    read: true,
                    write: true,
                    create: false,
                    truncate: false,
                },
            )
            .expect("open database for corruption test");
        file.write_at(28, &2_u32.to_be_bytes())
            .expect("corrupt page count");
        assert!(matches!(
            Pager::open(&vfs, path, 1),
            Err(PagerError::PageCountMismatch {
                header_count: 2,
                file_count: 1
            })
        ));
    }

    #[test]
    fn cache_tracks_hits_and_evicts_only_unpinned_frames() {
        let vfs = SimVfs::new();
        let mut pager =
            Pager::create(&vfs, Path::new("cache-hit.fdb"), 1).expect("create database");
        let page_number = PageNumber::new(1).expect("valid page number");
        let pinned = pager.read_page(page_number).expect("cache miss");
        let hit = pager.read_page(page_number).expect("cache hit");
        assert_eq!(
            pager.cache_stats().expect("cache stats"),
            CacheStats {
                hits: 1,
                misses: 1,
                evictions: 0
            }
        );
        drop(hit);
        drop(pinned);
    }

    #[test]
    fn cache_rejects_zero_capacity_and_all_pinned_eviction() {
        assert!(matches!(
            PageCache::new(0),
            Err(PagerError::ZeroCacheCapacity)
        ));
        assert!(matches!(
            PageCache::new(usize::MAX),
            Err(PagerError::CacheAllocation)
        ));

        let cache = PageCache::new(1).expect("create cache");
        let first = cache
            .insert(PageNumber::new(1).expect("page 1"), Page::zeroed())
            .expect("insert page 1");
        assert!(matches!(
            cache.insert(PageNumber::new(2).expect("page 2"), Page::zeroed()),
            Err(PagerError::CacheFullPinned)
        ));
        drop(first);

        let second = cache
            .insert(PageNumber::new(2).expect("page 2"), Page::zeroed())
            .expect("insert page 2 after unpin");
        assert_eq!(second.page_number().get(), 2);
        assert_eq!(cache.stats().expect("cache stats").evictions, 1);
    }
}
