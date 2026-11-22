use ferrite_btree::Value;
use ferrite_vdbe::{SqlBackend, SqlBackendError, SqlTransaction, VdbeError};
use ferritedb_core::{CommitChange, CommitHook, Database, DatabaseError, DatabaseTransaction};
use redis::{Client, Connection, Script};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

const READ_STATE_SCRIPT: &str = r#"
local value = redis.call('GET', KEYS[1])
local dirty = redis.call('GET', KEYS[2])
return {value or false, dirty or false}
"#;

const MARK_DIRTY_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
redis.call('SET', KEYS[1], ARGV[1])
return current or false
"#;

const APPLY_CHANGE_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[2])
if current and current ~= ARGV[1] then
    return 0
end
if ARGV[2] == '1' then
    redis.call('SET', KEYS[1], ARGV[3])
else
    redis.call('DEL', KEYS[1])
end
if current == ARGV[1] then
    redis.call('DEL', KEYS[2])
end
return 1
"#;

const RESTORE_DIRTY_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    if ARGV[2] == '' then
        redis.call('DEL', KEYS[1])
    else
        redis.call('SET', KEYS[1], ARGV[2])
    end
end
return 1
"#;

const REFRESH_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[2])
local expected = ARGV[1]
if (current or '') ~= expected then
    return 0
end
if ARGV[2] == '1' then
    redis.call('SET', KEYS[1], ARGV[3])
else
    redis.call('DEL', KEYS[1])
end
if current then
    redis.call('DEL', KEYS[2])
end
return 1
"#;

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum CacheError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("cache backend error: {0}")]
    Backend(String),
    #[error("cache backend lock was poisoned")]
    Poisoned,
    #[error("cache state changed concurrently; retry the read")]
    ConcurrentChange,
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheState {
    pub record: Option<Vec<u8>>,
    pub dirty_token: Option<String>,
}

pub trait CacheClient: Send {
    fn read_state(&mut self, data_key: &str, dirty_key: &str) -> Result<CacheState, String>;
    fn mark_dirty(&mut self, dirty_key: &str, token: &str) -> Result<Option<String>, String>;
    fn apply_change(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        token: &str,
        record: Option<&[u8]>,
    ) -> Result<bool, String>;
    fn restore_dirty(
        &mut self,
        dirty_key: &str,
        token: &str,
        previous: Option<&str>,
    ) -> Result<(), String>;
    fn refresh_if_unchanged(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        expected_token: Option<&str>,
        record: Option<&[u8]>,
    ) -> Result<bool, String>;
}

pub struct RedisValkey {
    connection: Connection,
}

impl RedisValkey {
    pub fn connect(url: &str) -> Result<Self, CacheError> {
        let client = Client::open(url)?;
        Ok(Self {
            connection: client.get_connection()?,
        })
    }
}

impl CacheClient for RedisValkey {
    fn read_state(&mut self, data_key: &str, dirty_key: &str) -> Result<CacheState, String> {
        let state: (Option<Vec<u8>>, Option<String>) = Script::new(READ_STATE_SCRIPT)
            .key(data_key)
            .key(dirty_key)
            .invoke(&mut self.connection)
            .map_err(|error| error.to_string())?;
        Ok(CacheState {
            record: state.0,
            dirty_token: state.1,
        })
    }

    fn mark_dirty(&mut self, dirty_key: &str, token: &str) -> Result<Option<String>, String> {
        Script::new(MARK_DIRTY_SCRIPT)
            .key(dirty_key)
            .arg(token)
            .invoke(&mut self.connection)
            .map_err(|error| error.to_string())
    }

    fn apply_change(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        token: &str,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        let script = Script::new(APPLY_CHANGE_SCRIPT);
        script.key(data_key).key(dirty_key).arg(token);
        if let Some(record) = record {
            script.arg(1).arg(record);
        } else {
            script.arg(0).arg(&b""[..]);
        }
        let applied: i64 = script
            .invoke(&mut self.connection)
            .map_err(|error| error.to_string())?;
        Ok(applied == 1)
    }

