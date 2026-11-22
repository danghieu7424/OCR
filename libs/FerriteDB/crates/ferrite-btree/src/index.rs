use super::*;
use std::cmp::Ordering;

const MAX_INDEX_DEPTH: u32 = 64;

#[derive(Clone, Debug)]
struct Entry {
    record: Vec<u8>,
    rowid: i64,
    encoded: Vec<u8>,
}

impl Entry {
    fn new(record: Vec<u8>, rowid: i64) -> Result<Self, BtreeError> {
        RecordRef::new(&record)?;
        let record_length = u64::try_from(record.len()).map_err(|_| BtreeError::RecordTooLarge)?;
        let capacity = varint_len(record_length)
            .checked_add(varint_len(rowid as u64))
            .and_then(|length| length.checked_add(record.len()))
            .ok_or(BtreeError::RecordTooLarge)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(capacity)
            .map_err(|_| BtreeError::RecordAllocation)?;
        let mut varint = [0; 9];
        let length = encode_varint(record_length, &mut varint)?;
        encoded.extend_from_slice(&varint[..length]);
        let length = encode_varint(rowid as u64, &mut varint)?;
        encoded.extend_from_slice(&varint[..length]);
        encoded.extend_from_slice(&record);
        Ok(Self {
            record,
            rowid,
            encoded,
        })
    }

    fn compare(&self, other: &Self) -> Result<Ordering, BtreeError> {
        Ok(
            compare_records(&self.record, &other.record)?
                .then_with(|| self.rowid.cmp(&other.rowid)),
        )
    }
}

#[derive(Clone, Debug)]
struct Node {
    children: Vec<PageNumber>,
    separators: Vec<Entry>,
}

#[derive(Clone, Debug)]
struct Split {
    separator: Entry,
    right_page: PageNumber,
}

/// Persistent composite-key index. Keys sort by NULL, numeric value, text bytes,
/// then blob bytes; text comparison is binary and case-sensitive.
#[derive(Clone, Copy, Debug)]
pub struct IndexBtree {
    root: PageNumber,
}

impl IndexBtree {
    pub fn initialize(
        transaction: &mut impl PageTransaction,
        root: PageNumber,
    ) -> Result<Self, BtreeError> {
        if root.get() > transaction.page_count() {
            return Err(BtreeError::InvalidTree(
                "index root page is outside the database",
            ));
        }
        let mut page = transaction.read_page(root)?;
        SlottedPage::initialize(&mut page, root, PageKind::IndexLeaf, None)?;
        transaction.write_page(root, &page)?;
        Ok(Self { root })
    }

    pub fn open(
        transaction: &mut impl PageTransaction,
        root: PageNumber,
    ) -> Result<Self, BtreeError> {
        if root.get() > transaction.page_count() {
            return Err(BtreeError::InvalidTree(
                "index root page is outside the database",
            ));
        }
        let mut page = transaction.read_page(root)?;
        index_page_kind(&mut page, root)?;
        Ok(Self { root })
    }

