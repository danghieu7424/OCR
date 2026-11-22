use ferrite_pager::{PAGE_SIZE, Page, PageNumber};
use std::collections::HashSet;
use thiserror::Error;

const DATABASE_HEADER_SIZE: usize = 100;
const LEAF_HEADER_SIZE: usize = 8;
const INTERIOR_HEADER_SIZE: usize = 12;
const MAX_TREE_DEPTH: u32 = 64;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BtreeError {
    #[error("varint output buffer is too small: need {required} bytes, got {available}")]
    VarintOutputTooSmall { required: usize, available: usize },
    #[error("varint input ended before a complete value was available")]
    TruncatedVarint,
    #[error("unsupported B-tree page type: {0:#04x}")]
    InvalidPageKind(u8),
    #[error("page 1 reserves its first 100 bytes for the database header")]
    InvalidPageHeader,
    #[error("corrupt B-tree page: {0}")]
    CorruptPage(&'static str),
    #[error("cell slot {slot} is outside 0..{cell_count}")]
    InvalidSlot { slot: usize, cell_count: usize },
    #[error("cell must contain at least one byte")]
    EmptyCell,
    #[error("cell does not fit in the page's contiguous free space")]
    NoSpace,
    #[error("interior page requires a nonzero right-most child page")]
    MissingRightmostChild,
    #[error("leaf page cannot have a right-most child")]
    UnexpectedRightmostChild,
    #[error("record is malformed: {0}")]
    InvalidRecord(&'static str),
    #[error("record contains unsupported serial type {0}")]
    InvalidSerialType(u64),
    #[error("record text is not valid UTF-8")]
    InvalidUtf8,
    #[error("record size exceeds the supported range")]
    RecordTooLarge,
    #[error("memory allocation failed while encoding record")]
    RecordAllocation,
    #[error("duplicate table rowid {0}")]
    DuplicateRowId(i64),
    #[error("duplicate index entry for rowid {0}")]
    DuplicateIndexEntry(i64),
    #[error("page {page_number} is not a table B-tree page")]
    WrongTreePage { page_number: u32 },
    #[error("table cell is too large to fit in a B-tree page")]
    CellTooLarge,
    #[error("page number space is exhausted")]
    PageNumberExhausted,
    #[error("B-tree page transaction failed: {0}")]
    Transaction(String),
    #[error("B-tree exceeds the maximum supported depth")]
    TreeTooDeep,
    #[error("B-tree integrity check failed: {0}")]
    InvalidTree(&'static str),
}

pub mod index;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PageKind {
    IndexInterior = 0x02,
    TableInterior = 0x05,
    IndexLeaf = 0x0a,
    TableLeaf = 0x0d,
}

impl PageKind {
    fn from_byte(byte: u8) -> Result<Self, BtreeError> {
        match byte {
            0x02 => Ok(Self::IndexInterior),
            0x05 => Ok(Self::TableInterior),
            0x0a => Ok(Self::IndexLeaf),
            0x0d => Ok(Self::TableLeaf),
            other => Err(BtreeError::InvalidPageKind(other)),
        }
    }

    fn is_interior(self) -> bool {
        matches!(self, Self::IndexInterior | Self::TableInterior)
    }

    fn header_size(self) -> usize {
        if self.is_interior() {
            INTERIOR_HEADER_SIZE
        } else {
            LEAF_HEADER_SIZE
        }
    }
}

/// Slotted B-tree page layout. Page 1's B-tree header starts after the DB header.
///
/// ```text
/// Leaf (8-byte header) / Interior (12-byte header)
/// +--------------------+----------------------+------------------+
/// | Page Header        | Cell Pointer Array   | Free Space       |
/// | 8 or 12 bytes      | 2 bytes per cell     |                  |
/// +--------------------+----------------------+------------------+
/// | Cell content area grows backward from the end of the 4096-byte page |
/// +---------------------------------------------------------------------+
/// Interior header adds a 4-byte big-endian right-most child page number.
/// ```
pub struct SlottedPage<'a> {
    page: &'a mut Page,
    base_offset: usize,
    kind: PageKind,
}

impl<'a> SlottedPage<'a> {
    pub fn initialize(
        page: &'a mut Page,
        page_number: PageNumber,
        kind: PageKind,
        rightmost_child: Option<PageNumber>,
    ) -> Result<Self, BtreeError> {
        match (kind.is_interior(), rightmost_child) {
            (true, None) => return Err(BtreeError::MissingRightmostChild),
            (false, Some(_)) => return Err(BtreeError::UnexpectedRightmostChild),
            _ => {}
        }

        let base_offset = page_base_offset(page_number)?;
        page.as_mut_bytes()[base_offset..].fill(0);
        let bytes = page.as_mut_bytes();
        bytes[base_offset] = kind as u8;
        write_u16(bytes, base_offset + 5, PAGE_SIZE as u16);
        if let Some(child) = rightmost_child {
            write_u32(bytes, base_offset + 8, child.get());
        }
        Ok(Self {
            page,
            base_offset,
            kind,
        })
    }

    pub fn open(page: &'a mut Page, page_number: PageNumber) -> Result<Self, BtreeError> {
        let base_offset = page_base_offset(page_number)?;
        let bytes = page.as_bytes();
        let kind = PageKind::from_byte(bytes[base_offset])?;
        validate_page(bytes, base_offset, kind)?;
        Ok(Self {
            page,
            base_offset,
            kind,
        })
    }

    pub fn kind(&self) -> PageKind {
        self.kind
    }

    pub fn cell_count(&self) -> usize {
        usize::from(read_u16(self.page.as_bytes(), self.base_offset + 3))
    }

    pub fn rightmost_child(&self) -> Option<PageNumber> {
        if !self.kind.is_interior() {
            return None;
        }
        PageNumber::new(read_u32(self.page.as_bytes(), self.base_offset + 8)).ok()
    }

    pub fn cell_offset(&self, slot: usize) -> Result<usize, BtreeError> {
        let cell_count = self.cell_count();
        if slot >= cell_count {
            return Err(BtreeError::InvalidSlot { slot, cell_count });
        }
        let pointer_offset = self.base_offset + self.kind.header_size() + slot * 2;
        Ok(usize::from(read_u16(self.page.as_bytes(), pointer_offset)))
    }

    pub fn cell_bytes(&self, slot: usize, length: usize) -> Result<&[u8], BtreeError> {
        let offset = self.cell_offset(slot)?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= PAGE_SIZE)
            .ok_or(BtreeError::CorruptPage("cell extends beyond the page"))?;
        Ok(&self.page.as_bytes()[offset..end])
    }

    /// Inserts raw cell bytes at a caller-selected key-sorted slot.
    ///
    /// The caller orders slots by decoded key; bytewise cell ordering is not
    /// equivalent to signed rowid ordering. This initial implementation does
    /// not compact fragmented/freeblock space.
    pub fn insert_cell(&mut self, slot: usize, cell: &[u8]) -> Result<usize, BtreeError> {
        if cell.is_empty() {
            return Err(BtreeError::EmptyCell);
        }
        let bytes = self.page.as_mut_bytes();
        let header_size = self.kind.header_size();
        let cell_count = usize::from(read_u16(bytes, self.base_offset + 3));
        if slot > cell_count {
            return Err(BtreeError::InvalidSlot { slot, cell_count });
        }
        let new_count = cell_count.checked_add(1).ok_or(BtreeError::NoSpace)?;
        let new_count = u16::try_from(new_count).map_err(|_| BtreeError::NoSpace)?;
        let pointer_end = self
            .base_offset
            .checked_add(header_size)
            .and_then(|offset| offset.checked_add(usize::from(new_count) * 2))
            .ok_or(BtreeError::NoSpace)?;
        let content_start_offset = self.base_offset + 5;
        let content_start = usize::from(read_u16(bytes, content_start_offset));
        if pointer_end > content_start || cell.len() > content_start - pointer_end {
            return Err(BtreeError::NoSpace);
        }
        let new_content_start = content_start - cell.len();
        if new_content_start < pointer_end || new_content_start < self.base_offset {
            return Err(BtreeError::NoSpace);
        }

        let old_pointer_start = self.base_offset + header_size;
        bytes.copy_within(
            old_pointer_start + slot * 2..old_pointer_start + cell_count * 2,
            old_pointer_start + (slot + 1) * 2,
        );
        bytes[new_content_start..content_start].copy_from_slice(cell);
        write_u16(
            bytes,
            old_pointer_start + slot * 2,
            new_content_start as u16,
        );
        write_u16(bytes, self.base_offset + 3, new_count);
        write_u16(bytes, content_start_offset, new_content_start as u16);
        Ok(new_content_start)
    }
}

/// Page access provided by a transaction. Implementations must keep page writes
/// isolated until the caller commits, and discard them on rollback.
pub trait PageTransaction {
    fn page_count(&self) -> u32;
    fn allocate_page(&mut self) -> Result<PageNumber, BtreeError>;
    fn read_page(&mut self, page_number: PageNumber) -> Result<Page, BtreeError>;
    fn write_page(&mut self, page_number: PageNumber, page: &Page) -> Result<(), BtreeError>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeStats {
    pub pages: u32,
    pub rows: u64,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TableBtree {
    root: PageNumber,
}

#[derive(Clone, Debug)]
struct LeafCell {
    rowid: i64,
    encoded: Vec<u8>,
    record_offset: usize,
}

#[derive(Clone, Debug)]
struct InternalNode {
    children: Vec<PageNumber>,
    separators: Vec<i64>,
}

#[derive(Clone, Copy, Debug)]
struct Split {
    separator: i64,
    right_page: PageNumber,
}

impl TableBtree {
    pub fn initialize(transaction: &mut impl PageTransaction) -> Result<Self, BtreeError> {
        let root = root_page_number()?;
        Self::initialize_at(transaction, root)
    }

    pub fn initialize_at(
        transaction: &mut impl PageTransaction,
        root: PageNumber,
    ) -> Result<Self, BtreeError> {
        let mut page = transaction.read_page(root)?;
        SlottedPage::initialize(&mut page, root, PageKind::TableLeaf, None)?;
        transaction.write_page(root, &page)?;
        Ok(Self { root })
    }

    pub fn open(transaction: &mut impl PageTransaction) -> Result<Self, BtreeError> {
        let root = root_page_number()?;
        Self::open_at(transaction, root)
    }

    pub fn open_at(
        transaction: &mut impl PageTransaction,
        root: PageNumber,
    ) -> Result<Self, BtreeError> {
        let mut page = transaction.read_page(root)?;
        table_page_kind(&mut page, root)?;
        Ok(Self { root })
    }

    pub fn insert(
        &self,
        transaction: &mut impl PageTransaction,
        rowid: i64,
        record: &[u8],
    ) -> Result<(), BtreeError> {
        RecordRef::new(record)?;
        let encoded = encode_leaf_cell(rowid, record)?;
        match insert_recursive(transaction, self.root, self.root, rowid, encoded, 0)? {
            None => Ok(()),
            Some(_) => Err(BtreeError::InvalidTree(
                "root insertion left an unpropagated page split",
            )),
        }
    }

    pub fn get(
        &self,
        transaction: &mut impl PageTransaction,
        rowid: i64,
    ) -> Result<Option<Vec<u8>>, BtreeError> {
        get_recursive(transaction, self.root, rowid, 0)
    }

    pub fn remove(
        &self,
        transaction: &mut impl PageTransaction,
        rowid: i64,
    ) -> Result<Option<Vec<u8>>, BtreeError> {
        Ok(
            remove_recursive(transaction, self.root, self.root, rowid, 0)?
                .map(|removed| removed.record),
        )
    }

    pub fn scan(
        &self,
        transaction: &mut impl PageTransaction,
    ) -> Result<Vec<(i64, Vec<u8>)>, BtreeError> {
        let mut rows = Vec::new();
        let mut visited = HashSet::new();
        scan_recursive(transaction, self.root, 0, &mut visited, &mut rows)?;
        Ok(rows)
    }

    pub fn validate(
        &self,
        transaction: &mut impl PageTransaction,
    ) -> Result<TreeStats, BtreeError> {
        let page_count = transaction.page_count();
        if page_count == 0 {
            return Err(BtreeError::InvalidTree("database has no root page"));
        }
        let mut visited = HashSet::new();
        let validation = validate_subtree(
            transaction,
            self.root,
            page_count,
            None,
            None,
            0,
            &mut visited,
        )?;
        Ok(TreeStats {
            pages: validation.pages,
            rows: validation.rows,
            height: validation.height,
        })
    }
}

fn root_page_number() -> Result<PageNumber, BtreeError> {
    PageNumber::new(1).map_err(|_| BtreeError::InvalidTree("root page number is invalid"))
}

fn table_page_kind(page: &mut Page, page_number: PageNumber) -> Result<PageKind, BtreeError> {
    let kind = SlottedPage::open(page, page_number)?.kind();
    if matches!(kind, PageKind::TableLeaf | PageKind::TableInterior) {
        Ok(kind)
    } else {
        Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        })
    }
}

fn encode_leaf_cell(rowid: i64, record: &[u8]) -> Result<LeafCell, BtreeError> {
    let payload_length = u64::try_from(record.len()).map_err(|_| BtreeError::RecordTooLarge)?;
    let mut encoded = Vec::new();
    let capacity = varint_len(payload_length)
        .checked_add(varint_len(rowid as u64))
        .and_then(|length| length.checked_add(record.len()))
        .ok_or(BtreeError::RecordTooLarge)?;
    encoded
        .try_reserve_exact(capacity)
        .map_err(|_| BtreeError::RecordAllocation)?;
    let mut varint = [0; 9];
    let length = encode_varint(payload_length, &mut varint)?;
    encoded.extend_from_slice(&varint[..length]);
    let length = encode_varint(rowid as u64, &mut varint)?;
    encoded.extend_from_slice(&varint[..length]);
    let record_offset = encoded.len();
    encoded.extend_from_slice(record);
    Ok(LeafCell {
        rowid,
        encoded,
        record_offset,
    })
}

fn read_leaf_cells(page: &mut Page, page_number: PageNumber) -> Result<Vec<LeafCell>, BtreeError> {
    let slotted = SlottedPage::open(page, page_number)?;
    if slotted.kind() != PageKind::TableLeaf {
        return Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        });
    }
    let ranges = cell_ranges(&slotted)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for (slot, (offset, end)) in ranges.iter().copied().enumerate() {
        let available = end - offset;
        let bytes = slotted.cell_bytes(slot, available)?;
        let (payload_length, payload_size_len) = decode_varint(bytes)?;
        let key_start = payload_size_len;
        let (rowid, rowid_size_len) = decode_varint(&bytes[key_start..])?;
        let record_offset = key_start
            .checked_add(rowid_size_len)
            .ok_or(BtreeError::RecordTooLarge)?;
        let payload_length =
            usize::try_from(payload_length).map_err(|_| BtreeError::RecordTooLarge)?;
        let cell_length = record_offset
            .checked_add(payload_length)
            .filter(|length| *length <= available)
            .ok_or(BtreeError::CorruptPage(
                "table leaf cell extends beyond the page",
            ))?;
        if cell_length != available {
            return Err(BtreeError::CorruptPage(
                "table leaf cell length does not match its page range",
            ));
        }
        let encoded = copy_slice(slotted.cell_bytes(slot, cell_length)?)?;
        RecordRef::new(&encoded[record_offset..])?;
        cells.push(LeafCell {
            rowid: rowid as i64,
            encoded,
            record_offset,
        });
    }
    if cells
        .windows(2)
        .any(|window| window[0].rowid >= window[1].rowid)
    {
        return Err(BtreeError::CorruptPage(
            "table leaf rowids are not strictly increasing",
        ));
    }
    Ok(cells)
}

