use std::collections::HashMap;
use std::fs::{File, OpenOptions as StdOpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid open options: {0}")]
    InvalidOpenOptions(&'static str),
    #[error("file lock is already held or conflicts with another handle")]
    LockBusy,
    #[error("file handle does not hold a lock")]
    LockNotHeld,
    #[error("too many shared locks are held for this simulated file")]
    LockCountOverflow,
    #[error("system clock is before the Unix epoch")]
    TimeUnavailable,
    #[error("random source failed: {0}")]
    Random(String),
    #[error("requested file size is not representable on this platform")]
    SizeOverflow,
    #[error("memory allocation failed")]
    Allocation,
    #[error("simulated VFS fault: {0:?}")]
    InjectedFault(SimFaultKind),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OpenOptions {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
}

impl OpenOptions {
    fn validate(self) -> Result<Self, VfsError> {
        if !self.read && !self.write {
            return Err(VfsError::InvalidOpenOptions(
                "at least one of read or write must be enabled",
            ));
        }
        if self.create && !self.write {
            return Err(VfsError::InvalidOpenOptions("create requires write access"));
        }
        if self.truncate && !self.write {
            return Err(VfsError::InvalidOpenOptions(
                "truncate requires write access",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimOperation {
    Open,
    Read,
    Write,
    Length,
    SetLen,
    Sync,
    Lock,
    Unlock,
    Clock,
    Random,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimFaultKind {
    Io,
    NoSpace,
    PowerLoss,
}

pub trait FileHandle: Send {
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VfsError>;
    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> Result<usize, VfsError>;
    fn len(&mut self) -> Result<u64, VfsError>;
    fn is_empty(&mut self) -> Result<bool, VfsError> {
        Ok(self.len()? == 0)
    }
    fn set_len(&mut self, length: u64) -> Result<(), VfsError>;
    fn sync_all(&mut self) -> Result<(), VfsError>;
    fn lock(&mut self, mode: LockMode) -> Result<(), VfsError>;
    fn unlock(&mut self) -> Result<(), VfsError>;
}

pub trait Vfs: Send + Sync {
    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError>;
    fn now_millis(&self) -> Result<u64, VfsError>;
    fn fill_random(&self, buffer: &mut [u8]) -> Result<(), VfsError>;
    fn identity(&self) -> usize {
        self as *const Self as *const () as usize
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OsVfs;

static OS_VFS_IDENTITY: u8 = 0;

struct OsFileHandle {
    file: File,
    lock_mode: Option<LockMode>,
}

impl Vfs for OsVfs {
    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError> {
        let options = options.validate()?;
        let file = StdOpenOptions::new()
            .read(options.read)
            .write(options.write)
            .create(options.create)
            .truncate(options.truncate)
            .open(path)?;
        Ok(Box::new(OsFileHandle {
            file,
            lock_mode: None,
        }))
    }

    fn identity(&self) -> usize {
        &OS_VFS_IDENTITY as *const u8 as usize
    }

    fn now_millis(&self) -> Result<u64, VfsError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VfsError::TimeUnavailable)?;
        u64::try_from(duration.as_millis()).map_err(|_| VfsError::SizeOverflow)
    }

    fn fill_random(&self, buffer: &mut [u8]) -> Result<(), VfsError> {
        getrandom::fill(buffer).map_err(|error| VfsError::Random(error.to_string()))
    }
}

impl FileHandle for OsFileHandle {
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VfsError> {
        self.file.seek(SeekFrom::Start(offset))?;
        Ok(self.file.read(buffer)?)
    }

    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> Result<usize, VfsError> {
        self.file.seek(SeekFrom::Start(offset))?;
        Ok(self.file.write(buffer)?)
    }

    fn len(&mut self) -> Result<u64, VfsError> {
        Ok(self.file.metadata()?.len())
    }

    fn set_len(&mut self, length: u64) -> Result<(), VfsError> {
        self.file.set_len(length)?;
        Ok(())
    }

    fn sync_all(&mut self) -> Result<(), VfsError> {
        self.file.sync_all()?;
        Ok(())
    }

    fn lock(&mut self, mode: LockMode) -> Result<(), VfsError> {
        if self.lock_mode.is_some() {
            return Err(VfsError::LockBusy);
        }
        match mode {
            LockMode::Shared => fs2::FileExt::try_lock_shared(&self.file),
            LockMode::Exclusive => fs2::FileExt::try_lock_exclusive(&self.file),
        }
        .map_err(|error| {
            if is_lock_conflict(&error) {
                VfsError::LockBusy
            } else {
                VfsError::Io(error)
            }
        })?;
        self.lock_mode = Some(mode);
        Ok(())
    }

    fn unlock(&mut self) -> Result<(), VfsError> {
        self.lock_mode.ok_or(VfsError::LockNotHeld)?;
        fs2::FileExt::unlock(&self.file)?;
        self.lock_mode = None;
        Ok(())
    }
}

#[cfg(windows)]
fn is_lock_conflict(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock || error.raw_os_error() == Some(33)
}

#[cfg(not(windows))]
fn is_lock_conflict(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

#[derive(Clone)]
pub struct SimVfs {
    state: Arc<Mutex<SimState>>,
}

struct SimState {
    files: HashMap<PathBuf, SimFile>,
    now_millis: u64,
    random_state: u64,
    fault: Option<FaultPlan>,
}

#[derive(Clone, Copy)]
enum FaultMode {
    Error(SimFaultKind),
    TornWrite(usize),
}

impl FaultMode {
    fn kind(self) -> SimFaultKind {
        match self {
            Self::Error(kind) => kind,
            Self::TornWrite(_) => SimFaultKind::PowerLoss,
        }
    }

    fn error(self) -> VfsError {
        VfsError::InjectedFault(self.kind())
    }
}

struct FaultPlan {
    operation: SimOperation,
    remaining_successes: usize,
    mode: FaultMode,
    tripped: bool,
}

impl SimState {
    fn before_operation(&mut self, operation: SimOperation) -> Result<Option<FaultMode>, VfsError> {
        let Some(fault) = self.fault.as_mut() else {
            return Ok(None);
        };
        if fault.tripped {
            return Err(fault.mode.error());
        }
        if fault.operation != operation {
            return Ok(None);
        }
        if fault.remaining_successes > 0 {
            fault.remaining_successes -= 1;
            return Ok(None);
        }
        fault.tripped = true;
        Ok(Some(fault.mode))
    }
}

#[derive(Default)]
struct SimFile {
    bytes: Vec<u8>,
    shared_locks: usize,
    exclusive_lock: bool,
}

struct SimFileHandle {
    path: PathBuf,
    state: Arc<Mutex<SimState>>,
    options: OpenOptions,
    lock_mode: Option<LockMode>,
}

impl SimVfs {
    pub fn new() -> Self {
        Self::with_seed(0x9e37_79b9_7f4a_7c15, 0)
    }

    pub fn with_seed(seed: u64, now_millis: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(SimState {
                files: HashMap::new(),
                now_millis,
                random_state: if seed == 0 {
                    0x9e37_79b9_7f4a_7c15
                } else {
                    seed
                },
                fault: None,
            })),
        }
    }

    pub fn inject_failure(
        &self,
        operation: SimOperation,
        successful_operations: usize,
        kind: SimFaultKind,
    ) -> Result<(), VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        state.fault = Some(FaultPlan {
            operation,
            remaining_successes: successful_operations,
            mode: FaultMode::Error(kind),
            tripped: false,
        });
        Ok(())
    }

    pub fn inject_torn_write(
        &self,
        successful_writes: usize,
        bytes_before_power_loss: usize,
    ) -> Result<(), VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        state.fault = Some(FaultPlan {
            operation: SimOperation::Write,
            remaining_successes: successful_writes,
            mode: FaultMode::TornWrite(bytes_before_power_loss),
            tripped: false,
        });
        Ok(())
    }

    pub fn clear_fault(&self) -> Result<(), VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        state.fault = None;
        Ok(())
    }
}

impl Default for SimVfs {
    fn default() -> Self {
        Self::new()
    }
}

impl Vfs for SimVfs {
    fn identity(&self) -> usize {
        Arc::as_ptr(&self.state) as usize
    }

    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError> {
        let options = options.validate()?;
        let path = path.to_path_buf();
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Open)? {
            return Err(fault.error());
        }
        if !state.files.contains_key(&path) && !options.create {
            return Err(VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            )));
        }
        let file = state.files.entry(path.clone()).or_default();
        if options.truncate {
            file.bytes.clear();
        }
        drop(state);
        Ok(Box::new(SimFileHandle {
            path,
            state: Arc::clone(&self.state),
            options,
            lock_mode: None,
        }))
    }

    fn now_millis(&self) -> Result<u64, VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Clock)? {
            return Err(fault.error());
        }
        Ok(state.now_millis)
    }

    fn fill_random(&self, buffer: &mut [u8]) -> Result<(), VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Random)? {
            return Err(fault.error());
        }
        for chunk in buffer.chunks_mut(8) {
            let mut value = state.random_state;
            value ^= value << 13;
            value ^= value >> 7;
            value ^= value << 17;
            state.random_state = value;
            let bytes = value.to_be_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

impl FileHandle for SimFileHandle {
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VfsError> {
        if !self.options.read {
            return Err(VfsError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "file was not opened for reading",
            )));
        }
        let offset = usize::try_from(offset).map_err(|_| VfsError::SizeOverflow)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Read)? {
            return Err(fault.error());
        }
        let file = state.files.get(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        if offset >= file.bytes.len() {
            return Ok(0);
        }
        let length = buffer.len().min(file.bytes.len() - offset);
        buffer[..length].copy_from_slice(&file.bytes[offset..offset + length]);
        Ok(length)
    }

    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> Result<usize, VfsError> {
        if !self.options.write {
            return Err(VfsError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "file was not opened for writing",
            )));
        }
        let offset = usize::try_from(offset).map_err(|_| VfsError::SizeOverflow)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        let fault = state.before_operation(SimOperation::Write)?;
        if let Some(FaultMode::Error(kind)) = fault {
            return Err(VfsError::InjectedFault(kind));
        }
        let write_length = match fault {
            Some(FaultMode::TornWrite(length)) => length.min(buffer.len()),
            _ => buffer.len(),
        };
        let write_end = offset
            .checked_add(write_length)
            .ok_or(VfsError::SizeOverflow)?;
        let file = state.files.get_mut(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        if write_end > file.bytes.len() {
            file.bytes
                .try_reserve(write_end - file.bytes.len())
                .map_err(|_| VfsError::Allocation)?;
            file.bytes.resize(write_end, 0);
        }
        file.bytes[offset..write_end].copy_from_slice(&buffer[..write_length]);
        if let Some(fault) = fault {
            return Err(fault.error());
        }
        Ok(buffer.len())
    }

    fn len(&mut self) -> Result<u64, VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Length)? {
            return Err(fault.error());
        }
        let file = state.files.get(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        u64::try_from(file.bytes.len()).map_err(|_| VfsError::SizeOverflow)
    }

    fn set_len(&mut self, length: u64) -> Result<(), VfsError> {
        if !self.options.write {
            return Err(VfsError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "file was not opened for writing",
            )));
        }
        let length = usize::try_from(length).map_err(|_| VfsError::SizeOverflow)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::SetLen)? {
            return Err(fault.error());
        }
        let file = state.files.get_mut(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        if length > file.bytes.len() {
            file.bytes
                .try_reserve(length - file.bytes.len())
                .map_err(|_| VfsError::Allocation)?;
        }
        file.bytes.resize(length, 0);
        Ok(())
    }

    fn sync_all(&mut self) -> Result<(), VfsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Sync)? {
            return Err(fault.error());
        }
        Ok(())
    }

    fn lock(&mut self, mode: LockMode) -> Result<(), VfsError> {
        if self.lock_mode.is_some() {
            return Err(VfsError::LockBusy);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Lock)? {
            return Err(fault.error());
        }
        let file = state.files.get_mut(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        match mode {
            LockMode::Shared if !file.exclusive_lock => {
                file.shared_locks = file
                    .shared_locks
                    .checked_add(1)
                    .ok_or(VfsError::LockCountOverflow)?;
            }
            LockMode::Exclusive if !file.exclusive_lock && file.shared_locks == 0 => {
                file.exclusive_lock = true;
            }
            _ => return Err(VfsError::LockBusy),
        }
        self.lock_mode = Some(mode);
        Ok(())
    }

    fn unlock(&mut self) -> Result<(), VfsError> {
        let mode = self.lock_mode.ok_or(VfsError::LockNotHeld)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VfsError::Io(io::Error::other("SimVfs state lock was poisoned")))?;
        if let Some(fault) = state.before_operation(SimOperation::Unlock)? {
            return Err(fault.error());
        }
        let file = state.files.get_mut(&self.path).ok_or_else(|| {
            VfsError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "file does not exist",
            ))
        })?;
        match mode {
            LockMode::Shared => file.shared_locks -= 1,
            LockMode::Exclusive => file.exclusive_lock = false,
        }
        self.lock_mode = None;
        Ok(())
    }
}