    pub fn insert(
        &self,
        transaction: &mut impl PageTransaction,
        fields: &[Value<'_>],
        rowid: i64,
    ) -> Result<(), BtreeError> {
        let entry = Entry::new(encode_record(fields)?, rowid)?;
        if insert_recursive(transaction, self.root, self.root, entry, 0)?.is_some() {
            return Err(BtreeError::InvalidTree(
                "root insertion left an unpropagated index split",
            ));
        }
        Ok(())
    }

    pub fn remove(
        &self,
        transaction: &mut impl PageTransaction,
        fields: &[Value<'_>],
        rowid: i64,
    ) -> Result<bool, BtreeError> {
        let target = Entry::new(encode_record(fields)?, rowid)?;
        Ok(remove_recursive(transaction, self.root, self.root, &target, 0)?.is_some())
    }

    pub fn find(
        &self,
        transaction: &mut impl PageTransaction,
        fields: &[Value<'_>],
    ) -> Result<Vec<i64>, BtreeError> {
        let record = encode_record(fields)?;
        let mut matches = Vec::new();
        let mut visited = HashSet::new();
        find_recursive(
            transaction,
            self.root,
            &record,
            0,
            &mut visited,
            &mut matches,
        )?;
        Ok(matches)
    }

    pub fn validate(
        &self,
        transaction: &mut impl PageTransaction,
    ) -> Result<TreeStats, BtreeError> {
        let page_count = transaction.page_count();
        if page_count == 0 || self.root.get() > page_count {
            return Err(BtreeError::InvalidTree(
                "index root page is outside the database",
            ));
        }
        let mut visited = HashSet::new();
        let result = validate_subtree(
            transaction,
            self.root,
            page_count,
            None,
            None,
            0,
            &mut visited,
        )?;
        Ok(TreeStats {
            pages: result.pages,
            rows: result.rows,
            height: result.height,
        })
    }
}

fn index_page_kind(page: &mut Page, page_number: PageNumber) -> Result<PageKind, BtreeError> {
    let kind = SlottedPage::open(page, page_number)?.kind();
    if matches!(kind, PageKind::IndexLeaf | PageKind::IndexInterior) {
        Ok(kind)
    } else {
        Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        })
    }
}

fn compare_records(left: &[u8], right: &[u8]) -> Result<Ordering, BtreeError> {
    let left = RecordRef::new(left)?;
    let right = RecordRef::new(right)?;
    let mut left_values = left.values();
    let mut right_values = right.values();
    loop {
        match (left_values.next(), right_values.next()) {
            (Some(Ok(left)), Some(Ok(right))) => {
                let ordering = compare_values(left, right);
                if ordering != Ordering::Equal {
                    return Ok(ordering);
                }
            }
            (Some(Err(error)), _) | (_, Some(Err(error))) => return Err(error),
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
        }
    }
}

pub(super) fn compare_values(left: ValueRef<'_>, right: ValueRef<'_>) -> Ordering {
    match (left, right) {
        (ValueRef::Null, ValueRef::Null) => Ordering::Equal,
        (ValueRef::Null, _) => Ordering::Less,
        (_, ValueRef::Null) => Ordering::Greater,
        (ValueRef::Integer(left), ValueRef::Integer(right)) => left.cmp(&right),
        (ValueRef::Integer(integer), ValueRef::Real(real)) => compare_integer_real(integer, real),
        (ValueRef::Real(real), ValueRef::Integer(integer)) => {
            compare_integer_real(integer, real).reverse()
        }
        (ValueRef::Real(left), ValueRef::Real(right)) => match (left.is_nan(), right.is_nan()) {
            (true, true) => left.total_cmp(&right),
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) if left == right => Ordering::Equal,
            (false, false) => left.total_cmp(&right),
        },
        (ValueRef::Text(left), ValueRef::Text(right)) => left.as_bytes().cmp(right.as_bytes()),
        (ValueRef::Text(_), ValueRef::Blob(_)) => Ordering::Less,
        (ValueRef::Blob(_), ValueRef::Text(_)) => Ordering::Greater,
        (ValueRef::Text(_), _) => Ordering::Greater,
        (_, ValueRef::Text(_)) => Ordering::Less,
        (ValueRef::Blob(left), ValueRef::Blob(right)) => left.cmp(right),
        (ValueRef::Blob(_), _) => Ordering::Greater,
        (_, ValueRef::Blob(_)) => Ordering::Less,
    }
}

fn compare_integer_real(integer: i64, real: f64) -> Ordering {
    if real.is_nan() {
        return Ordering::Less;
    }
    const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
    const I64_MAX_EXCLUSIVE_AS_F64: f64 = 9_223_372_036_854_775_808.0;
    if real < I64_MIN_AS_F64 {
        return Ordering::Greater;
    }
    if real >= I64_MAX_EXCLUSIVE_AS_F64 {
        return Ordering::Less;
    }
    match integer.cmp(&(real as i64)) {
        Ordering::Equal if real.fract() > 0.0 => Ordering::Less,
        Ordering::Equal if real.fract() < 0.0 => Ordering::Greater,
        ordering => ordering,
    }
}

