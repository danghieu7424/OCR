use ferrite_btree::{RecordRef, Value, ValueRef};
use ferritedb_core::{Database, DatabaseError, DatabaseTransaction, IndexInfo};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    fn as_value(&self) -> Value<'_> {
        match self {
            Self::Null => Value::Null,
            Self::Integer(value) => Value::Integer(*value),
            Self::Real(value) => Value::Real(*value),
            Self::Text(value) => Value::Text(value),
            Self::Blob(value) => Value::Blob(value),
        }
    }

    fn from_ref(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(value) => Self::Integer(value),
            ValueRef::Real(value) => Self::Real(value),
            ValueRef::Text(value) => Self::Text(value.to_owned()),
            ValueRef::Blob(value) => Self::Blob(value.to_vec()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Predicate {
    pub column: String,
    pub value: SqlValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpCode {
    CreateTable {
        table: String,
        columns: Vec<String>,
    },
    CreateIndex {
        name: String,
        table: String,
        columns: Vec<String>,
    },
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        values: Vec<SqlValue>,
    },
    Select {
        table: String,
        columns: Option<Vec<String>>,
        predicate: Option<Predicate>,
    },
    Update {
        table: String,
        assignments: Vec<(String, SqlValue)>,
        predicate: Predicate,
    },
    Delete {
        table: String,
        predicate: Predicate,
    },
    Halt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessPath {
    FullTableScan,
    IndexLookup { name: String, column: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub opcodes: Vec<OpCode>,
    pub access_path: AccessPath,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExecutionResult {
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<SqlValue>>,
    },
    CommandComplete {
        affected_rows: usize,
    },
}

pub trait SqlBackend {
    type Error: SqlBackendError;
    type Transaction<'a>: SqlTransaction<Error = Self::Error>
    where
        Self: 'a;

    fn table_columns(&mut self, table: &str) -> std::result::Result<Vec<String>, Self::Error>;
    fn indexes(&mut self) -> std::result::Result<Vec<IndexInfo>, Self::Error>;
    fn next_rowid(&mut self, table: &str) -> std::result::Result<i64, Self::Error>;
    fn begin_transaction(&mut self) -> Self::Transaction<'_>;
}

pub trait SqlTransaction {
    type Error: SqlBackendError;

    fn create_table(
        &mut self,
        table: &str,
        columns: &[&str],
    ) -> std::result::Result<(), Self::Error>;
    fn create_index_on(
        &mut self,
        name: &str,
        table: &str,
        columns: &[usize],
    ) -> std::result::Result<(), Self::Error>;
    fn scan_table(&mut self, table: &str) -> std::result::Result<Vec<(i64, Vec<u8>)>, Self::Error>;
    fn find_index_on(
        &mut self,
        name: &str,
        table: &str,
        values: &[Value<'_>],
    ) -> std::result::Result<Vec<i64>, Self::Error>;
    fn get_from(
        &mut self,
        table: &str,
        rowid: i64,
    ) -> std::result::Result<Option<Vec<u8>>, Self::Error>;
    fn insert_into(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> std::result::Result<(), Self::Error>;
    fn update_in(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> std::result::Result<(), Self::Error>;
    fn delete_from(&mut self, table: &str, rowid: i64) -> std::result::Result<bool, Self::Error>;
    fn commit(self) -> std::result::Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum VdbeError {
    Syntax { offset: usize, message: String },
    Database(DatabaseError),
    Record(ferrite_btree::BtreeError),
    UnknownColumn(String),
    DuplicateColumn(String),
    InsertArity { expected: usize, actual: usize },
    EmptyIndex,
    Backend(String),
}

impl fmt::Display for VdbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { offset, message } => {
                write!(formatter, "SQL syntax error at byte {offset}: {message}")
            }
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Record(error) => write!(formatter, "corrupt stored record: {error}"),
            Self::UnknownColumn(column) => write!(formatter, "column does not exist: {column}"),
            Self::DuplicateColumn(column) => write!(formatter, "duplicate column: {column}"),
            Self::InsertArity { expected, actual } => {
                write!(
                    formatter,
                    "INSERT has {actual} values for {expected} columns"
                )
            }
            Self::EmptyIndex => formatter.write_str("index column list must not be empty"),
            Self::Backend(error) => write!(formatter, "SQL backend error: {error}"),
        }
    }
}

impl std::error::Error for VdbeError {}

impl From<DatabaseError> for VdbeError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ferrite_btree::BtreeError> for VdbeError {
    fn from(error: ferrite_btree::BtreeError) -> Self {
        Self::Record(error)
    }
}

pub trait SqlBackendError: fmt::Display {
    fn into_vdbe_error(self) -> VdbeError;
}

impl SqlBackendError for DatabaseError {
    fn into_vdbe_error(self) -> VdbeError {
        VdbeError::Database(self)
    }
}

impl SqlBackend for Database {
    type Error = DatabaseError;
    type Transaction<'a> = DatabaseTransaction<'a>;

    fn table_columns(&mut self, table: &str) -> std::result::Result<Vec<String>, Self::Error> {
        Database::table_columns(self, table)
    }

    fn indexes(&mut self) -> std::result::Result<Vec<IndexInfo>, Self::Error> {
        Database::indexes(self)
    }

    fn next_rowid(&mut self, table: &str) -> std::result::Result<i64, Self::Error> {
        Database::next_rowid(self, table)
    }

    fn begin_transaction(&mut self) -> Self::Transaction<'_> {
        Database::begin_transaction(self)
    }
}

impl SqlTransaction for DatabaseTransaction<'_> {
    type Error = DatabaseError;

    fn create_table(
        &mut self,
        table: &str,
        columns: &[&str],
    ) -> std::result::Result<(), Self::Error> {
        DatabaseTransaction::create_table(self, table, columns)
    }

    fn create_index_on(
        &mut self,
        name: &str,
        table: &str,
        columns: &[usize],
    ) -> std::result::Result<(), Self::Error> {
        DatabaseTransaction::create_index_on(self, name, table, columns)
    }

    fn scan_table(&mut self, table: &str) -> std::result::Result<Vec<(i64, Vec<u8>)>, Self::Error> {
        DatabaseTransaction::scan_table(self, table)
    }

    fn find_index_on(
        &mut self,
        name: &str,
        table: &str,
        values: &[Value<'_>],
    ) -> std::result::Result<Vec<i64>, Self::Error> {
        DatabaseTransaction::find_index_on(self, name, table, values)
    }

    fn get_from(
        &mut self,
        table: &str,
        rowid: i64,
    ) -> std::result::Result<Option<Vec<u8>>, Self::Error> {
        DatabaseTransaction::get_from(self, table, rowid)
    }

    fn insert_into(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> std::result::Result<(), Self::Error> {
        DatabaseTransaction::insert_into(self, table, rowid, values)
    }

    fn update_in(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> std::result::Result<(), Self::Error> {
        DatabaseTransaction::update_in(self, table, rowid, values)
    }

    fn delete_from(&mut self, table: &str, rowid: i64) -> std::result::Result<bool, Self::Error> {
        DatabaseTransaction::delete_from(self, table, rowid)
    }

    fn commit(self) -> std::result::Result<(), Self::Error> {
        DatabaseTransaction::commit(self)
    }
}

type Result<T> = std::result::Result<T, VdbeError>;

pub fn compile(sql: &str) -> Result<Vec<OpCode>> {
    let statement = Parser::new(sql)?.parse_statement()?;
    Ok(vec![statement, OpCode::Halt])
}

pub fn plan<D: SqlBackend>(database: &mut D, sql: &str) -> Result<Plan> {
    let opcodes = compile(sql)?;
    let access_path = plan_opcodes(database, &opcodes)?;
    Ok(Plan {
        opcodes,
        access_path,
    })
}

fn plan_opcodes<D: SqlBackend>(database: &mut D, opcodes: &[OpCode]) -> Result<AccessPath> {
    let (table, predicate) = match opcodes.first() {
        Some(OpCode::Select {
            table,
            predicate: Some(predicate),
            ..
        })
        | Some(OpCode::Update {
            table, predicate, ..
        })
        | Some(OpCode::Delete { table, predicate }) => (table, predicate),
        _ => return Ok(AccessPath::FullTableScan),
    };
    let columns = backend(database.table_columns(table))?;
    let position = column_position(&columns, &predicate.column)?;
    Ok(backend(database.indexes())?
        .into_iter()
        .find(|index| {
            index.table_name.eq_ignore_ascii_case(table) && index.columns.as_slice() == [position]
        })
        .map(|index| AccessPath::IndexLookup {
            name: index.name,
            column: position,
        })
        .unwrap_or(AccessPath::FullTableScan))
}

pub fn execute<D: SqlBackend>(database: &mut D, opcodes: &[OpCode]) -> Result<ExecutionResult> {
    let path = plan_opcodes(database, opcodes)?;
    execute_with_path(database, opcodes, Some(&path))
}

pub fn execute_plan<D: SqlBackend>(database: &mut D, plan: &Plan) -> Result<ExecutionResult> {
    execute_with_path(database, &plan.opcodes, Some(&plan.access_path))
}

fn execute_with_path<D: SqlBackend>(
    database: &mut D,
    opcodes: &[OpCode],
    path: Option<&AccessPath>,
) -> Result<ExecutionResult> {
    let statement = opcodes
        .iter()
        .find(|opcode| !matches!(opcode, OpCode::Halt))
        .ok_or_else(|| syntax(0, "empty program"))?;
    match statement {
        OpCode::CreateTable { table, columns } => {
            let refs = columns.iter().map(String::as_str).collect::<Vec<_>>();
            let mut transaction = database.begin_transaction();
            backend(transaction.create_table(table, &refs))?;
            backend(transaction.commit())?;
            Ok(ExecutionResult::CommandComplete { affected_rows: 0 })
        }
        OpCode::CreateIndex {
            name,
            table,
            columns,
        } => {
            let table_columns = backend(database.table_columns(table))?;
            let positions = columns
                .iter()
                .map(|column| column_position(&table_columns, column))
                .collect::<Result<Vec<_>>>()?;
            if positions.is_empty() {
                return Err(VdbeError::EmptyIndex);
            }
            let mut transaction = database.begin_transaction();
            backend(transaction.create_index_on(name, table, &positions))?;
            backend(transaction.commit())?;
            Ok(ExecutionResult::CommandComplete { affected_rows: 0 })
        }
        OpCode::Insert {
            table,
            columns,
            values,
        } => {
            let table_columns = backend(database.table_columns(table))?;
            let (row, selected_columns) = if let Some(columns) = columns {
                validate_distinct(columns)?;
                let mut row = vec![SqlValue::Null; table_columns.len()];
                for (column, value) in columns.iter().zip(values) {
                    let position = column_position(&table_columns, column)?;
                    row[position] = value.clone();
                }
                (row, columns.len())
            } else {
                (values.clone(), table_columns.len())
            };
            if selected_columns != values.len() || row.len() != table_columns.len() {
                return Err(VdbeError::InsertArity {
                    expected: selected_columns,
                    actual: values.len(),
                });
            }
            let rowid = backend(database.next_rowid(table))?;
            let db_values = row.iter().map(SqlValue::as_value).collect::<Vec<_>>();
            let mut transaction = database.begin_transaction();
            backend(transaction.insert_into(table, rowid, &db_values))?;
            backend(transaction.commit())?;
            Ok(ExecutionResult::CommandComplete { affected_rows: 1 })
        }
        OpCode::Select {
            table,
            columns,
            predicate,
        } => {
            let table_columns = backend(database.table_columns(table))?;
            let projection = resolve_projection(&table_columns, columns.as_deref())?;
            if let Some(predicate) = predicate {
                column_position(&table_columns, &predicate.column)?;
            }
            let mut transaction = database.begin_transaction();
            let matching = match predicate {
                Some(predicate) => matching_rows(&mut transaction, table, predicate, path)?,
                None => backend(transaction.scan_table(table))?,
            };
            let mut rows = Vec::new();
            for (_, encoded) in matching {
                let record = RecordRef::new(&encoded)?;
                let values = record
                    .values()
                    .map(|value| value.map(SqlValue::from_ref))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if values.len() != table_columns.len() {
                    return Err(VdbeError::Record(ferrite_btree::BtreeError::InvalidRecord(
                        "record does not match table schema",
                    )));
                }
                if let Some(predicate) = predicate {
                    let position = column_position(&table_columns, &predicate.column)?;
                    if values.get(position) != Some(&predicate.value) {
                        continue;
                    }
                }
                rows.push(
                    projection
                        .iter()
                        .map(|position| values[*position].clone())
                        .collect(),
                );
            }
            Ok(ExecutionResult::Rows {
                columns: projection
                    .iter()
                    .map(|position| table_columns[*position].clone())
                    .collect(),
                rows,
            })
        }
        OpCode::Update {
            table,
            assignments,
            predicate,
        } => {
            let columns = backend(database.table_columns(table))?;
            let predicate_position = column_position(&columns, &predicate.column)?;
            let assignment_positions = assignments
                .iter()
                .map(|(name, value)| Ok((column_position(&columns, name)?, value)))
                .collect::<Result<Vec<_>>>()?;
            let mut transaction = database.begin_transaction();
            let rows = matching_rows(&mut transaction, table, predicate, path)?;
            let mut updates = Vec::new();
            for (rowid, encoded) in rows {
                let record = RecordRef::new(&encoded)?;
                let mut values = record
                    .values()
                    .map(|value| value.map(SqlValue::from_ref))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if values.get(predicate_position) == Some(&predicate.value) {
                    for (position, value) in &assignment_positions {
                        values[*position] = (*value).clone();
                    }
                    updates.push((rowid, values));
                }
            }
            for (rowid, values) in &updates {
                let db_values = values.iter().map(SqlValue::as_value).collect::<Vec<_>>();
                backend(transaction.update_in(table, *rowid, &db_values))?;
            }
            backend(transaction.commit())?;
            Ok(ExecutionResult::CommandComplete {
                affected_rows: updates.len(),
            })
        }
        OpCode::Delete { table, predicate } => {
            let columns = backend(database.table_columns(table))?;
            let position = column_position(&columns, &predicate.column)?;
            let mut transaction = database.begin_transaction();
            let rows = matching_rows(&mut transaction, table, predicate, path)?;
            let mut matches = Vec::new();
            for (rowid, encoded) in rows {
                let record = RecordRef::new(&encoded)?;
                if record.get(position)?.map(SqlValue::from_ref).as_ref() == Some(&predicate.value)
                {
                    matches.push(rowid);
                }
            }
            for rowid in &matches {
                backend(transaction.delete_from(table, *rowid))?;
            }
            backend(transaction.commit())?;
            Ok(ExecutionResult::CommandComplete {
                affected_rows: matches.len(),
            })
        }
        OpCode::Halt => unreachable!(),
    }
}

fn matching_rows<T: SqlTransaction>(
    transaction: &mut T,
    table: &str,
    predicate: &Predicate,
    path: Option<&AccessPath>,
) -> Result<Vec<(i64, Vec<u8>)>> {
    if let Some(AccessPath::IndexLookup { name, .. }) = path {
        let key = [predicate.value.as_value()];
        let rowids = backend(transaction.find_index_on(name, table, &key))?;
        let mut records = Vec::with_capacity(rowids.len());
        for rowid in rowids {
            if let Some(record) = backend(transaction.get_from(table, rowid))? {
                records.push((rowid, record));
            }
        }
        Ok(records)
    } else {
        Ok(backend(transaction.scan_table(table))?)
    }
}

fn backend<T, E: SqlBackendError>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(SqlBackendError::into_vdbe_error)
}

fn resolve_projection(columns: &[String], projection: Option<&[String]>) -> Result<Vec<usize>> {
    match projection {
        Some(names) => names
            .iter()
            .map(|name| column_position(columns, name))
            .collect(),
        None => Ok((0..columns.len()).collect()),
    }
}

fn column_position(columns: &[String], name: &str) -> Result<usize> {
    columns
        .iter()
        .position(|column| column.eq_ignore_ascii_case(name))
        .ok_or_else(|| VdbeError::UnknownColumn(name.to_owned()))
}

fn validate_distinct(columns: &[String]) -> Result<()> {
    for (index, column) in columns.iter().enumerate() {
        if columns[..index]
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(column))
        {
            return Err(VdbeError::DuplicateColumn(column.clone()));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum TokenKind {
    Ident(String),
    Literal(SqlValue),
    LParen,
    RParen,
    Comma,
    Star,
    Equal,
    Semicolon,
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    offset: usize,
}

fn syntax(offset: usize, message: impl Into<String>) -> VdbeError {
    VdbeError::Syntax {
        offset,
        message: message.into(),
    }
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    end: usize,
}

impl Parser {
    fn new(input: &str) -> Result<Self> {
        Ok(Self {
            tokens: tokenize(input)?,
            cursor: 0,
            end: input.len(),
        })
    }

    fn parse_statement(mut self) -> Result<OpCode> {
        let statement = if self.consume_keyword("CREATE") {
            self.parse_create()?
        } else if self.consume_keyword("INSERT") {
            self.parse_insert()?
        } else if self.consume_keyword("SELECT") {
            self.parse_select()?
        } else if self.consume_keyword("UPDATE") {
            self.parse_update()?
        } else if self.consume_keyword("DELETE") {
            self.parse_delete()?
        } else {
            return Err(self.error("expected CREATE, INSERT, SELECT, UPDATE, or DELETE"));
        };
        self.consume_kind(|kind| matches!(kind, TokenKind::Semicolon));
        if self.cursor != self.tokens.len() {
            return Err(self.error("unexpected trailing input"));
        }
        Ok(statement)
    }

    fn parse_create(&mut self) -> Result<OpCode> {
        if self.consume_keyword("TABLE") {
            let table = self.identifier()?;
            self.expect_kind(|kind| matches!(kind, TokenKind::LParen), "expected '('")?;
            let mut columns = Vec::new();
            loop {
                columns.push(self.identifier()?);
                if self.peek_keyword_any(&[
                    "INTEGER", "INT", "REAL", "FLOAT", "DOUBLE", "TEXT", "VARCHAR", "CHAR", "BLOB",
                    "BOOLEAN", "NUMERIC",
                ]) {
                    self.cursor += 1;
                    if self.consume_kind(|kind| matches!(kind, TokenKind::LParen)) {
                        self.expect_kind(
                            |kind| matches!(kind, TokenKind::Literal(SqlValue::Integer(_))),
                            "expected type size",
                        )?;
                        self.expect_kind(
                            |kind| matches!(kind, TokenKind::RParen),
                            "expected ')' after type size",
                        )?;
                    }
                }
                if !self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
            }
            self.expect_kind(|kind| matches!(kind, TokenKind::RParen), "expected ')'")?;
            validate_distinct(&columns)?;
            Ok(OpCode::CreateTable { table, columns })
        } else if self.consume_keyword("INDEX") {
            let name = self.identifier()?;
            self.expect_keyword("ON")?;
            let table = self.identifier()?;
            self.expect_kind(|kind| matches!(kind, TokenKind::LParen), "expected '('")?;
            let mut columns = vec![self.identifier()?];
            while self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                columns.push(self.identifier()?);
            }
            self.expect_kind(|kind| matches!(kind, TokenKind::RParen), "expected ')'")?;
            validate_distinct(&columns)?;
            Ok(OpCode::CreateIndex {
                name,
                table,
                columns,
            })
        } else {
            Err(self.error("expected TABLE or INDEX after CREATE"))
        }
    }

    fn parse_insert(&mut self) -> Result<OpCode> {
        self.expect_keyword("INTO")?;
        let table = self.identifier()?;
        let columns = if self.consume_kind(|kind| matches!(kind, TokenKind::LParen)) {
            let mut columns = vec![self.identifier()?];
            while self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                columns.push(self.identifier()?);
            }
            self.expect_kind(|kind| matches!(kind, TokenKind::RParen), "expected ')'")?;
            validate_distinct(&columns)?;
            Some(columns)
        } else {
            None
        };
        self.expect_keyword("VALUES")?;
        self.expect_kind(
            |kind| matches!(kind, TokenKind::LParen),
            "expected '(' after VALUES",
        )?;
        let mut values = vec![self.literal()?];
        while self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
            values.push(self.literal()?);
        }
        self.expect_kind(
            |kind| matches!(kind, TokenKind::RParen),
            "expected ')' after values",
        )?;
        Ok(OpCode::Insert {
            table,
            columns,
            values,
        })
    }

    fn parse_select(&mut self) -> Result<OpCode> {
        let columns = if self.consume_kind(|kind| matches!(kind, TokenKind::Star)) {
            None
        } else {
            let mut columns = vec![self.identifier()?];
            while self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                columns.push(self.identifier()?);
            }
            Some(columns)
        };
        self.expect_keyword("FROM")?;
        let table = self.identifier()?;
        let predicate = if self.consume_keyword("WHERE") {
            Some(self.predicate()?)
        } else {
            None
        };
        Ok(OpCode::Select {
            table,
            columns,
            predicate,
        })
    }

    fn parse_update(&mut self) -> Result<OpCode> {
        let table = self.identifier()?;
        self.expect_keyword("SET")?;
        let mut assignments = Vec::new();
        loop {
            let column = self.identifier()?;
            self.expect_kind(|kind| matches!(kind, TokenKind::Equal), "expected '='")?;
            assignments.push((column, self.literal()?));
            if !self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        validate_distinct(
            &assignments
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>(),
        )?;
        self.expect_keyword("WHERE")?;
        let predicate = self.predicate()?;
        Ok(OpCode::Update {
            table,
            assignments,
            predicate,
        })
    }

    fn parse_delete(&mut self) -> Result<OpCode> {
        self.expect_keyword("FROM")?;
        let table = self.identifier()?;
        self.expect_keyword("WHERE")?;
        Ok(OpCode::Delete {
            table,
            predicate: self.predicate()?,
        })
    }

    fn predicate(&mut self) -> Result<Predicate> {
        let column = self.identifier()?;
        self.expect_kind(
            |kind| matches!(kind, TokenKind::Equal),
            "expected '=' in predicate",
        )?;
        Ok(Predicate {
            column,
            value: self.literal()?,
        })
    }

    fn literal(&mut self) -> Result<SqlValue> {
        match self.next_kind() {
            Some(TokenKind::Literal(value)) => Ok(value),
            _ => Err(self.error("expected a literal value")),
        }
    }

    fn identifier(&mut self) -> Result<String> {
        match self.next_kind() {
            Some(TokenKind::Ident(value)) => Ok(value),
            _ => Err(self.error("expected a bare identifier")),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<()> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error(format!("expected {keyword}")))
        }
    }

    fn expect_kind(
        &mut self,
        predicate: impl FnOnce(&TokenKind) -> bool,
        message: &str,
    ) -> Result<()> {
        if self.consume_kind(predicate) {
            Ok(())
        } else {
            Err(self.error(message))
        }
    }

    fn peek_keyword_any(&self, keywords: &[&str]) -> bool {
        matches!(self.tokens.get(self.cursor).map(|token| &token.kind), Some(TokenKind::Ident(value)) if keywords.iter().any(|keyword| value.eq_ignore_ascii_case(keyword)))
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if matches!(self.tokens.get(self.cursor).map(|token| &token.kind), Some(TokenKind::Ident(value)) if value.eq_ignore_ascii_case(keyword))
        {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn consume_kind(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        if self
            .tokens
            .get(self.cursor)
            .is_some_and(|token| predicate(&token.kind))
        {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn next_kind(&mut self) -> Option<TokenKind> {
        let token = self.tokens.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token.kind)
    }

    fn error(&self, message: impl Into<String>) -> VdbeError {
        syntax(
            self.tokens
                .get(self.cursor)
                .map(|token| token.offset)
                .unwrap_or(self.end),
            message,
        )
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        let byte = bytes[index];
        let kind = match byte {
            b'(' => {
                index += 1;
                TokenKind::LParen
            }
            b')' => {
                index += 1;
                TokenKind::RParen
            }
            b',' => {
                index += 1;
                TokenKind::Comma
            }
            b'*' => {
                index += 1;
                TokenKind::Star
            }
            b'=' => {
                index += 1;
                TokenKind::Equal
            }
            b';' => {
                index += 1;
                TokenKind::Semicolon
            }
            b'\'' => {
                let (value, next) = parse_string(input, index)?;
                index = next;
                TokenKind::Literal(SqlValue::Text(value))
            }
            b'x' | b'X' if bytes.get(index + 1) == Some(&b'\'') => {
                let (blob, next) = parse_blob(input, index + 1)?;
                index = next;
                TokenKind::Literal(SqlValue::Blob(blob))
            }
            b'-' | b'+' | b'0'..=b'9' => {
                let (value, next) = parse_number(input, index)?;
                index = next;
                TokenKind::Literal(value)
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let value = &input[start..index];
                if value.eq_ignore_ascii_case("NULL") {
                    TokenKind::Literal(SqlValue::Null)
                } else {
                    TokenKind::Ident(value.to_owned())
                }
            }
            _ => return Err(syntax(index, "unexpected character")),
        };
        tokens.push(Token {
            kind,
            offset: start,
        });
    }
    Ok(tokens)
}

fn parse_string(input: &str, start: usize) -> Result<(String, usize)> {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\'' {
            if bytes.get(index + 1) == Some(&b'\'') {
                output.push('\'');
                index += 2;
            } else {
                return Ok((output, index + 1));
            }
        } else {
            let character = input[index..]
                .chars()
                .next()
                .ok_or_else(|| syntax(index, "invalid UTF-8 boundary"))?;
            output.push(character);
            index += character.len_utf8();
        }
    }
    Err(syntax(start, "unterminated string literal"))
}

fn parse_blob(input: &str, quote: usize) -> Result<(Vec<u8>, usize)> {
    let bytes = input.as_bytes();
    let mut index = quote + 1;
    let start = index;
    while index < bytes.len() && bytes[index] != b'\'' {
        if !bytes[index].is_ascii_hexdigit() {
            return Err(syntax(index, "blob literal contains a non-hex digit"));
        }
        index += 1;
    }
    if index == bytes.len() {
        return Err(syntax(quote, "unterminated blob literal"));
    }
    if !(index - start).is_multiple_of(2) {
        return Err(syntax(
            start,
            "blob literal must contain an even number of hex digits",
        ));
    }
    let mut blob = Vec::with_capacity((index - start) / 2);
    let (pairs, _) = bytes[start..index].as_chunks::<2>();
    for pair in pairs {
        let text = std::str::from_utf8(pair).map_err(|_| syntax(start, "invalid blob literal"))?;
        blob.push(u8::from_str_radix(text, 16).map_err(|_| syntax(start, "invalid blob literal"))?);
    }
    Ok((blob, index + 1))
}

fn parse_number(input: &str, start: usize) -> Result<(SqlValue, usize)> {
    let bytes = input.as_bytes();
    let mut index = start;
    if matches!(bytes[index], b'-' | b'+') {
        index += 1;
        if index == bytes.len() || !bytes[index].is_ascii_digit() {
            return Err(syntax(start, "expected digits after sign"));
        }
    }
    let integer_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if integer_start == index {
        return Err(syntax(start, "expected integer digits"));
    }
    let mut real = false;
    if bytes.get(index) == Some(&b'.') {
        real = true;
        index += 1;
        let fraction_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if fraction_start == index {
            return Err(syntax(start, "expected digits after decimal point"));
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        real = true;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if exponent_start == index {
            return Err(syntax(start, "expected exponent digits"));
        }
    }
    if index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        return Err(syntax(index, "invalid character after numeric literal"));
    }
    let source = &input[start..index];
    let value = if real {
        let number = source
            .parse::<f64>()
            .map_err(|_| syntax(start, "invalid real literal"))?;
        if !number.is_finite() {
            return Err(syntax(start, "real literal is out of range"));
        }
        SqlValue::Real(number)
    } else {
        SqlValue::Integer(
            source
                .parse::<i64>()
                .map_err(|_| syntax(start, "integer literal is out of range"))?,
        )
    };
    Ok((value, index))
}

impl fmt::Display for SqlValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("NULL"),
            Self::Integer(value) => value.fmt(formatter),
            Self::Real(value) => value.fmt(formatter),
            Self::Text(value) => formatter.write_str(value),
            Self::Blob(value) => write!(formatter, "{value:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_all_supported_forms_and_escapes_values() {
        assert!(matches!(
            compile("CREATE TABLE notes (id INTEGER, body TEXT)").unwrap()[0],
            OpCode::CreateTable { .. }
        ));
        assert!(matches!(
            compile("CREATE INDEX ix ON notes (id)").unwrap()[0],
            OpCode::CreateIndex { .. }
        ));
        assert_eq!(
            compile("INSERT INTO notes (id, body) VALUES (-2, 'it''s')").unwrap()[0],
            OpCode::Insert {
                table: "notes".into(),
                columns: Some(vec!["id".into(), "body".into()]),
                values: vec![SqlValue::Integer(-2), SqlValue::Text("it's".into())],
            }
        );
        assert_eq!(
            compile("INSERT INTO notes VALUES (+1, +2.5, NULL, X'00ff')")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn rejects_malformed_and_unsupported_sql_without_panicking() {
        for sql in [
            "",
            "SELECT FROM t",
            "SELECT * FROM t WHERE a > 1",
            "INSERT INTO t VALUES (1,)",
            "CREATE TABLE t (x,)",
            "CREATE TABLE t (x, X)",
            "DELETE FROM t",
            "UPDATE t SET x=1",
            "SELECT * FROM t; SELECT * FROM t",
            "INSERT INTO t VALUES (X'0')",
            "INSERT INTO t VALUES ('unterminated)",
        ] {
            assert!(compile(sql).is_err(), "accepted {sql:?}");
        }
    }
}