    fn restore_dirty(
        &mut self,
        dirty_key: &str,
        token: &str,
        previous: Option<&str>,
    ) -> Result<(), String> {
        Script::new(RESTORE_DIRTY_SCRIPT)
            .key(dirty_key)
            .arg(token)
            .arg(previous.unwrap_or(""))
            .invoke::<i64>(&mut self.connection)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn refresh_if_unchanged(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        expected_token: Option<&str>,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        let script = Script::new(REFRESH_SCRIPT);
        script.key(data_key).key(dirty_key);
        script.arg(expected_token.unwrap_or(""));
        if let Some(record) = record {
            script.arg(1).arg(record);
        } else {
            script.arg(0).arg(&b""[..]);
        }
        let refreshed: i64 = script
            .invoke(&mut self.connection)
            .map_err(|error| error.to_string())?;
        Ok(refreshed == 1)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryValkey {
    values: HashMap<String, Vec<u8>>,
    dirty: HashMap<String, String>,
    fail_next_apply: bool,
}

impl InMemoryValkey {
    pub fn fail_next_apply(&mut self) {
        self.fail_next_apply = true;
    }

    pub fn value(&self, data_key: &str) -> Option<&[u8]> {
        self.values.get(data_key).map(Vec::as_slice)
    }

    pub fn dirty_token(&self, dirty_key: &str) -> Option<&str> {
        self.dirty.get(dirty_key).map(String::as_str)
    }
}

impl CacheClient for InMemoryValkey {
    fn read_state(&mut self, data_key: &str, dirty_key: &str) -> Result<CacheState, String> {
        Ok(CacheState {
            record: self.values.get(data_key).cloned(),
            dirty_token: self.dirty.get(dirty_key).cloned(),
        })
    }

    fn mark_dirty(&mut self, dirty_key: &str, token: &str) -> Result<Option<String>, String> {
        let previous = self.dirty.get(dirty_key).cloned();
        self.dirty.insert(dirty_key.to_owned(), token.to_owned());
        Ok(previous)
    }

    fn apply_change(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        token: &str,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        if std::mem::take(&mut self.fail_next_apply) {
            return Err("injected post-commit cache failure".to_owned());
        }
        if self
            .dirty
            .get(dirty_key)
            .is_some_and(|current| current != token)
        {
            return Ok(false);
        }
        match record {
            Some(record) => {
                self.values.insert(data_key.to_owned(), record.to_vec());
            }
            None => {
                self.values.remove(data_key);
            }
        }
        if self
            .dirty
            .get(dirty_key)
            .is_some_and(|current| current == token)
        {
            self.dirty.remove(dirty_key);
        }
        Ok(true)
    }

    fn restore_dirty(
        &mut self,
        dirty_key: &str,
        token: &str,
        previous: Option<&str>,
    ) -> Result<(), String> {
        if self
            .dirty
            .get(dirty_key)
            .is_some_and(|current| current == token)
        {
            match previous {
                Some(previous) => {
                    self.dirty.insert(dirty_key.to_owned(), previous.to_owned());
                }
                None => {
                    self.dirty.remove(dirty_key);
                }
            }
        }
        Ok(())
    }

    fn refresh_if_unchanged(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        expected_token: Option<&str>,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        if self.dirty.get(dirty_key).map(String::as_str) != expected_token {
            return Ok(false);
        }
        match record {
            Some(record) => {
                self.values.insert(data_key.to_owned(), record.to_vec());
            }
            None => {
                self.values.remove(data_key);
            }
        }
        if expected_token.is_some() {
            self.dirty.remove(dirty_key);
        }
        Ok(true)
    }
}

pub struct CachedDatabase<C> {
    database: Database,
    client: Arc<Mutex<C>>,
    namespace: String,
}

impl<C: CacheClient + 'static> CachedDatabase<C> {
    pub fn new(database: Database, client: C, namespace: impl Into<String>) -> Self {
        Self::with_client_handle(database, Arc::new(Mutex::new(client)), namespace)
    }

    pub fn with_client_handle(
        database: Database,
        client: Arc<Mutex<C>>,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            database,
            client,
            namespace: namespace.into(),
        }
    }

    pub fn client_handle(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.client)
    }