impl Drop for SimFileHandle {
    fn drop(&mut self) {
        let Some(mode) = self.lock_mode else {
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(file) = state.files.get_mut(&self.path) else {
            return;
        };
        match mode {
            LockMode::Shared => file.shared_locks = file.shared_locks.saturating_sub(1),
            LockMode::Exclusive => file.exclusive_lock = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_write_create() -> OpenOptions {
        OpenOptions {
            read: true,
            write: true,
            create: true,
            truncate: false,
        }
    }

    #[test]
    fn sim_vfs_reads_and_writes_at_offsets() {
        let vfs = SimVfs::new();
        let mut file = vfs
            .open(Path::new("db.fdb"), read_write_create())
            .expect("open simulated file");
        assert_eq!(file.write_at(2, b"abc").expect("write"), 3);

        let mut bytes = [0; 5];
        assert_eq!(file.read_at(0, &mut bytes).expect("read"), 5);
        assert_eq!(&bytes, b"\0\0abc");
        assert_eq!(file.len().expect("length"), 5);
    }

    #[test]
    fn sim_vfs_enforces_file_locks() {
        let vfs = SimVfs::new();
        let path = Path::new("locked.fdb");
        let mut first = vfs.open(path, read_write_create()).expect("first open");
        let mut second = vfs.open(path, read_write_create()).expect("second open");

        first.lock(LockMode::Exclusive).expect("exclusive lock");
        assert!(matches!(
            second.lock(LockMode::Shared),
            Err(VfsError::LockBusy)
        ));
        first.unlock().expect("unlock");
        second.lock(LockMode::Shared).expect("shared lock");
    }

    #[test]
    fn sim_vfs_releases_locks_when_a_handle_is_dropped() {
        let vfs = SimVfs::new();
        let path = Path::new("dropped-lock.fdb");
        let mut reader = vfs.open(path, read_write_create()).expect("open reader");
        reader.lock(LockMode::Shared).expect("shared lock");
        drop(reader);

        let mut writer = vfs.open(path, read_write_create()).expect("open writer");
        writer
            .lock(LockMode::Exclusive)
            .expect("exclusive lock after reader closes");
    }

    #[test]
    fn sim_vfs_has_deterministic_time_and_randomness() {
        let first = SimVfs::with_seed(7, 1234);
        let second = SimVfs::with_seed(7, 1234);
        assert_eq!(first.now_millis().expect("time"), 1234);

        let mut first_bytes = [0; 16];
        let mut second_bytes = [0; 16];
        first.fill_random(&mut first_bytes).expect("random bytes");
        second.fill_random(&mut second_bytes).expect("random bytes");
        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn sim_vfs_zero_seed_still_produces_nonzero_deterministic_bytes() {
        let vfs = SimVfs::with_seed(0, 0);
        let mut bytes = [0; 16];
        vfs.fill_random(&mut bytes).expect("random bytes");
        assert_ne!(bytes, [0; 16]);
    }

    #[test]
    fn rejects_open_options_without_access() {
        let error = SimVfs::new()
            .open(Path::new("invalid.fdb"), OpenOptions::default())
            .err()
            .expect("invalid options should fail");
        assert!(matches!(error, VfsError::InvalidOpenOptions(_)));
    }

    #[test]
    fn sim_vfs_injects_persistent_operation_failures_until_cleared() {
        let vfs = SimVfs::new();
        let mut file = vfs
            .open(Path::new("faulted.fdb"), read_write_create())
            .expect("open simulated file");
        vfs.inject_failure(SimOperation::Write, 1, SimFaultKind::NoSpace)
            .expect("configure write fault");

        assert_eq!(file.write_at(0, b"one").expect("first write"), 3);
        assert!(matches!(
            file.write_at(3, b"two"),
            Err(VfsError::InjectedFault(SimFaultKind::NoSpace))
        ));
        assert!(matches!(
            file.len(),
            Err(VfsError::InjectedFault(SimFaultKind::NoSpace))
        ));

        vfs.clear_fault().expect("clear write fault");
        let mut bytes = [0; 3];
        assert_eq!(file.read_at(0, &mut bytes).expect("read after reset"), 3);
        assert_eq!(&bytes, b"one");
    }

    #[test]
    fn sim_vfs_torn_write_preserves_only_the_written_prefix() {
        let vfs = SimVfs::new();
        let mut file = vfs
            .open(Path::new("torn.fdb"), read_write_create())
            .expect("open simulated file");
        file.write_at(0, b"original").expect("initial write");
        vfs.inject_torn_write(0, 3).expect("configure torn write");

        assert!(matches!(
            file.write_at(0, b"replacement"),
            Err(VfsError::InjectedFault(SimFaultKind::PowerLoss))
        ));
        vfs.clear_fault().expect("clear torn-write fault");
        let mut bytes = [0; 8];
        assert_eq!(file.read_at(0, &mut bytes).expect("read after reset"), 8);
        assert_eq!(&bytes, b"repginal");
    }

    #[test]
    fn sim_vfs_can_inject_sync_failures() {
        let vfs = SimVfs::new();
        let mut file = vfs
            .open(Path::new("sync.fdb"), read_write_create())
            .expect("open simulated file");
        vfs.inject_failure(SimOperation::Sync, 0, SimFaultKind::Io)
            .expect("configure sync fault");

        assert!(matches!(
            file.sync_all(),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
    }

    #[test]
    fn sim_vfs_can_inject_faults_for_each_io_operation() {
        let vfs = SimVfs::new();
        vfs.inject_failure(SimOperation::Open, 0, SimFaultKind::Io)
            .expect("configure open fault");
        assert!(matches!(
            vfs.open(Path::new("all-ops.fdb"), read_write_create()),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear open fault");
        let mut file = vfs
            .open(Path::new("all-ops.fdb"), read_write_create())
            .expect("open simulated file");

        vfs.inject_failure(SimOperation::Read, 0, SimFaultKind::Io)
            .expect("configure read fault");
        assert!(matches!(
            file.read_at(0, &mut [0; 1]),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear read fault");

        vfs.inject_failure(SimOperation::Write, 0, SimFaultKind::Io)
            .expect("configure write fault");
        assert!(matches!(
            file.write_at(0, b"x"),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear write fault");

        vfs.inject_failure(SimOperation::Length, 0, SimFaultKind::Io)
            .expect("configure length fault");
        assert!(matches!(
            file.len(),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear length fault");

        vfs.inject_failure(SimOperation::SetLen, 0, SimFaultKind::Io)
            .expect("configure set-length fault");
        assert!(matches!(
            file.set_len(0),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear set-length fault");

        vfs.inject_failure(SimOperation::Sync, 0, SimFaultKind::Io)
            .expect("configure sync fault");
        assert!(matches!(
            file.sync_all(),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear sync fault");

        vfs.inject_failure(SimOperation::Lock, 0, SimFaultKind::Io)
            .expect("configure lock fault");
        assert!(matches!(
            file.lock(LockMode::Shared),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear lock fault");
        file.lock(LockMode::Shared).expect("acquire shared lock");

        vfs.inject_failure(SimOperation::Unlock, 0, SimFaultKind::Io)
            .expect("configure unlock fault");
        assert!(matches!(
            file.unlock(),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear unlock fault");
        file.unlock().expect("release shared lock");

        vfs.inject_failure(SimOperation::Clock, 0, SimFaultKind::Io)
            .expect("configure clock fault");
        assert!(matches!(
            vfs.now_millis(),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
        vfs.clear_fault().expect("clear clock fault");

        vfs.inject_failure(SimOperation::Random, 0, SimFaultKind::Io)
            .expect("configure random fault");
        assert!(matches!(
            vfs.fill_random(&mut [0; 1]),
            Err(VfsError::InjectedFault(SimFaultKind::Io))
        ));
    }
}