fn read_leaf_entries(page: &mut Page, page_number: PageNumber) -> Result<Vec<Entry>, BtreeError> {
    let slotted = SlottedPage::open(page, page_number)?;
    if slotted.kind() != PageKind::IndexLeaf {
        return Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        });
    }
    let ranges = index_cell_ranges(&slotted)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for (slot, (offset, end)) in ranges.into_iter().enumerate() {
        let available = end - offset;
        let cell = slotted.cell_bytes(slot, available)?;
        let (record_length, length_size) = decode_varint(cell)?;
        let (rowid, rowid_size) = decode_varint(&cell[length_size..])?;
        let record_start = length_size
            .checked_add(rowid_size)
            .ok_or(BtreeError::RecordTooLarge)?;
        let record_end = record_start
            .checked_add(usize::try_from(record_length).map_err(|_| BtreeError::RecordTooLarge)?)
            .filter(|end| *end == available)
            .ok_or(BtreeError::CorruptPage(
                "index leaf cell length does not match its page range",
            ))?;
        let record = copy_slice(&cell[record_start..record_end])?;
        let encoded = copy_slice(cell)?;
        entries.push(Entry {
            record,
            rowid: rowid as i64,
            encoded,
        });
    }
    for pair in entries.windows(2) {
        if pair[0].compare(&pair[1])? != Ordering::Less {
            return Err(BtreeError::CorruptPage(
                "index leaf keys are not strictly increasing",
            ));
        }
    }
    Ok(entries)
}

fn read_node(
    page: &mut Page,
    page_number: PageNumber,
    page_count: u32,
) -> Result<Node, BtreeError> {
    let slotted = SlottedPage::open(page, page_number)?;
    if slotted.kind() != PageKind::IndexInterior {
        return Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        });
    }
    let ranges = index_cell_ranges(&slotted)?;
    let mut children = Vec::new();
    let mut separators = Vec::new();
    children
        .try_reserve_exact(slotted.cell_count().saturating_add(1))
        .map_err(|_| BtreeError::RecordAllocation)?;
    separators
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for (slot, (offset, end)) in ranges.into_iter().enumerate() {
        let cell = slotted.cell_bytes(slot, end - offset)?;
        if cell.len() < 5 {
            return Err(BtreeError::CorruptPage("index interior cell is too short"));
        }
        let child = PageNumber::new(read_u32_from_slice(cell))
            .map_err(|_| BtreeError::CorruptPage("index cell has a zero child"))?;
        if child.get() > page_count {
            return Err(BtreeError::InvalidTree(
                "index interior cell references a page outside the database",
            ));
        }
        let (record_length, length_size) = decode_varint(&cell[4..])?;
        let (rowid, rowid_size) = decode_varint(&cell[4 + length_size..])?;
        let record_start = 4_usize
            .checked_add(length_size)
            .and_then(|length| length.checked_add(rowid_size))
            .ok_or(BtreeError::RecordTooLarge)?;
        let record_end = record_start
            .checked_add(usize::try_from(record_length).map_err(|_| BtreeError::RecordTooLarge)?)
            .filter(|end| *end == cell.len())
            .ok_or(BtreeError::CorruptPage(
                "index interior cell length is invalid",
            ))?;
        separators.push(Entry::new(
            copy_slice(&cell[record_start..record_end])?,
            rowid as i64,
        )?);
        children.push(child);
    }
    let rightmost = slotted.rightmost_child().ok_or(BtreeError::CorruptPage(
        "index interior page has no right-most child",
    ))?;
    if rightmost.get() > page_count {
        return Err(BtreeError::InvalidTree(
            "index right-most child is outside the database",
        ));
    }
    children.push(rightmost);
    for pair in separators.windows(2) {
        if pair[0].compare(&pair[1])? != Ordering::Less {
            return Err(BtreeError::CorruptPage(
                "index interior separators are not strictly increasing",
            ));
        }
    }
    Ok(Node {
        children,
        separators,
    })
}