    pub fn begin_transaction(&mut self) -> CachedTransaction<'_> {
        let token = format!(
            "{}-{}",
            std::process::id(),
            NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
        );
        let mut transaction = self.database.begin_transaction();
        transaction.set_commit_hook(Box::new(CacheCommitHook {
            client: Arc::clone(&self.client),
            namespace: self.namespace.clone(),
            token,
            previous_dirty: HashMap::new(),
        }));
        CachedTransaction {
            transaction: Some(transaction),
        }
    }

    pub fn get(&mut self, table_name: &str, rowid: i64) -> Result<Option<Vec<u8>>, CacheError> {
        let client = Arc::clone(&self.client);
        let namespace = self.namespace.clone();
        let database = &mut self.database;
        cached_read(&client, &namespace, table_name, rowid, || {
            database
                .get_from(table_name, rowid)
                .map_err(CacheError::from)
        })
    }

    pub fn cache_state(&self, table_name: &str, rowid: i64) -> Result<CacheState, CacheError> {
        let (data_key, dirty_key) = cache_keys(&self.namespace, table_name, rowid);
        lock(&self.client)?
            .read_state(&data_key, &dirty_key)
            .map_err(CacheError::Backend)
    }
}

pub struct CachedTransaction<'a> {
    transaction: Option<DatabaseTransaction<'a>>,
}

impl CachedTransaction<'_> {
    pub fn insert(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), CacheError> {
        self.transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .insert_into(table_name, rowid, values)?;
        Ok(())
    }

    pub fn update(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), CacheError> {
        self.transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .update_in(table_name, rowid, values)?;
        Ok(())
    }

    pub fn delete(&mut self, table_name: &str, rowid: i64) -> Result<bool, CacheError> {
        Ok(self
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .delete_from(table_name, rowid)?)
    }

    pub fn commit(mut self) -> Result<(), CacheError> {
        self.transaction
            .take()
            .ok_or(CacheError::ConcurrentChange)?
            .commit()?;
        Ok(())
    }

    pub fn rollback(mut self) -> Result<(), CacheError> {
        self.transaction
            .take()
            .ok_or(CacheError::ConcurrentChange)?
            .rollback()?;
        Ok(())
    }
}

impl<C: CacheClient + 'static> SqlBackend for CachedDatabase<C> {
    type Error = CacheError;
    type Transaction<'a> = CachedSqlTransaction<'a, C>;

    fn table_columns(&mut self, table: &str) -> Result<Vec<String>, Self::Error> {
        Ok(self.database.table_columns(table)?)
    }

    fn indexes(&mut self) -> Result<Vec<ferritedb_core::IndexInfo>, Self::Error> {
        Ok(self.database.indexes()?)
    }

    fn next_rowid(&mut self, table: &str) -> Result<i64, Self::Error> {
        Ok(self.database.next_rowid(table)?)
    }

    fn begin_transaction(&mut self) -> Self::Transaction<'_> {
        let client = Arc::clone(&self.client);
        let namespace = self.namespace.clone();
        CachedSqlTransaction {
            transaction: CachedDatabase::begin_transaction(self),
            client,
            namespace,
        }
    }
}

impl SqlBackendError for CacheError {
    fn into_vdbe_error(self) -> VdbeError {
        VdbeError::Backend(self.to_string())
    }
}

pub struct CachedSqlTransaction<'a, C> {
    transaction: CachedTransaction<'a>,
    client: Arc<Mutex<C>>,
    namespace: String,
}

impl<C: CacheClient + 'static> CachedSqlTransaction<'_, C> {
    fn get_from(&mut self, table: &str, rowid: i64) -> Result<Option<Vec<u8>>, CacheError> {
        let client = Arc::clone(&self.client);
        let namespace = self.namespace.clone();
        let transaction = self
            .transaction
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?;
        cached_read(&client, &namespace, table, rowid, || {
            transaction.get_from(table, rowid).map_err(CacheError::from)
        })
    }
}

impl<C: CacheClient + 'static> SqlTransaction for CachedSqlTransaction<'_, C> {
    type Error = CacheError;

    fn create_table(&mut self, table: &str, columns: &[&str]) -> Result<(), Self::Error> {
        self.transaction
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .create_table(table, columns)?;
        Ok(())
    }

    fn create_index_on(
        &mut self,
        name: &str,
        table: &str,
        columns: &[usize],
    ) -> Result<(), Self::Error> {
        self.transaction
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .create_index_on(name, table, columns)?;
        Ok(())
    }

    fn scan_table(&mut self, table: &str) -> Result<Vec<(i64, Vec<u8>)>, Self::Error> {
        let rows = self
            .transaction
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .scan_table(table)?;
        let mut cached = Vec::with_capacity(rows.len());
        for (rowid, _) in rows {
            if let Some(record) = self.get_from(table, rowid)? {
                cached.push((rowid, record));
            }
        }
        Ok(cached)
    }