fn read_internal_node(
    page: &mut Page,
    page_number: PageNumber,
    page_count: u32,
) -> Result<InternalNode, BtreeError> {
    let slotted = SlottedPage::open(page, page_number)?;
    if slotted.kind() != PageKind::TableInterior {
        return Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        });
    }
    let ranges = cell_ranges(&slotted)?;
    let mut children = Vec::new();
    let mut separators = Vec::new();
    children
        .try_reserve_exact(slotted.cell_count().saturating_add(1))
        .map_err(|_| BtreeError::RecordAllocation)?;
    separators
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for (slot, (offset, end)) in ranges.iter().copied().enumerate() {
        let available = end - offset;
        let cell = slotted.cell_bytes(slot, available)?;
        if cell.len() < 5 {
            return Err(BtreeError::CorruptPage("table interior cell is too short"));
        }
        let child = PageNumber::new(read_u32_from_slice(cell))
            .map_err(|_| BtreeError::CorruptPage("table interior cell has a zero child"))?;
        let (separator, separator_len) = decode_varint(&cell[4..])?;
        if separator_len + 4 != cell.len() {
            return Err(BtreeError::CorruptPage(
                "table interior cell has trailing bytes",
            ));
        }
        if child.get() > page_count {
            return Err(BtreeError::InvalidTree(
                "table interior cell references a page outside the database",
            ));
        }
        separators.push(separator as i64);
        children.push(child);
    }
    let rightmost = slotted.rightmost_child().ok_or(BtreeError::CorruptPage(
        "table interior page has no right-most child",
    ))?;
    if rightmost.get() > page_count {
        return Err(BtreeError::InvalidTree(
            "right-most child references a page outside the database",
        ));
    }
    children.push(rightmost);
    if children.len() != separators.len() + 1
        || children.is_empty()
        || separators.windows(2).any(|window| window[0] >= window[1])
    {
        return Err(BtreeError::CorruptPage(
            "table interior node has invalid children or separators",
        ));
    }
    Ok(InternalNode {
        children,
        separators,
    })
}