fn insert_recursive(
    transaction: &mut impl PageTransaction,
    root: PageNumber,
    page_number: PageNumber,
    entry: Entry,
    depth: u32,
) -> Result<Option<Split>, BtreeError> {
    if depth >= MAX_INDEX_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match index_page_kind(&mut page, page_number)? {
        PageKind::IndexLeaf => {
            let mut entries = read_leaf_entries(&mut page, page_number)?;
            let mut insert_at = entries.len();
            for (index, existing) in entries.iter().enumerate() {
                match existing.compare(&entry)? {
                    Ordering::Equal => return Err(BtreeError::DuplicateIndexEntry(entry.rowid)),
                    Ordering::Greater => {
                        insert_at = index;
                        break;
                    }
                    Ordering::Less => {}
                }
            }
            entries
                .try_reserve(1)
                .map_err(|_| BtreeError::RecordAllocation)?;
            entries.insert(insert_at, entry);
            match pack_leaf(page_number, &entries, page) {
                Ok(packed) => {
                    transaction.write_page(page_number, &packed)?;
                    Ok(None)
                }
                Err(BtreeError::NoSpace) => {
                    let split = leaf_split_point(&entries)?;
                    let separator = entries[split - 1].clone();
                    if page_number == root {
                        let left_page = allocate_page(transaction)?;
                        let right_page = allocate_page(transaction)?;
                        let left = pack_leaf(left_page, &entries[..split], Page::zeroed())?;
                        let right = pack_leaf(right_page, &entries[split..], Page::zeroed())?;
                        transaction.write_page(left_page, &left)?;
                        transaction.write_page(right_page, &right)?;
                        let root_page = transaction.read_page(page_number)?;
                        let root_node = root_node(left_page, right_page, separator)?;
                        let packed = pack_node(page_number, &root_node, root_page)?;
                        transaction.write_page(page_number, &packed)?;
                        Ok(None)
                    } else {
                        let left = pack_leaf(page_number, &entries[..split], Page::zeroed())?;
                        let right_page = allocate_page(transaction)?;
                        let right = pack_leaf(right_page, &entries[split..], Page::zeroed())?;
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
        PageKind::IndexInterior => {
            let mut node = read_node(&mut page, page_number, transaction.page_count())?;
            let mut child_index = node.separators.len();
            for (index, separator) in node.separators.iter().enumerate() {
                if entry.compare(separator)? != Ordering::Greater {
                    child_index = index;
                    break;
                }
            }
            let child = node.children[child_index];
            let Some(split) = insert_recursive(transaction, root, child, entry, depth + 1)? else {
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
            match pack_node(page_number, &node, page) {
                Ok(packed) => {
                    transaction.write_page(page_number, &packed)?;
                    Ok(None)
                }
                Err(BtreeError::NoSpace) => {
                    let (left_node, separator, right_node) = split_node(&node)?;
                    if page_number == root {
                        let left_page = allocate_page(transaction)?;
                        let right_page = allocate_page(transaction)?;
                        let left = pack_node(left_page, &left_node, Page::zeroed())?;
                        let right = pack_node(right_page, &right_node, Page::zeroed())?;
                        transaction.write_page(left_page, &left)?;
                        transaction.write_page(right_page, &right)?;
                        let root = transaction.read_page(page_number)?;
                        let root_node = root_node(left_page, right_page, separator)?;
                        let packed = pack_node(page_number, &root_node, root)?;
                        transaction.write_page(page_number, &packed)?;
                        Ok(None)
                    } else {
                        let left = pack_node(page_number, &left_node, Page::zeroed())?;
                        let right_page = allocate_page(transaction)?;
                        let right = pack_node(right_page, &right_node, Page::zeroed())?;
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

struct RemovedEntry {
    maximum: Option<Entry>,
}

fn remove_recursive(
    transaction: &mut impl PageTransaction,
    root: PageNumber,
    page_number: PageNumber,
    target: &Entry,
    depth: u32,
) -> Result<Option<RemovedEntry>, BtreeError> {
    if depth >= MAX_INDEX_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match index_page_kind(&mut page, page_number)? {
        PageKind::IndexLeaf => {
            let mut entries = read_leaf_entries(&mut page, page_number)?;
            let mut remove_at = None;
            for (index, entry) in entries.iter().enumerate() {
                if entry.compare(target)? == Ordering::Equal {
                    remove_at = Some(index);
                    break;
                }
            }
            let Some(remove_at) = remove_at else {
                return Ok(None);
            };
            entries.remove(remove_at);
            let maximum = entries.last().cloned();
            let base_page = if page_number == root {
                page
            } else {
                Page::zeroed()
            };
            let packed = pack_leaf(page_number, &entries, base_page)?;
            transaction.write_page(page_number, &packed)?;
            Ok(Some(RemovedEntry { maximum }))
        }
        PageKind::IndexInterior => {
            let mut node = read_node(&mut page, page_number, transaction.page_count())?;
            let mut child_index = node.separators.len();
            for (index, separator) in node.separators.iter().enumerate() {
                if target.compare(separator)? != Ordering::Greater {
                    child_index = index;
                    break;
                }
            }
            let child = node.children[child_index];
            let Some(removed) = remove_recursive(transaction, root, child, target, depth + 1)?
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
                    SlottedPage::initialize(&mut empty_root, root, PageKind::IndexLeaf, None)?;
                    transaction.write_page(root, &empty_root)?;
                }
                return Ok(Some(RemovedEntry { maximum: None }));
            }

            let packed = pack_node(page_number, &node, page)?;
            transaction.write_page(page_number, &packed)?;
            let maximum = maximum_entry(transaction, page_number, depth)?;
            Ok(Some(RemovedEntry { maximum }))
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn maximum_entry(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    depth: u32,
) -> Result<Option<Entry>, BtreeError> {
    if depth >= MAX_INDEX_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    let mut page = transaction.read_page(page_number)?;
    match index_page_kind(&mut page, page_number)? {
        PageKind::IndexLeaf => Ok(read_leaf_entries(&mut page, page_number)?.last().cloned()),
        PageKind::IndexInterior => {
            let node = read_node(&mut page, page_number, transaction.page_count())?;
            let rightmost = *node.children.last().ok_or(BtreeError::CorruptPage(
                "index interior node has no right-most child",
            ))?;
            maximum_entry(transaction, rightmost, depth + 1)
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}

fn root_node(left: PageNumber, right: PageNumber, separator: Entry) -> Result<Node, BtreeError> {
    let mut children = Vec::new();
    children
        .try_reserve_exact(2)
        .map_err(|_| BtreeError::RecordAllocation)?;
    children.push(left);
    children.push(right);
    let mut separators = Vec::new();
    separators
        .try_reserve_exact(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    separators.push(separator);
    Ok(Node {
        children,
        separators,
    })
}

fn allocate_page(transaction: &mut impl PageTransaction) -> Result<PageNumber, BtreeError> {
    let expected = transaction
        .page_count()
        .checked_add(1)
        .ok_or(BtreeError::PageNumberExhausted)?;
    let allocated = transaction.allocate_page()?;
    if allocated.get() != expected {
        return Err(BtreeError::InvalidTree(
            "page transaction did not append the allocated index page",
        ));
    }
    Ok(allocated)
}

fn pack_leaf(
    page_number: PageNumber,
    entries: &[Entry],
    mut page: Page,
) -> Result<Page, BtreeError> {
    let mut slotted = SlottedPage::initialize(&mut page, page_number, PageKind::IndexLeaf, None)?;
    for (slot, entry) in entries.iter().enumerate() {
        slotted.insert_cell(slot, &entry.encoded)?;
    }
    Ok(page)
}

fn pack_node(page_number: PageNumber, node: &Node, mut page: Page) -> Result<Page, BtreeError> {
    if node.children.len() != node.separators.len() + 1 || node.children.is_empty() {
        return Err(BtreeError::CorruptPage(
            "index interior child and separator counts disagree",
        ));
    }
    let rightmost = *node.children.last().ok_or(BtreeError::CorruptPage(
        "index interior node has no right-most child",
    ))?;
    let mut slotted = SlottedPage::initialize(
        &mut page,
        page_number,
        PageKind::IndexInterior,
        Some(rightmost),
    )?;
    let mut cell = Vec::new();
    for (slot, (child, separator)) in node.children.iter().zip(&node.separators).enumerate() {
        cell.clear();
        cell.try_reserve(4 + separator.encoded.len())
            .map_err(|_| BtreeError::RecordAllocation)?;
        cell.extend_from_slice(&child.get().to_be_bytes());
        cell.extend_from_slice(&separator.encoded);
        slotted.insert_cell(slot, &cell)?;
    }
    Ok(page)
}

fn leaf_split_point(entries: &[Entry]) -> Result<usize, BtreeError> {
    let capacity = PAGE_SIZE - PageKind::IndexLeaf.header_size();
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(entries.len() + 1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    prefix.push(0_usize);
    for entry in entries {
        let cost = entry
            .encoded
            .len()
            .checked_add(2)
            .ok_or(BtreeError::RecordTooLarge)?;
        if cost > capacity {
            return Err(BtreeError::CellTooLarge);
        }
        prefix.push(
            prefix
                .last()
                .copied()
                .unwrap_or(0)
                .checked_add(cost)
                .ok_or(BtreeError::RecordTooLarge)?,
        );
    }
    choose_split(&prefix, entries.len(), capacity)
}

fn split_node(node: &Node) -> Result<(Node, Entry, Node), BtreeError> {
    let capacity = PAGE_SIZE - PageKind::IndexInterior.header_size();
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(node.separators.len() + 1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    prefix.push(0_usize);
    for entry in &node.separators {
        let cost = 4_usize
            .checked_add(entry.encoded.len())
            .and_then(|length| length.checked_add(2))
            .ok_or(BtreeError::RecordTooLarge)?;
        prefix.push(
            prefix
                .last()
                .copied()
                .unwrap_or(0)
                .checked_add(cost)
                .ok_or(BtreeError::RecordTooLarge)?,
        );
    }
    let boundary = choose_split(&prefix, node.children.len(), capacity)?;
    Ok((
        Node {
            children: copy_slice(&node.children[..boundary])?,
            separators: clone_slice(&node.separators[..boundary - 1])?,
        },
        node.separators[boundary - 1].clone(),
        Node {
            children: copy_slice(&node.children[boundary..])?,
            separators: clone_slice(&node.separators[boundary..])?,
        },
    ))
}

fn clone_slice<T: Clone>(slice: &[T]) -> Result<Vec<T>, BtreeError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(slice.len())
        .map_err(|_| BtreeError::RecordAllocation)?;
    copy.extend_from_slice(slice);
    Ok(copy)
}

fn choose_split(prefix: &[usize], item_count: usize, capacity: usize) -> Result<usize, BtreeError> {
    let total = *prefix.last().ok_or(BtreeError::CellTooLarge)?;
    let mut best = None;
    let mut best_difference = usize::MAX;
    for (boundary, left) in prefix.iter().copied().enumerate().take(item_count).skip(1) {
        let right = total - left;
        if left <= capacity && right <= capacity {
            let difference = left.abs_diff(right);
            if difference < best_difference {
                best = Some(boundary);
                best_difference = difference;
            }
        }
    }
    best.ok_or(BtreeError::CellTooLarge)
}

fn index_cell_ranges(slotted: &SlottedPage<'_>) -> Result<Vec<(usize, usize)>, BtreeError> {
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for slot in 0..slotted.cell_count() {
        offsets.push((slotted.cell_offset(slot)?, slot));
    }
    offsets.sort_unstable_by_key(|location| location.0);
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(slotted.cell_count())
        .map_err(|_| BtreeError::RecordAllocation)?;
    ranges.resize(slotted.cell_count(), (0, 0));
    for (index, (start, slot)) in offsets.iter().copied().enumerate() {
        let end = offsets
            .get(index + 1)
            .map(|location| location.0)
            .unwrap_or(PAGE_SIZE);
        if end <= start {
            return Err(BtreeError::CorruptPage("index cell ranges overlap"));
        }
        ranges[slot] = (start, end);
    }
    Ok(ranges)
}

fn find_recursive(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    key: &[u8],
    depth: u32,
    visited: &mut HashSet<u32>,
    matches: &mut Vec<i64>,
) -> Result<(), BtreeError> {
    if depth >= MAX_INDEX_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    visited
        .try_reserve(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    if !visited.insert(page_number.get()) {
        return Err(BtreeError::InvalidTree(
            "index references a page more than once",
        ));
    }
    let mut page = transaction.read_page(page_number)?;
    match index_page_kind(&mut page, page_number)? {
        PageKind::IndexLeaf => {
            for entry in read_leaf_entries(&mut page, page_number)? {
                match compare_records(&entry.record, key)? {
                    Ordering::Equal => {
                        matches
                            .try_reserve(1)
                            .map_err(|_| BtreeError::RecordAllocation)?;
                        matches.push(entry.rowid);
                    }
                    Ordering::Greater => break,
                    Ordering::Less => {}
                }
            }
        }
        PageKind::IndexInterior => {
            let node = read_node(&mut page, page_number, transaction.page_count())?;
            for (index, separator) in node.separators.iter().enumerate() {
                match compare_records(&separator.record, key)? {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        find_recursive(
                            transaction,
                            node.children[index],
                            key,
                            depth + 1,
                            visited,
                            matches,
                        )?;
                    }
                    Ordering::Greater => {
                        find_recursive(
                            transaction,
                            node.children[index],
                            key,
                            depth + 1,
                            visited,
                            matches,
                        )?;
                        return Ok(());
                    }
                }
            }
            if let Some(rightmost) = node.children.last().copied() {
                find_recursive(transaction, rightmost, key, depth + 1, visited, matches)?;
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

struct Validation {
    minimum: Option<Entry>,
    maximum: Option<Entry>,
    pages: u32,
    rows: u64,
    height: u32,
}

fn validate_subtree(
    transaction: &mut impl PageTransaction,
    page_number: PageNumber,
    page_count: u32,
    lower: Option<&Entry>,
    upper: Option<&Entry>,
    depth: u32,
    visited: &mut HashSet<u32>,
) -> Result<Validation, BtreeError> {
    if depth >= MAX_INDEX_DEPTH {
        return Err(BtreeError::TreeTooDeep);
    }
    if page_number.get() > page_count {
        return Err(BtreeError::InvalidTree(
            "index references a page outside the database",
        ));
    }
    visited
        .try_reserve(1)
        .map_err(|_| BtreeError::RecordAllocation)?;
    if !visited.insert(page_number.get()) {
        return Err(BtreeError::InvalidTree(
            "index references a page more than once",
        ));
    }
    let mut page = transaction.read_page(page_number)?;
    match index_page_kind(&mut page, page_number)? {
        PageKind::IndexLeaf => {
            let entries = read_leaf_entries(&mut page, page_number)?;
            for entry in &entries {
                if lower
                    .map(|bound| entry.compare(bound))
                    .transpose()?
                    .is_some_and(|ordering| ordering != Ordering::Greater)
                    || upper
                        .map(|bound| entry.compare(bound))
                        .transpose()?
                        .is_some_and(|ordering| ordering == Ordering::Greater)
                {
                    return Err(BtreeError::InvalidTree(
                        "index key is outside its parent separator range",
                    ));
                }
            }
            Ok(Validation {
                minimum: entries.first().cloned(),
                maximum: entries.last().cloned(),
                pages: 1,
                rows: u64::try_from(entries.len()).map_err(|_| BtreeError::RecordTooLarge)?,
                height: 1,
            })
        }
        PageKind::IndexInterior => {
            let node = read_node(&mut page, page_number, page_count)?;
            if node.children.len() != node.separators.len() + 1 {
                return Err(BtreeError::InvalidTree(
                    "index child and separator counts disagree",
                ));
            }
            let mut result = Validation {
                minimum: None,
                maximum: None,
                pages: 1,
                rows: 0,
                height: 0,
            };
            let mut child_height = None;
            for (index, child_page) in node.children.iter().copied().enumerate() {
                let child_lower = if index == 0 {
                    lower
                } else {
                    Some(&node.separators[index - 1])
                };
                let child_upper = node.separators.get(index).or(upper);
                let child = validate_subtree(
                    transaction,
                    child_page,
                    page_count,
                    child_lower,
                    child_upper,
                    depth + 1,
                    visited,
                )?;
                let Some(child_maximum) = child.maximum.as_ref() else {
                    return Err(BtreeError::InvalidTree(
                        "index interior page references an empty child",
                    ));
                };
                if child_height.is_some_and(|height| height != child.height) {
                    return Err(BtreeError::InvalidTree(
                        "index leaves have different depths",
                    ));
                }
                child_height = Some(child.height);
                if index == 0 {
                    result.minimum = child.minimum.clone();
                }
                if index < node.separators.len()
                    && child_maximum.compare(&node.separators[index])? != Ordering::Equal
                {
                    return Err(BtreeError::InvalidTree(
                        "index separator does not match its left child's maximum key",
                    ));
                }
                result.maximum = child.maximum;
                result.rows = result
                    .rows
                    .checked_add(child.rows)
                    .ok_or(BtreeError::RecordTooLarge)?;
                result.pages = result
                    .pages
                    .checked_add(child.pages)
                    .ok_or(BtreeError::RecordTooLarge)?;
            }
            result.height = child_height
                .and_then(|height| height.checked_add(1))
                .ok_or(BtreeError::InvalidTree(
                    "index interior page has no valid children",
                ))?;
            Ok(result)
        }
        _ => Err(BtreeError::WrongTreePage {
            page_number: page_number.get(),
        }),
    }
}