    fn find_index_on(
        &mut self,
        name: &str,
        table: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, Self::Error> {
        Ok(self
            .transaction
            .transaction
            .as_mut()
            .ok_or(CacheError::ConcurrentChange)?
            .find_index_on(name, table, values)?)
    }

    fn get_from(&mut self, table: &str, rowid: i64) -> Result<Option<Vec<u8>>, Self::Error> {
        CachedSqlTransaction::get_from(self, table, rowid)
    }

    fn insert_into(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), Self::Error> {
        self.transaction.insert(table, rowid, values)
    }

    fn update_in(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), Self::Error> {
        self.transaction.update(table, rowid, values)
    }

    fn delete_from(&mut self, table: &str, rowid: i64) -> Result<bool, Self::Error> {
        self.transaction.delete(table, rowid)
    }

    fn commit(self) -> Result<(), Self::Error> {
        self.transaction.commit()
    }
}

struct CacheCommitHook<C> {
    client: Arc<Mutex<C>>,
    namespace: String,
    token: String,
    previous_dirty: HashMap<String, Option<String>>,
}

impl<C: CacheClient + 'static> CommitHook for CacheCommitHook<C> {
    fn before_commit(&mut self, changes: &[CommitChange]) -> Result<(), String> {
        let mut client = lock(&self.client).map_err(|error| error.to_string())?;
        for change in changes {
            let (_, dirty_key) = cache_keys(&self.namespace, &change.table_name, change.rowid);
            let previous = client.mark_dirty(&dirty_key, &self.token)?;
            self.previous_dirty.insert(dirty_key, previous);
        }
        Ok(())
    }

    fn after_commit(&mut self, changes: &[CommitChange]) -> Result<(), String> {
        let mut client = lock(&self.client).map_err(|error| error.to_string())?;
        for change in changes {
            let (data_key, dirty_key) =
                cache_keys(&self.namespace, &change.table_name, change.rowid);
            if !client.apply_change(&data_key, &dirty_key, &self.token, change.record.as_deref())? {
                return Err("dirty marker changed before atomic cache update".to_owned());
            }
        }
        Ok(())
    }

    fn after_rollback(&mut self, changes: &[CommitChange]) -> Result<(), String> {
        let mut client = lock(&self.client).map_err(|error| error.to_string())?;
        for change in changes {
            let (_, dirty_key) = cache_keys(&self.namespace, &change.table_name, change.rowid);
            if let Some(previous) = self.previous_dirty.get(&dirty_key) {
                client.restore_dirty(&dirty_key, &self.token, previous.as_deref())?;
            }
        }
        Ok(())
    }
}

fn cache_keys(namespace: &str, table_name: &str, rowid: i64) -> (String, String) {
    let tag = format!(
        "{{{}:{}:{rowid}}}",
        encode_key_part(namespace),
        encode_key_part(&table_name.to_ascii_lowercase())
    );
    (
        format!("ferritedb:{tag}:record"),
        format!("ferritedb:{tag}:dirty"),
    )
}

fn cached_read<C: CacheClient>(
    client: &Arc<Mutex<C>>,
    namespace: &str,
    table_name: &str,
    rowid: i64,
    mut load: impl FnMut() -> Result<Option<Vec<u8>>, CacheError>,
) -> Result<Option<Vec<u8>>, CacheError> {
    let (data_key, dirty_key) = cache_keys(namespace, table_name, rowid);
    for _ in 0..3 {
        let state = lock(client)?
            .read_state(&data_key, &dirty_key)
            .map_err(CacheError::Backend)?;
        if state.dirty_token.is_none()
            && let Some(record) = state.record
        {
            return Ok(Some(record));
        }
        let record = load()?;
        let refreshed = lock(client)?
            .refresh_if_unchanged(
                &data_key,
                &dirty_key,
                state.dirty_token.as_deref(),
                record.as_deref(),
            )
            .map_err(CacheError::Backend)?;
        if refreshed {
            return Ok(record);
        }
    }
    Err(CacheError::ConcurrentChange)
}

fn encode_key_part(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn lock<C>(client: &Arc<Mutex<C>>) -> Result<MutexGuard<'_, C>, CacheError> {
    client.lock().map_err(|_| CacheError::Poisoned)
}