fn get_recursive(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    rowid: i64,
    depth: u32,
) -> Result<Option<Vec<u8>>, BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => {
            let cells = read_leaf_cells(&mut page, page_number)?;
            match cells.binary_search_by_key(&rowid, |cell| cell.rowid) {
                Ok(index) => Ok(Some(copy_slice(
                    &cells[index].encoded[cells[index].record_offset..],
                )?)),
                Err(_) => Ok(None),
            }
        }
        PageKind::TableInterior => {
            let node = read_internal_node(&mut page, page_number, transaction.page_count())?;
            let child_index = node
                .separators
                .partition_point(|separator| rowid > *separator);
            get_recursive(transaction, node.children[child_index], rowid, depth + 1)
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

struct RemovedRow {
    record: Vec<u8>,
    maximum: Option<i64>,
}

fn remove_recursive(
    transaction: &mut impl PageTransaction,
    root: PageNumber,
    page_number: PageNumber,
    rowid: i64,
    depth: u32,
) -> Result<Option<RemovedRow>, BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => {
            let mut cells = read_leaf_cells(&mut page, page_number)?;
            let Ok(index) = cells.binary_search_by_key(&rowid, |cell| cell.rowid) else {
                return Ok(None);
            };
            let removed = cells.remove(index);
            let record = copy_slice(&removed.encoded[removed.record_offset..])?;
            let maximum = cells.last().map(|cell| cell.rowid);
            let base_page = if page_number == root {
                page
            } else {
                Page::zeroed()
            };
            let packed = pack_leaf_page(page_number, &cells, base_page)?;
            transaction.write_page(page_number, &packed)?;
            Ok(Some(RemovedRow { record, maximum }))
        }
        PageKind::TableInterior => {
            let mut node = read_internal_node(&mut page, page_number, transaction.page_count())?;
            let child_index = node
                .separators
                .partition_point(|separator| rowid > *separator);
            let child_page = node.children[child_index];
            let Some(removed) = remove_recursive(transaction, root, child_page, rowid, depth + 1)?
            else {
                return Ok(None);
            };

            if let Some(maximum) = removed.maximum {
                if child_index < node.separators.len() {
                    node.separators[child_index] = maximum;
                }
            } else {
                node.children.remove(child_index);
                if !node.separators.is_empty() {
                    let separator_index = child_index.min(node.separators.len() - 1);
                    node.separators.remove(separator_index);
                }
            }

            if node.children.is_empty() {
                if page_number == root {
                    let mut empty_root = transaction.read_page(root)?;
                    SlottedPage::initialize(&mut empty_root, root, PageKind::TableLeaf, None)?;
                    transaction.write_page(root, &empty_root)?;
                }
                return Ok(Some(RemovedRow {
                    record: removed.record,
                    maximum: None,
                }));
            }

            let packed = pack_internal_page(page_number, &node, page)?;
            transaction.write_page(page_number, &packed)?;
            let maximum = maximum_rowid(transaction, page_number, depth)?;
            Ok(Some(RemovedRow {
                record: removed.record,
                maximum,
            }))
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn maximum_rowid(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    depth: u32,
) -> Result<Option<i64>, BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => Ok(read_leaf_cells(&mut page, page_number)?
            .last()
            .map(|cell| cell.rowid)),
        PageKind::TableInterior => {
            let node = read_internal_node(&mut page, page_number, transaction.page_count())?;
            let rightmost = *node.children.last().ok_or(BtreeError::CorruptPage(
                "table interior node has no right-most child",
            ))?;
            maximum_rowid(transaction, rightmost, depth + 1)
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn scan_recursive(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    depth: u32,
    visited: &mut HashSet<u32>,
    rows: &mut Vec<(i64, Vec<u8>)>,
) -> Result<(), BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    visited
        .try_reserve(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    if !visited.insert(page_number.get()) {
        return Err(BtreeError::InvalidTree(
            "table tree references a page more than once",
        ));
    }
    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => {
            let cells = read_leaf_cells(&mut page, page_number)?;
            rows.try_reserve(cells.len())
                .map_err(|_| BtreeError::RecordAllocation)?;
            for cell in cells {
                rows.push((cell.rowid, copy_slice(&cell.encoded[cell.record_offset..])?));
            }
        }
        PageKind::TableInterior => {
            let node = read_internal_node(&mut page, page_number, transaction.page_count())?;
            for child in node.children {
                scan_recursive(transaction, child, depth + 1, visited, rows)?;
            }
        }
        _ => {
            return Err(BtreeError::WrongTreePage {
                page_number: page_number.get(),
            });
        }
    }
    Ok(())
}

fn insert_recursive(
    transaction: &mut impl PageTransaction,
    root: PageNumber,
    page_number: PageNumber,
    rowid: i64,
    encoded_cell: LeafCell,
    depth: u32,
) -> Result<Option<Split>, BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => {
            let mut cells = read_leaf_cells(&mut page, page_number)?;
            let insert_at = match cells.binary_search_by_key(&rowid, |cell| cell.rowid) {
                Ok(_) => return Err(BtreeError::DuplicateRowId(rowid)),
                Err(index) => index,
            };
            cells
                .try_reserve(1)
                .map_err(|_| BtreeError::RecordAllocation)?;
            cells.insert(insert_at, encoded_cell);
            let base_page = if page_number == root {
                page
            } else {
                Page::zeroed()
            };
            match pack_leaf_page(page_number, &cells, base_page) {
                Ok(packed) => {
                    transaction.write_page(page_number, &packed)?;
                    Ok(None)
                }
                Err(BtreeError::NoSpace) => {
                    let split_index = leaf_split_index(&cells)?;
                    let separator = cells[split_index - 1].rowid;
                    if page_number == root {
                        let left_page = allocate_tree_page(transaction)?;
                        let right_page = allocate_tree_page(transaction)?;
                        let left =
                            pack_leaf_page(left_page, &cells[..split_index], Page::zeroed())?;
                        let right =
                            pack_leaf_page(right_page, &cells[split_index..], Page::zeroed())?;
                        transaction.write_page(left_page, &left)?;
                        transaction.write_page(right_page, &right)?;
                        let root_page = transaction.read_page(root)?;
                        let root_node = root_internal_node(left_page, right_page, separator)?;
                        let root_page = pack_internal_page(root, &root_node, root_page)?;
                        transaction.write_page(root, &root_page)?;
                        Ok(None)
                    } else {
                        let left =
                            pack_leaf_page(page_number, &cells[..split_index], Page::zeroed())?;
                        let right_page = allocate_tree_page(transaction)?;
                        let right =
                            pack_leaf_page(right_page, &cells[split_index..], Page::zeroed())?;
                        transaction.write_page(page_number, &left)?;
                        transaction.write_page(right_page, &right)?;
                        Ok(Some(Split {
                            separator,
                            right_page,
                        }))
                    }
                }
                Err(error) => Err(error),
            }
        }
        PageKind::TableInterior => {
            let mut node = read_internal_node(&mut page, page_number, transaction.page_count())?;
            let child_index = node
                .separators
                .partition_point(|separator| rowid > *separator);
            let child_page = node.children[child_index];
            let Some(split) = insert_recursive(
                transaction,
                root,
                child_page,
                rowid,
                encoded_cell,
                depth + 1,
            )?
            else {
                return Ok(None);
            };
            node.separators
                .try_reserve(1)
                .map_err(|_| BtreeError::RecordAllocation)?;
            node.children
                .try_reserve(1)
                .map_err(|_| BtreeError::RecordAllocation)?;
            node.separators.insert(child_index, split.separator);
            node.children.insert(child_index + 1, split.right_page);
            match pack_internal_page(page_number, &node, page) {
                Ok(packed) => {
                    transaction.write_page(page_number, &packed)?;
                    Ok(None)
                }
                Err(BtreeError::NoSpace) => {
                    let (left_node, separator, right_node) = split_internal_node(&node)?;
                    if page_number == root {
                        let left_page = allocate_tree_page(transaction)?;
                        let right_page = allocate_tree_page(transaction)?;
                        let left = pack_internal_page(left_page, &left_node, Page::zeroed())?;
                        let right = pack_internal_page(right_page, &right_node, Page::zeroed())?;
                        transaction.write_page(left_page, &left)?;
                        transaction.write_page(right_page, &right)?;
                        let root_page = transaction.read_page(root)?;
                        let root_node = root_internal_node(left_page, right_page, separator)?;
                        let root_page = pack_internal_page(root, &root_node, root_page)?;
                        transaction.write_page(root, &root_page)?;
                        Ok(None)
                    } else {
                        let left = pack_internal_page(page_number, &left_node, Page::zeroed())?;
                        let right_page = allocate_tree_page(transaction)?;
                        let right = pack_internal_page(right_page, &right_node, Page::zeroed())?;
                        transaction.write_page(page_number, &left)?;
                        transaction.write_page(right_page, &right)?;
                        Ok(Some(Split {
                            separator,
                            right_page,
                        }))
                    }
                }
                Err(error) => Err(error),
            }
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn allocate_tree_page(transaction: &mut impl PageTransaction) -> Result<PageNumber, BtreeError> {
    let expected = transaction
        .page_count()
        .checked_add(1)
        .ok_or(BtreeError::PageNumberExhausted)?;
    let page_number = transaction.allocate_page()?;
    if page_number.get() != expected {
        return Err(BtreeError::InvalidTree(
            "page transaction did not append the allocated page",
        ));
    }
    Ok(page_number)
}

fn pack_leaf_page(
    page_number: PageNumber,
    cells: &[LeafCell],
    mut page: Page,
) -> Result<Page, BtreeError> {
    {
        let mut slotted =
            SlottedPage::initialize(&mut page, page_number, PageKind::TableLeaf, None)?;
        for (slot, cell) in cells.iter().enumerate() {
            slotted.insert_cell(slot, &cell.encoded)?;
        }
    }
    Ok(page)
}

fn pack_internal_page(
    page_number: PageNumber,
    node: &InternalNode,
    mut page: Page,
) -> Result<Page, BtreeError> {
    if node.children.len() != node.separators.len() + 1 || node.children.is_empty() {
        return Err(BtreeError::CorruptPage(
            "table interior child and separator counts disagree",
        ));
    }
    let rightmost = *node.children.last().ok_or(BtreeError::CorruptPage(
        "table interior node has no right-most child",
    ))?;
    {
        let mut slotted = SlottedPage::initialize(
            &mut page,
            page_number,
            PageKind::TableInterior,
            Some(rightmost),
        )?;
        let mut encoded = [0; 13];
        for (slot, (child, separator)) in node.children.iter().zip(&node.separators).enumerate() {
            encoded[..4].copy_from_slice(&child.get().to_be_bytes());
            let length = encode_varint(*separator as u64, &mut encoded[4..])?;
            slotted.insert_cell(slot, &encoded[..4 + length])?;
        }
    }
    Ok(page)
}

fn root_internal_node(
    left_page: PageNumber,
    right_page: PageNumber,
    separator: i64,
) -> Result<InternalNode, BtreeError> {
    let mut children = Vec::new();
    children
        .try_reserve_exact(2)
        .map_err(|_| BtreeError::RecordAllocation)?;
    children.push(left_page);
    children.push(right_page);
    let mut separators = Vec::new();
    separators
        .try_reserve_exact(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    separators.push(separator);
    Ok(InternalNode {
        children,
        separators,
    })
}

fn copy_slice<T: Copy>(slice: &[T]) -> Result<Vec<T>, BtreeError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(slice.len())
        .map_err(|_| BtreeError::RecordAllocation)?;
    copy.extend_from_slice(slice);
    Ok(copy)
}

fn cell_ranges(slotted: &SlottedPage<'_>) -> Result<Vec<(usize, usize)>, BtreeError> {
    let mut locations = Vec::new();
    locations
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for slot in 0..slotted.cell_count() {
        locations.push((slotted.cell_offset(slot)?, slot));
    }
    locations.sort_unstable_by_key(|location| location.0);

    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    ranges.resize(slotted.cell_count(), (0, 0));
    for (index, (start, slot)) in locations.iter().copied().enumerate() {
        let end = locations
            .get(index + 1)
            .map(|location| location.0)
            .unwrap_or(PAGE_SIZE);
        if end <= start {
            return Err(BtreeError::CorruptPage("cell ranges overlap"));
        }
        ranges[slot] = (start, end);
    }
    Ok(ranges)
}

fn leaf_split_index(cells: &[LeafCell]) -> Result<usize, BtreeError> {
    let capacity = PAGE_SIZE - LEAF_HEADER_SIZE;
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(cells.len() + 1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    prefix.push(0_usize);
    for cell in cells {
        let cost = cell
            .encoded
            .len()
            .checked_add(2)
            .ok_or(BtreeError::RecordTooLarge)?;
        if cost > capacity {
            return Err(BtreeError::CellTooLarge);
        }
        let next = prefix
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(cost)
            .ok_or(BtreeError::RecordTooLarge)?;
        prefix.push(next);
    }
    let total = *prefix.last().unwrap_or(&0);
    let mut best = None;
    let mut best_difference = usize::MAX;
    for (split, left) in prefix.iter().copied().enumerate().take(cells.len()).skip(1) {
        let right = total - left;
        if left <= capacity && right <= capacity {
            let difference = left.abs_diff(right);
            if difference < best_difference {
                best = Some(split);
                best_difference = difference;
            }
        }
    }
    best.ok_or(BtreeError::CellTooLarge)
}

fn split_internal_node(
    node: &InternalNode,
) -> Result<(InternalNode, i64, InternalNode), BtreeError> {
    let capacity = PAGE_SIZE - INTERIOR_HEADER_SIZE;
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(node.separators.len() + 1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    prefix.push(0_usize);
    for separator in &node.separators {
        let cost = 4_usize
            .checked_add(varint_len(*separator as u64))
            .and_then(|length| length.checked_add(2))
            .ok_or(BtreeError::RecordTooLarge)?;
        let next = prefix
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(cost)
            .ok_or(BtreeError::RecordTooLarge)?;
        prefix.push(next);
    }
    let total = *prefix.last().unwrap_or(&0);
    let child_count = node.children.len();
    let mut best = None;
    let mut best_difference = usize::MAX;
    for boundary in 2..child_count.saturating_sub(1) {
        let left = prefix[boundary - 1];
        let right = total - prefix[boundary];
        if left <= capacity && right <= capacity {
            let difference = left.abs_diff(right);
            if difference < best_difference {
                best = Some(boundary);
                best_difference = difference;
            }
        }
    }
    let boundary = best.ok_or(BtreeError::InvalidTree(
        "overfull interior node cannot be split into valid children",
    ))?;
    let separator = node.separators[boundary - 1];
    Ok((
        InternalNode {
            children: copy_slice(&node.children[..boundary])?,
            separators: copy_slice(&node.separators[..boundary - 1])?,
        },
        separator,
        InternalNode {
            children: copy_slice(&node.children[boundary..])?,
            separators: copy_slice(&node.separators[boundary..])?,
        },
    ))
}

struct Validation {
    minimum: Option<i64>,
    maximum: Option<i64>,
    pages: u32,
    rows: u64,
    height: u32,
}

fn validate_subtree(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    page_count: u32,
    lower_bound: Option<i64>,
    upper_bound: Option<i64>,
    depth: u32,
    visited: &mut HashSet<u32>,
) -> Result<Validation, BtreeError> {
    if depth >= MAX_TREE_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    if page_number.get() > page_count {
        return Err(BtreeError::InvalidTree(
            "tree contains a page outside the database",
        ));
    }
    visited
        .try_reserve(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    if !visited.insert(page_number.get()) {
        return Err(BtreeError::InvalidTree(
            "tree references a page more than once",
        ));
    }

    let mut page = transaction.read_page(page_number)?;
    match table_page_kind(&mut page, page_number)? {
        PageKind::TableLeaf => {
            let cells = read_leaf_cells(&mut page, page_number)?;
            for cell in &cells {
                if lower_bound.is_some_and(|bound| cell.rowid <= bound)
                    || upper_bound.is_some_and(|bound| cell.rowid > bound)
                {
                    return Err(BtreeError::InvalidTree(
                        "leaf rowid is outside its parent separator range",
                    ));
                }
            }
            let rows = u64::try_from(cells.len()).map_err(|_| BtreeError::RecordTooLarge)?;
            Ok(Validation {
                minimum: cells.first().map(|cell| cell.rowid),
                maximum: cells.last().map(|cell| cell.rowid),
                pages: 1,
                rows,
                height: 1,
            })
        }
        PageKind::TableInterior => {
            let node = read_internal_node(&mut page, page_number, page_count)?;
            let mut rows = 0_u64;
            let mut pages = 1_u32;
            let mut child_height = None;
            let mut minimum = None;
            let mut maximum = None;
            for (index, child_page) in node.children.iter().copied().enumerate() {
                let child_lower = if index == 0 {
                    lower_bound
                } else {
                    Some(node.separators[index - 1])
                };
                let child_upper = node.separators.get(index).copied().or(upper_bound);
                let child = validate_subtree(
                    transaction,
                    child_page,
                    page_count,
                    child_lower,
                    child_upper,
                    depth + 1,
                    visited,
                )?;
                if child.maximum.is_none() {
                    return Err(BtreeError::InvalidTree(
                        "interior page references an empty child",
                    ));
                }
                if child_height.is_some_and(|height| height != child.height) {
                    return Err(BtreeError::InvalidTree(
                        "table B-tree leaves have different depths",
                    ));
                }
                child_height = Some(child.height);
                if index == 0 {
                    minimum = child.minimum;
                }
                if index < node.separators.len() && child.maximum != Some(node.separators[index]) {
                    return Err(BtreeError::InvalidTree(
                        "interior separator does not match its left child's maximum rowid",
                    ));
                }
                maximum = child.maximum;
                rows = rows
                    .checked_add(child.rows)
                    .ok_or(BtreeError::RecordTooLarge)?;
                pages = pages
                    .checked_add(child.pages)
                    .ok_or(BtreeError::RecordTooLarge)?;
            }
            let height = child_height
                .and_then(|height| height.checked_add(1))
                .ok_or(BtreeError::InvalidTree(
                    "interior page has no valid children",
                ))?;
            Ok(Validation {
                minimum,
                maximum,
                pages,
                rows,
                height,
            })
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn page_base_offset(page_number: PageNumber) -> Result<usize, BtreeError> {
    if page_number.get() == 1 {
        Ok(DATABASE_HEADER_SIZE)
    } else {
        Ok(0)
    }
}

fn validate_page(
    bytes: &[u8; PAGE_SIZE],
    base_offset: usize,
    kind: PageKind,
) -> Result<(), BtreeError> {
    let header_end = base_offset
        .checked_add(kind.header_size())
        .ok_or(BtreeError::InvalidPageHeader)?;
    if header_end > PAGE_SIZE {
        return Err(BtreeError::InvalidPageHeader);
    }

    let cell_count = usize::from(read_u16(bytes, base_offset + 3));
    let pointer_end = header_end
        .checked_add(
            cell_count
                .checked_mul(2)
                .ok_or(BtreeError::InvalidPageHeader)?,
        )
        .ok_or(BtreeError::InvalidPageHeader)?;
    let content_start = usize::from(read_u16(bytes, base_offset + 5));
    if content_start < pointer_end || content_start > PAGE_SIZE {
        return Err(BtreeError::CorruptPage(
            "cell content overlaps the header/pointer array or exceeds the page",
        ));
    }
    if bytes[base_offset + 7] > 60 {
        return Err(BtreeError::CorruptPage(
            "fragmented free-byte count exceeds the page format limit",
        ));
    }

    let mut freeblock = usize::from(read_u16(bytes, base_offset + 1));
    let mut previous_freeblock = 0;
    while freeblock != 0 {
        if freeblock <= previous_freeblock || freeblock < content_start {
            return Err(BtreeError::CorruptPage(
                "freeblock chain is unordered or outside the content area",
            ));
        }
        let header_end = freeblock
            .checked_add(4)
            .ok_or(BtreeError::CorruptPage("freeblock header overflows"))?;
        if header_end > PAGE_SIZE {
            return Err(BtreeError::CorruptPage(
                "freeblock header extends beyond the page",
            ));
        }
        let size = usize::from(read_u16(bytes, freeblock + 2));
        if size < 4
            || freeblock
                .checked_add(size)
                .filter(|end| *end <= PAGE_SIZE)
                .is_none()
        {
            return Err(BtreeError::CorruptPage(
                "freeblock size is invalid or extends beyond the page",
            ));
        }
        previous_freeblock = freeblock;
        freeblock = usize::from(read_u16(bytes, previous_freeblock));
    }

    if kind.is_interior() && read_u32(bytes, base_offset + 8) == 0 {
        return Err(BtreeError::CorruptPage(
            "interior page has no right-most child",
        ));
    }

    let mut pointers = Vec::with_capacity(cell_count);
    for slot in 0..cell_count {
        let pointer = base_offset + kind.header_size() + slot * 2;
        let cell_offset = usize::from(read_u16(bytes, pointer));
        if cell_offset < content_start || cell_offset >= PAGE_SIZE {
            return Err(BtreeError::CorruptPage(
                "cell pointer is outside the cell content area",
            ));
        }
        pointers.push(cell_offset);
    }
    pointers.sort_unstable();
    if pointers.windows(2).any(|window| window[0] == window[1]) {
        return Err(BtreeError::CorruptPage("duplicate cell pointer"));
    }
    Ok(())
}

/// Encodes an unsigned integer using the 1–9 byte SQLite-style varint format.
pub fn encode_varint(value: u64, output: &mut [u8]) -> Result<usize, BtreeError> {
    let length = varint_len(value);
    if output.len() < length {
        return Err(BtreeError::VarintOutputTooSmall {
            required: length,
            available: output.len(),
        });
    }

    if length == 9 {
        for (output_byte, shift) in output.iter_mut().take(8).zip((8..=57).rev().step_by(7)) {
            *output_byte = (((value >> shift) & 0x7f) as u8) | 0x80;
        }
        output[8] = value as u8;
    } else {
        for (index, output_byte) in output.iter_mut().take(length).enumerate() {
            let shift = 7 * (length - index - 1);
            let mut byte = ((value >> shift) & 0x7f) as u8;
            if index + 1 < length {
                byte |= 0x80;
            }
            *output_byte = byte;
        }
    }
    Ok(length)
}

/// Decodes one unsigned varint and returns its value and encoded length.
pub fn decode_varint(input: &[u8]) -> Result<(u64, usize), BtreeError> {
    let mut value = 0_u64;
    for (index, byte) in input.iter().copied().take(8).enumerate() {
        value = (value << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    if input.len() < 9 {
        return Err(BtreeError::TruncatedVarint);
    }
    value = (value << 8) | u64::from(input[8]);
    Ok((value, 9))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a str),
    Blob(&'a [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a str),
    Blob(&'a [u8]),
}

/// Record format: a varint header size, serial-type varints, then column data.
///
/// ```text
/// +---------------------+----------------------+----------------------+
/// | Header size varint  | Serial type varints  | Column payload bytes |
/// +---------------------+----------------------+----------------------+
/// Header size includes both header sections; payloads are stored in column order.
/// ```
pub fn encode_record(values: &[Value<'_>]) -> Result<Vec<u8>, BtreeError> {
    let mut serial_types = Vec::new();
    serial_types
        .try_reserve_exact(values.len())
        .map_err(|_| BtreeError::RecordAllocation)?;
    let mut serial_types_size = 0_usize;
    let mut payload_size = 0_usize;
    for value in values {
        let serial_type = serial_type_for(value)?;
        let payload_len = serial_payload_size(value)?;
        serial_types_size = serial_types_size
            .checked_add(varint_len(serial_type))
            .ok_or(BtreeError::RecordTooLarge)?;
        payload_size = payload_size
            .checked_add(payload_len)
            .ok_or(BtreeError::RecordTooLarge)?;
        serial_types.push(serial_type);
    }

    let mut header_size = serial_types_size
        .checked_add(1)
        .ok_or(BtreeError::RecordTooLarge)?;
    loop {
        let header_size_varint_len =
            varint_len(u64::try_from(header_size).map_err(|_| BtreeError::RecordTooLarge)?);
        let next_header_size = serial_types_size
            .checked_add(header_size_varint_len)
            .ok_or(BtreeError::RecordTooLarge)?;
        if next_header_size == header_size {
            break;
        }
        header_size = next_header_size;
    }
    let total_size = header_size
        .checked_add(payload_size)
        .ok_or(BtreeError::RecordTooLarge)?;
    let header_size = u64::try_from(header_size).map_err(|_| BtreeError::RecordTooLarge)?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(total_size)
        .map_err(|_| BtreeError::RecordAllocation)?;

    let mut varint = [0; 9];
    let varint_size = encode_varint(header_size, &mut varint)?;
    encoded.extend_from_slice(&varint[..varint_size]);
    for serial_type in serial_types {
        let varint_size = encode_varint(serial_type, &mut varint)?;
        encoded.extend_from_slice(&varint[..varint_size]);
    }
    for value in values {
        match value {
            Value::Null => {}
            Value::Integer(value) => {
                let serial_type = serial_type_for(&Value::Integer(*value))?;
                let payload_len = integer_payload_size(serial_type);
                let bytes = value.to_be_bytes();
                encoded.extend_from_slice(&bytes[8 - payload_len..]);
            }
            Value::Real(value) => encoded.extend_from_slice(&value.to_bits().to_be_bytes()),
            Value::Text(value) => encoded.extend_from_slice(value.as_bytes()),
            Value::Blob(value) => encoded.extend_from_slice(value),
        }
    }
    Ok(encoded)
}

fn serial_type_for(value: &Value<'_>) -> Result<u64, BtreeError> {
    match value {
        Value::Null => Ok(0),
        Value::Integer(0) => Ok(8),
        Value::Integer(1) => Ok(9),
        Value::Integer(value) if i8::try_from(*value).is_ok() => Ok(1),
        Value::Integer(value) if i16::try_from(*value).is_ok() => Ok(2),
        Value::Integer(value) if (-8_388_608..=8_388_607).contains(value) => Ok(3),
        Value::Integer(value) if i32::try_from(*value).is_ok() => Ok(4),
        Value::Integer(value) if (-(1_i64 << 47)..=(1_i64 << 47) - 1).contains(value) => Ok(5),
        Value::Integer(_) => Ok(6),
        Value::Real(_) => Ok(7),
        Value::Blob(value) => serial_type_for_variable(12, value.len()),
        Value::Text(value) => serial_type_for_variable(13, value.len()),
    }
}

fn serial_type_for_variable(base: u64, length: usize) -> Result<u64, BtreeError> {
    u64::try_from(length)
        .ok()
        .and_then(|length| length.checked_mul(2))
        .and_then(|length| length.checked_add(base))
        .ok_or(BtreeError::RecordTooLarge)
}

fn serial_payload_size(value: &Value<'_>) -> Result<usize, BtreeError> {
    match value {
        Value::Null | Value::Integer(0) | Value::Integer(1) => Ok(0),
        Value::Integer(value) => Ok(integer_payload_size(serial_type_for(&Value::Integer(
            *value,
        ))?)),
        Value::Real(_) => Ok(8),
        Value::Text(value) => Ok(value.len()),
        Value::Blob(value) => Ok(value.len()),
    }
}

fn integer_payload_size(serial_type: u64) -> usize {
    match serial_type {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 6,
        6 => 8,
        _ => 0,
    }
}

pub struct RecordRef<'a> {
    bytes: &'a [u8],
    serial_types_start: usize,
    header_end: usize,
    payload_start: usize,
    field_count: usize,
}

impl<'a> RecordRef<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, BtreeError> {
        let (header_size, size_varint_len) = decode_varint(bytes)?;
        let header_end = usize::try_from(header_size)
            .map_err(|_| BtreeError::InvalidRecord("header size is too large"))?;
        if header_end < size_varint_len || header_end > bytes.len() {
            return Err(BtreeError::InvalidRecord(
                "header size is outside the encoded record",
            ));
        }

        let mut serial_types_offset = size_varint_len;
        let mut payload_offset = header_end;
        let mut field_count = 0_usize;
        while serial_types_offset < header_end {
            let (serial_type, encoded_len) =
                decode_varint(&bytes[serial_types_offset..header_end])?;
            serial_types_offset = serial_types_offset
                .checked_add(encoded_len)
                .ok_or(BtreeError::RecordTooLarge)?;
            let payload_len = serial_payload_len(serial_type)?;
            let payload_end = payload_offset
                .checked_add(payload_len)
                .filter(|end| *end <= bytes.len())
                .ok_or(BtreeError::InvalidRecord(
                    "column payload extends beyond the record",
                ))?;
            decode_value(serial_type, &bytes[payload_offset..payload_end])?;
            payload_offset = payload_end;
            field_count = field_count
                .checked_add(1)
                .ok_or(BtreeError::RecordTooLarge)?;
        }
        if serial_types_offset != header_end || payload_offset != bytes.len() {
            return Err(BtreeError::InvalidRecord(
                "record header or payload has trailing bytes",
            ));
        }

        Ok(Self {
            bytes,
            serial_types_start: size_varint_len,
            header_end,
            payload_start: header_end,
            field_count,
        })
    }

    pub fn field_count(&self) -> usize {
        self.field_count
    }

    pub fn get(&self, index: usize) -> Result<Option<ValueRef<'a>>, BtreeError> {
        if index >= self.field_count {
            return Ok(None);
        }
        self.values().nth(index).transpose()
    }

    pub fn values(&self) -> RecordIter<'a> {
        RecordIter {
            bytes: self.bytes,
            serial_offset: self.serial_types_start,
            header_end: self.header_end,
            payload_offset: self.payload_start,
            fields_left: self.field_count,
        }
    }
}

pub struct RecordIter<'a> {
    bytes: &'a [u8],
    serial_offset: usize,
    header_end: usize,
    payload_offset: usize,
    fields_left: usize,
}

impl<'a> Iterator for RecordIter<'a> {
    type Item = Result<ValueRef<'a>, BtreeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fields_left == 0 {
            return None;
        }
        let decoded = (|| {
            let (serial_type, serial_len) =
                decode_varint(&self.bytes[self.serial_offset..self.header_end])?;
            let payload_len = serial_payload_len(serial_type)?;
            let payload_end = self
                .payload_offset
                .checked_add(payload_len)
                .filter(|end| *end <= self.bytes.len())
                .ok_or(BtreeError::InvalidRecord(
                    "column payload extends beyond the record",
                ))?;
            let value = decode_value(serial_type, &self.bytes[self.payload_offset..payload_end])?;
            self.serial_offset += serial_len;
            self.payload_offset = payload_end;
            self.fields_left -= 1;
            Ok(value)
        })();
        if decoded.is_err() {
            self.fields_left = 0;
        }
        Some(decoded)
    }
}

fn serial_payload_len(serial_type: u64) -> Result<usize, BtreeError> {
    match serial_type {
        0 | 8 | 9 => Ok(0),
        1 => Ok(1),
        2 => Ok(2),
        3 => Ok(3),
        4 => Ok(4),
        5 => Ok(6),
        6 | 7 => Ok(8),
        10 | 11 => Err(BtreeError::InvalidSerialType(serial_type)),
        serial_type if serial_type >= 12 => {
            usize::try_from((serial_type - if serial_type % 2 == 0 { 12 } else { 13 }) / 2)
                .map_err(|_| BtreeError::RecordTooLarge)
        }
        _ => Err(BtreeError::InvalidSerialType(serial_type)),
    }
}

fn decode_value(serial_type: u64, payload: &[u8]) -> Result<ValueRef<'_>, BtreeError> {
    match serial_type {
        0 => Ok(ValueRef::Null),
        1..=6 => {
            let expected_len = serial_payload_len(serial_type)?;
            if payload.len() != expected_len {
                return Err(BtreeError::InvalidRecord(
                    "integer payload has wrong length",
                ));
            }
            let mut value = 0_u64;
            for byte in payload {
                value = (value << 8) | u64::from(*byte);
            }
            if payload[0] & 0x80 != 0 && expected_len < 8 {
                value |= u64::MAX << (expected_len * 8);
            }
            Ok(ValueRef::Integer(value as i64))
        }
        7 => {
            if payload.len() != 8 {
                return Err(BtreeError::InvalidRecord("real payload has wrong length"));
            }
            let bits = u64::from_be_bytes([
                payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
                payload[7],
            ]);
            Ok(ValueRef::Real(f64::from_bits(bits)))
        }
        8 => Ok(ValueRef::Integer(0)),
        9 => Ok(ValueRef::Integer(1)),
        serial_type if serial_type >= 12 && serial_type % 2 == 0 => Ok(ValueRef::Blob(payload)),
        serial_type if serial_type >= 13 => {
            let text = std::str::from_utf8(payload).map_err(|_| BtreeError::InvalidUtf8)?;
            Ok(ValueRef::Text(text))
        }
        _ => Err(BtreeError::InvalidSerialType(serial_type)),
    }
}

fn varint_len(value: u64) -> usize {
    if value > 0x00ff_ffff_ffff_ffff {
        return 9;
    }
    let significant_bits = (u64::BITS - value.leading_zeros()) as usize;
    significant_bits.max(1).div_ceil(7)
}

fn read_u16(bytes: &[u8; PAGE_SIZE], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8; PAGE_SIZE], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u32_from_slice(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn write_u16(bytes: &mut [u8; PAGE_SIZE], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32(bytes: &mut [u8; PAGE_SIZE], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    enum GeneratedValue {
        Null,
        Integer(i64),
        Real(u64),
        Text(String),
        Blob(Vec<u8>),
    }

    fn generated_value_strategy() -> impl Strategy<Value = GeneratedValue> {
        prop_oneof![
            Just(GeneratedValue::Null),
            any::<i64>().prop_map(GeneratedValue::Integer),
            any::<u64>().prop_map(GeneratedValue::Real),
            prop::collection::vec(any::<char>(), 0..64)
                .prop_map(|chars| GeneratedValue::Text(chars.into_iter().collect())),
            prop::collection::vec(any::<u8>(), 0..96).prop_map(GeneratedValue::Blob),
        ]
    }

    struct MemoryPageTransaction {
        pages: Vec<Page>,
    }

    impl MemoryPageTransaction {
        fn new() -> Self {
            Self {
                pages: vec![Page::zeroed()],
            }
        }

        fn copy_page(page: &Page) -> Page {
            let mut copy = Page::zeroed();
            copy.as_mut_bytes().copy_from_slice(page.as_bytes());
            copy
        }
    }

    impl PageTransaction for MemoryPageTransaction {
        fn page_count(&self) -> u32 {
            u32::try_from(self.pages.len()).unwrap_or(u32::MAX)
        }

        fn allocate_page(&mut self) -> Result<PageNumber, BtreeError> {
            let page_number = u32::try_from(self.pages.len())
                .ok()
                .and_then(|count| count.checked_add(1))
                .ok_or(BtreeError::PageNumberExhausted)?;
            self.pages.push(Page::zeroed());
            PageNumber::new(page_number).map_err(|error| BtreeError::Transaction(error.to_string()))
        }

        fn read_page(&mut self, page_number: PageNumber) -> Result<Page, BtreeError> {
            let index = usize::try_from(page_number.get() - 1)
                .map_err(|_| BtreeError::InvalidTree("invalid page index"))?;
            self.pages.get(index).map(Self::copy_page).ok_or_else(|| {
                BtreeError::Transaction(format!(
                    "page {} is outside the in-memory transaction",
                    page_number.get()
                ))
            })
        }

        fn write_page(&mut self, page_number: PageNumber, page: &Page) -> Result<(), BtreeError> {
            let index = usize::try_from(page_number.get() - 1)
                .map_err(|_| BtreeError::InvalidTree("invalid page index"))?;
            let destination = self.pages.get_mut(index).ok_or_else(|| {
                BtreeError::Transaction(format!(
                    "page {} is outside the in-memory transaction",
                    page_number.get()
                ))
            })?;
            destination.as_mut_bytes().copy_from_slice(page.as_bytes());
            Ok(())
        }
    }

    fn page_number(value: u32) -> PageNumber {
        match PageNumber::new(value) {
            Ok(page_number) => page_number,
            Err(error) => panic!("test page number must be valid: {error}"),
        }
    }

    #[test]
    fn varint_round_trips_boundaries_from_one_to_nine_bytes() {
        let values = [
            0,
            127,
            128,
            255,
            16_383,
            16_384,
            (1_u64 << 56) - 1,
            1_u64 << 56,
            u64::MAX,
        ];
        for value in values {
            let mut encoded = [0; 9];
            let length = encode_varint(value, &mut encoded).expect("encode varint");
            let (decoded, decoded_length) =
                decode_varint(&encoded[..length]).expect("decode varint");
            assert_eq!(decoded, value);
            assert_eq!(decoded_length, length);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 1024,
            ..ProptestConfig::default()
        })]

        #[test]
        fn property_varints_round_trip_full_u64_and_nine_byte_range(
            value in prop_oneof![any::<u64>(), (1_u64 << 56)..=u64::MAX]
        ) {
            let mut encoded = [0; 9];
            let length = encode_varint(value, &mut encoded).expect("encode generated varint");
            let (decoded, decoded_length) =
                decode_varint(&encoded[..length]).expect("decode generated varint");

            prop_assert_eq!(decoded, value);
            prop_assert_eq!(decoded_length, length);
            prop_assert_eq!(length, varint_len(value));
        }

        #[test]
        fn property_records_round_trip_generated_values(
            generated_values in prop::collection::vec(generated_value_strategy(), 0..48)
        ) {
            let values: Vec<_> = generated_values
                .iter()
                .map(|value| match value {
                    GeneratedValue::Null => Value::Null,
                    GeneratedValue::Integer(value) => Value::Integer(*value),
                    GeneratedValue::Real(bits) => Value::Real(f64::from_bits(*bits)),
                    GeneratedValue::Text(value) => Value::Text(value),
                    GeneratedValue::Blob(value) => Value::Blob(value),
                })
                .collect();
            let encoded = encode_record(&values).expect("encode generated record");
            let record = RecordRef::new(&encoded).expect("decode generated record");

            prop_assert_eq!(record.field_count(), generated_values.len());
            for (index, expected) in generated_values.iter().enumerate() {
                let actual = record.get(index).expect("read generated field")
                    .expect("generated field exists");
                match (expected, actual) {
                    (GeneratedValue::Null, ValueRef::Null) => {}
                    (GeneratedValue::Integer(expected), ValueRef::Integer(actual)) => {
                        prop_assert_eq!(*expected, actual);
                    }
                    (GeneratedValue::Real(expected), ValueRef::Real(actual)) => {
                        prop_assert_eq!(*expected, actual.to_bits());
                    }
                    (GeneratedValue::Text(expected), ValueRef::Text(actual)) => {
                        prop_assert_eq!(expected.as_str(), actual);
                    }
                    (GeneratedValue::Blob(expected), ValueRef::Blob(actual)) => {
                        prop_assert_eq!(expected.as_slice(), actual);
                    }
                    (expected, actual) => {
                        prop_assert!(false, "record value mismatch: {expected:?} vs {actual:?}");
                    }
                }
            }
        }

        #[test]
        fn property_slotted_page_insertions_preserve_cells(
            cells in prop::collection::vec(
                (any::<u16>(), prop::collection::vec(any::<u8>(), 1..100)),
                0..120
            )
        ) {
            let mut page = Page::zeroed();
            let mut expected = Vec::new();
            {
                let mut slotted = SlottedPage::initialize(
                    &mut page,
                    page_number(2),
                    PageKind::TableLeaf,
                    None,
                )
                .expect("initialize generated slotted page");

                for (slot_seed, cell) in cells {
                    let slot = usize::from(slot_seed) % (expected.len() + 1);
                    match slotted.insert_cell(slot, &cell) {
                        Ok(_) => expected.insert(slot, cell),
                        Err(BtreeError::NoSpace) => break,
                        Err(error) => {
                            return Err(TestCaseError::fail(format!(
                                "generated cell insertion failed unexpectedly: {error}"
                            )));
                        }
                    }
                }

                prop_assert_eq!(slotted.cell_count(), expected.len());
                for (slot, expected_cell) in expected.iter().enumerate() {
                    prop_assert_eq!(
                        slotted.cell_bytes(slot, expected_cell.len()),
                        Ok(expected_cell.as_slice())
                    );
                }
            }

            let reopened = SlottedPage::open(&mut page, page_number(2))
                .expect("reopen generated slotted page");
            prop_assert_eq!(reopened.cell_count(), expected.len());
            for (slot, expected_cell) in expected.iter().enumerate() {
                prop_assert_eq!(
                    reopened.cell_bytes(slot, expected_cell.len()),
                    Ok(expected_cell.as_slice())
                );
            }
        }

        #[test]
        fn property_table_btree_insertions_split_and_preserve_rows(
            rowids in prop::collection::btree_set(any::<i32>(), 90..140),
            seed in any::<u64>()
        ) {
            let mut transaction = MemoryPageTransaction::new();
            let tree = TableBtree::initialize(&mut transaction)
                .expect("initialize generated table tree");
            let mut rowids: Vec<_> = rowids.into_iter().collect();
            rowids.sort_by_key(|rowid| {
                ((*rowid as u32).wrapping_mul(0x9e37_79b9))
                    .rotate_left((seed & 31) as u32)
            });

            let mut expected = Vec::new();
            for rowid in rowids {
                let payload_len = 48 + ((rowid as u32).wrapping_mul(31) % 65) as usize;
                let payload = vec![(rowid as u8) ^ seed as u8; payload_len];
                let encoded = encode_record(&[
                    Value::Integer(i64::from(rowid)),
                    Value::Blob(&payload),
                ]).expect("encode generated B-tree record");
                tree.insert(&mut transaction, i64::from(rowid), &encoded)
                    .expect("insert generated row");
                expected.push((i64::from(rowid), encoded));
            }
            expected.sort_by_key(|(rowid, _)| *rowid);

            let stats = tree.validate(&mut transaction).expect("validate split B-tree");
            prop_assert!(stats.pages > 1);
            prop_assert!(stats.height > 1);
            prop_assert_eq!(tree.scan(&mut transaction).expect("scan split B-tree"), expected);
        }
    }

    #[test]
    fn varint_reports_short_output_and_truncated_input() {
        let mut output = [0; 1];
        assert_eq!(
            encode_varint(128, &mut output),
            Err(BtreeError::VarintOutputTooSmall {
                required: 2,
                available: 1
            })
        );
        assert_eq!(decode_varint(&[0x80]), Err(BtreeError::TruncatedVarint));
        assert_eq!(decode_varint(&[0xff; 8]), Err(BtreeError::TruncatedVarint));
    }

    #[test]
    fn initializes_page_one_without_overwriting_database_header() {
        let mut page = Page::zeroed();
        page.as_mut_bytes()[..DATABASE_HEADER_SIZE].fill(0xa5);
        {
            let slotted =
                SlottedPage::initialize(&mut page, page_number(1), PageKind::TableLeaf, None)
                    .expect("initialize page 1");
            assert_eq!(slotted.cell_count(), 0);
        }
        assert_eq!(
            &page.as_bytes()[..DATABASE_HEADER_SIZE],
            &[0xa5; DATABASE_HEADER_SIZE]
        );
        assert_eq!(
            page.as_bytes()[DATABASE_HEADER_SIZE],
            PageKind::TableLeaf as u8
        );
    }

    #[test]
    fn inserts_cells_at_requested_slots_and_reopens_page() {
        let mut page = Page::zeroed();
        {
            let mut slotted =
                SlottedPage::initialize(&mut page, page_number(2), PageKind::TableLeaf, None)
                    .expect("initialize leaf page");

            let high_offset = slotted.insert_cell(0, b"high").expect("insert first cell");
            let low_offset = slotted.insert_cell(0, b"low").expect("insert sorted cell");
            assert_eq!(slotted.cell_count(), 2);
            assert_eq!(slotted.cell_offset(0), Ok(low_offset));
            assert_eq!(slotted.cell_offset(1), Ok(high_offset));
        }

        let slotted = SlottedPage::open(&mut page, page_number(2)).expect("reopen valid page");
        assert_eq!(slotted.kind(), PageKind::TableLeaf);
        assert_eq!(slotted.cell_count(), 2);
    }

    #[test]
    fn interior_page_round_trips_rightmost_child() {
        let mut page = Page::zeroed();
        {
            let slotted = SlottedPage::initialize(
                &mut page,
                page_number(2),
                PageKind::TableInterior,
                Some(page_number(7)),
            )
            .expect("initialize interior page");
            assert_eq!(slotted.rightmost_child(), Some(page_number(7)));
        }

        let slotted = SlottedPage::open(&mut page, page_number(2)).expect("reopen interior page");
        assert_eq!(slotted.rightmost_child(), Some(page_number(7)));
    }

    #[test]
    fn rejects_invalid_slots_pages_and_cells() {
        let mut page = Page::zeroed();
        {
            let mut slotted =
                SlottedPage::initialize(&mut page, page_number(2), PageKind::TableLeaf, None)
                    .expect("initialize leaf page");
            assert_eq!(
                slotted.insert_cell(1, b"x"),
                Err(BtreeError::InvalidSlot {
                    slot: 1,
                    cell_count: 0
                })
            );
            assert_eq!(slotted.insert_cell(0, b""), Err(BtreeError::EmptyCell));
            assert_eq!(
                slotted.cell_offset(0),
                Err(BtreeError::InvalidSlot {
                    slot: 0,
                    cell_count: 0
                })
            );
            assert!(matches!(
                slotted.insert_cell(0, &[0; PAGE_SIZE]),
                Err(BtreeError::NoSpace)
            ));
        }
        assert_eq!(
            SlottedPage::initialize(&mut page, page_number(2), PageKind::TableInterior, None).err(),
            Some(BtreeError::MissingRightmostChild)
        );
    }

    #[test]
    fn rejects_corrupt_cell_pointers_and_unknown_page_kind() {
        let mut page = Page::zeroed();
        {
            let mut slotted =
                SlottedPage::initialize(&mut page, page_number(2), PageKind::TableLeaf, None)
                    .expect("initialize leaf page");
            slotted.insert_cell(0, b"cell").expect("insert cell");
        }
        page.as_mut_bytes()[8..10].copy_from_slice(&1_u16.to_be_bytes());
        assert!(matches!(
            SlottedPage::open(&mut page, page_number(2)),
            Err(BtreeError::CorruptPage(_))
        ));

        page.as_mut_bytes()[0] = 0xff;
        assert_eq!(
            SlottedPage::open(&mut page, page_number(2)).err(),
            Some(BtreeError::InvalidPageKind(0xff))
        );
    }

    #[test]
    fn rejects_corrupt_freeblock_chain_and_fragment_count() {
        let mut page = Page::zeroed();
        SlottedPage::initialize(&mut page, page_number(2), PageKind::TableLeaf, None)
            .expect("initialize leaf page");

        page.as_mut_bytes()[1..3].copy_from_slice(&4095_u16.to_be_bytes());
        assert!(matches!(
            SlottedPage::open(&mut page, page_number(2)),
            Err(BtreeError::CorruptPage(_))
        ));

        page.as_mut_bytes()[1..3].copy_from_slice(&0_u16.to_be_bytes());
        page.as_mut_bytes()[7] = 61;
        assert!(matches!(
            SlottedPage::open(&mut page, page_number(2)),
            Err(BtreeError::CorruptPage(_))
        ));
    }

    #[test]
    fn record_round_trips_all_supported_serial_types() {
        let values = [
            Value::Null,
            Value::Integer(0),
            Value::Integer(1),
            Value::Integer(i8::MIN as i64),
            Value::Integer(i8::MAX as i64),
            Value::Integer(i16::MIN as i64),
            Value::Integer(i16::MAX as i64),
            Value::Integer(-8_388_608),
            Value::Integer(8_388_607),
            Value::Integer(i32::MIN as i64),
            Value::Integer(i32::MAX as i64),
            Value::Integer(-(1_i64 << 47)),
            Value::Integer((1_i64 << 47) - 1),
            Value::Integer(i64::MIN),
            Value::Integer(i64::MAX),
            Value::Real(-123.5),
            Value::Text("Ferrite \u{03bb}"),
            Value::Blob(&[0, 0xff, 0x7f]),
        ];
        let encoded = encode_record(&values).expect("encode record");
        let record = RecordRef::new(&encoded).expect("decode record");

        assert_eq!(record.field_count(), values.len());
        assert_eq!(record.get(0), Ok(Some(ValueRef::Null)));
        assert_eq!(record.get(1), Ok(Some(ValueRef::Integer(0))));
        assert_eq!(record.get(2), Ok(Some(ValueRef::Integer(1))));
        for (index, value) in values.iter().enumerate().skip(3).take(12) {
            let Value::Integer(expected) = value else {
                panic!("expected integer in test fixture");
            };
            assert_eq!(record.get(index), Ok(Some(ValueRef::Integer(*expected))));
        }
        assert_eq!(record.get(15), Ok(Some(ValueRef::Real(-123.5))));
        assert_eq!(record.get(16), Ok(Some(ValueRef::Text("Ferrite \u{03bb}"))));
        assert_eq!(
            record.get(17),
            Ok(Some(ValueRef::Blob(&[0, 0xff, 0x7f][..])))
        );
        assert_eq!(record.get(18), Ok(None));
        assert_eq!(record.values().count(), values.len());
    }

    #[test]
    fn record_blob_reads_borrow_the_encoded_buffer() {
        let blob = [1, 2, 3, 4];
        let encoded = encode_record(&[Value::Blob(&blob)]).expect("encode blob record");
        let record = RecordRef::new(&encoded).expect("decode blob record");
        let ValueRef::Blob(decoded) = record.get(0).expect("read blob").expect("field exists")
        else {
            panic!("expected blob");
        };
        let (header_size, _) = decode_varint(&encoded).expect("read header size");

        assert_eq!(decoded, blob);
        assert_eq!(decoded.as_ptr(), encoded[header_size as usize..].as_ptr());
    }

    #[test]
    fn record_header_size_varint_handles_many_columns() {
        let values = vec![Value::Null; 130];
        let encoded = encode_record(&values).expect("encode wide record");
        let (header_size, prefix_size) = decode_varint(&encoded).expect("decode header size");

        assert!(header_size > 127);
        assert_eq!(prefix_size, 2);
        assert_eq!(
            RecordRef::new(&encoded)
                .expect("decode wide record")
                .field_count(),
            130
        );
    }

    #[test]
    fn rejects_truncated_and_malformed_records() {
        for bytes in [
            &[0x80][..],
            &[2, 0x80][..],
            &[2, 10][..],
            &[2, 1][..],
            &[2, 0, 0xff][..],
            &[2, 13, 0xff][..],
            &[2][..],
        ] {
            assert!(RecordRef::new(bytes).is_err(), "accepted {bytes:?}");
        }
        assert!(matches!(
            RecordRef::new(&[2, 10]),
            Err(BtreeError::InvalidSerialType(10))
        ));
        assert_eq!(
            RecordRef::new(&[2, 15, 0xff]).err(),
            Some(BtreeError::InvalidUtf8)
        );
    }

    #[test]
    fn table_tree_inserts_searches_and_rejects_duplicate_rowids() {
        let mut transaction = MemoryPageTransaction::new();
        transaction.pages[0].as_mut_bytes()[..DATABASE_HEADER_SIZE].fill(0xa5);
        let tree = TableBtree::initialize(&mut transaction).expect("initialize table tree");
        let first = encode_record(&[Value::Integer(-7), Value::Text("first")])
            .expect("encode first record");
        let second = encode_record(&[Value::Integer(0)]).expect("encode second record");
        let third = encode_record(&[Value::Integer(i64::MAX)]).expect("encode third record");

        tree.insert(&mut transaction, -10, &first)
            .expect("insert negative rowid");
        tree.insert(&mut transaction, 0, &second)
            .expect("insert zero rowid");
        tree.insert(&mut transaction, i64::MAX, &third)
            .expect("insert maximum rowid");

        assert_eq!(
            tree.get(&mut transaction, -10).expect("search first row"),
            Some(first)
        );
        assert_eq!(
            tree.get(&mut transaction, 0).expect("search second row"),
            Some(second.clone())
        );
        assert_eq!(
            tree.get(&mut transaction, 1).expect("search missing row"),
            None
        );
        assert_eq!(
            tree.insert(&mut transaction, 0, &second),
            Err(BtreeError::DuplicateRowId(0))
        );
        let stats = tree
            .validate(&mut transaction)
            .expect("validate table tree");
        assert_eq!(
            stats,
            TreeStats {
                pages: 1,
                rows: 3,
                height: 1
            }
        );
        assert_eq!(
            &transaction.pages[0].as_bytes()[..DATABASE_HEADER_SIZE],
            &[0xa5; DATABASE_HEADER_SIZE]
        );
    }

    #[test]
    fn table_tree_splits_leaf_and_interior_pages_and_validates_ranges() {
        let mut transaction = MemoryPageTransaction::new();
        let tree = TableBtree::initialize(&mut transaction).expect("initialize table tree");
        let blob = vec![0x5a; 1_950];
        let record = encode_record(&[Value::Blob(&blob)]).expect("encode large record");
        for index in 0..1_500 {
            let rowid = ((index * 137) % 1_500) as i64 - 750;
            tree.insert(&mut transaction, rowid, &record)
                .expect("insert table row");
        }

        let reopened = TableBtree::open(&mut transaction).expect("open table tree");
        for rowid in -750..750 {
            assert_eq!(
                reopened
                    .get(&mut transaction, rowid)
                    .expect("search table row"),
                Some(record.clone())
            );
        }
        assert_eq!(
            reopened
                .get(&mut transaction, -751)
                .expect("search missing row"),
            None
        );
        let stats = reopened
            .validate(&mut transaction)
            .expect("validate split table tree");
        assert_eq!(stats.rows, 1_500);
        assert!(stats.pages > 500);
        assert_eq!(stats.pages, transaction.page_count());
        assert!(
            stats.height >= 3,
            "tree did not split its interior root: {stats:?}"
        );
    }

    #[test]
    fn table_tree_rejects_oversized_rows_without_mutating_the_root() {
        let mut transaction = MemoryPageTransaction::new();
        let tree = TableBtree::initialize(&mut transaction).expect("initialize table tree");
        let blob = vec![0x7f; PAGE_SIZE + 32];
        let record = encode_record(&[Value::Blob(&blob)]).expect("encode oversized record");

        assert_eq!(
            tree.insert(&mut transaction, 1, &record),
            Err(BtreeError::CellTooLarge)
        );
        assert_eq!(
            tree.validate(&mut transaction)
                .expect("validate empty tree"),
            TreeStats {
                pages: 1,
                rows: 0,
                height: 1
            }
        );
    }

    #[test]
    fn table_tree_integrity_check_rejects_a_cyclic_page_reference() {
        let mut transaction = MemoryPageTransaction::new();
        let root = page_number(1);
        let mut page = Page::zeroed();
        {
            let mut slotted =
                SlottedPage::initialize(&mut page, root, PageKind::TableInterior, Some(root))
                    .expect("initialize cyclic root");
            slotted
                .insert_cell(0, &[0, 0, 0, 1, 0])
                .expect("insert cyclic child");
        }
        transaction.pages[0] = page;
        let tree = TableBtree::open(&mut transaction).expect("open corrupt tree root");

        assert_eq!(
            tree.validate(&mut transaction),
            Err(BtreeError::InvalidTree(
                "tree references a page more than once"
            ))
        );
    }

    #[test]
    fn index_tree_orders_composite_keys_and_allows_duplicate_keys() {
        assert_eq!(
            index::compare_values(
                ValueRef::Integer((1_i64 << 53) + 1),
                ValueRef::Real((1_u64 << 53) as f64)
            ),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            index::compare_values(ValueRef::Integer(10), ValueRef::Real(10.5)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            index::compare_values(ValueRef::Integer(1), ValueRef::Real(1.0)),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            index::compare_values(ValueRef::Real(-0.0), ValueRef::Real(0.0)),
            std::cmp::Ordering::Equal
        );

        let mut transaction = MemoryPageTransaction::new();
        let root = transaction.allocate_page().expect("allocate index root");
        let tree =
            index::IndexBtree::initialize(&mut transaction, root).expect("initialize index tree");

        tree.insert(
            &mut transaction,
            &[Value::Text("alpha"), Value::Integer(1)],
            9,
        )
        .expect("insert first matching key");
        tree.insert(
            &mut transaction,
            &[Value::Text("alpha"), Value::Integer(1)],
            2,
        )
        .expect("insert duplicate key with different rowid");
        tree.insert(
            &mut transaction,
            &[Value::Text("alpha"), Value::Integer(2)],
            2,
        )
        .expect("insert distinct composite key");
        assert_eq!(
            tree.insert(
                &mut transaction,
                &[Value::Text("alpha"), Value::Integer(1)],
                2
            ),
            Err(BtreeError::DuplicateIndexEntry(2))
        );

        assert_eq!(
            tree.find(&mut transaction, &[Value::Text("alpha"), Value::Integer(1)])
                .expect("find duplicate keys"),
            vec![2, 9]
        );
        assert_eq!(
            tree.find(&mut transaction, &[Value::Text("alpha"), Value::Integer(3)])
                .expect("find absent key"),
            Vec::<i64>::new()
        );
        assert_eq!(
            tree.validate(&mut transaction)
                .expect("validate index tree"),
            TreeStats {
                pages: 1,
                rows: 3,
                height: 1,
            }
        );
    }

    #[test]
    fn index_tree_splits_leaf_and_interior_pages() {
        let mut transaction = MemoryPageTransaction::new();
        let root = transaction.allocate_page().expect("allocate index root");
        let tree =
            index::IndexBtree::initialize(&mut transaction, root).expect("initialize index tree");
        let blob = vec![0x5a; 1_900];
        for index in (0..256).rev() {
            tree.insert(
                &mut transaction,
                &[Value::Integer(index), Value::Blob(&blob)],
                index,
            )
            .expect("insert composite index entry");
        }
        for rowid in 10_000..10_003 {
            tree.insert(
                &mut transaction,
                &[Value::Integer(37), Value::Blob(&blob)],
                rowid,
            )
            .expect("insert duplicate composite key across leaf splits");
        }

        let reopened = index::IndexBtree::open(&mut transaction, root).expect("reopen index tree");
        for index in [0, 127, 255] {
            assert_eq!(
                reopened
                    .find(
                        &mut transaction,
                        &[Value::Integer(index), Value::Blob(&blob)]
                    )
                    .expect("find indexed row"),
                vec![index]
            );
        }
        assert_eq!(
            reopened
                .find(&mut transaction, &[Value::Integer(37), Value::Blob(&blob)])
                .expect("find duplicate keys across splits"),
            vec![37, 10_000, 10_001, 10_002]
        );
        assert!(
            reopened
                .remove(
                    &mut transaction,
                    &[Value::Integer(37), Value::Blob(&blob)],
                    10_001
                )
                .expect("remove one duplicate index entry")
        );
        assert_eq!(
            reopened
                .find(&mut transaction, &[Value::Integer(37), Value::Blob(&blob)])
                .expect("find remaining duplicate keys"),
            vec![37, 10_000, 10_002]
        );
        let stats = reopened
            .validate(&mut transaction)
            .expect("validate split index tree");
        assert_eq!(stats.rows, 258);
        assert!(stats.pages < transaction.page_count());
        assert!(stats.height >= 3, "index root did not split: {stats:?}");
    }

    #[test]
    fn index_tree_integrity_check_rejects_a_cyclic_page_reference() {
        let mut transaction = MemoryPageTransaction::new();
        let root = transaction.allocate_page().expect("allocate index root");
        let mut page = Page::zeroed();
        {
            let mut slotted =
                SlottedPage::initialize(&mut page, root, PageKind::IndexInterior, Some(root))
                    .expect("initialize cyclic index root");
            slotted
                .insert_cell(0, &[0, 0, 0, 2, 2, 0, 2, 9])
                .expect("insert cyclic child");
        }
        transaction
            .write_page(root, &page)
            .expect("write cyclic index root");
        let tree =
            index::IndexBtree::open(&mut transaction, root).expect("open corrupt index root");

        assert_eq!(
            tree.validate(&mut transaction),
            Err(BtreeError::InvalidTree(
                "index references a page more than once"
            ))
        );
    }
}
