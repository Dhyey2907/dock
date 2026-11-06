//! File search through the Windows Search index (the `SystemIndex` catalog), the same index
//! Explorer and Start search use. `ISearchQueryHelper` turns what was typed into the index's SQL;
//! OLE DB runs it. Nothing is crawled here, so results come back in a moment.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileHit {
    pub name: String,
    pub path: String,
    pub folder: bool,
}

/// What's typed, made safe to hand to the query helper: one line, no control characters,
/// a sensible length. Empty means "don't search".
pub fn clean_query(query: &str) -> String {
    query
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(windows)]
pub use self::win::search;

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    use windows::{
        core::{Interface, GUID, HSTRING, PWSTR},
        Win32::System::{
            Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER},
            Search::{
                CSearchManager, IAccessor, ICommandText, IDBCreateCommand, IDBCreateSession, IDBInitialize,
                IDataInitialize, IRowset, ISearchManager, DBACCESSOR_ROWDATA, DBBINDING,
                DBMEMOWNER_CLIENTOWNED, DBPART_LENGTH, DBPART_STATUS, DBPART_VALUE, DBSTATUS_S_OK,
                DBTYPE_WSTR, HACCESSOR, MSDAINITIALIZE,
            },
        },
    };

    use super::{clean_query, FileHit};
    use crate::error::{AppError, AppResult};

    /// The default SQL dialect (`DBGUID_DEFAULT`).
    const DBGUID_DEFAULT: GUID = GUID::from_u128(0xc8b521fb_5cf3_11ce_ade5_00aa0044773d);
    const COLUMNS: usize = 3;
    /// Characters kept per column (Windows paths are at most 260 in the index's display form,
    /// but long paths exist).
    const CHARS: usize = 1024;
    const BATCH: usize = 32;

    #[repr(C)]
    struct Cell {
        status: u32,
        length: usize,
        value: [u16; CHARS],
    }

    #[repr(C)]
    struct Row {
        cells: [Cell; COLUMNS],
    }

    fn take_string(text: PWSTR) -> String {
        if text.is_null() {
            return String::new();
        }
        // SAFETY: the helper returns a NUL-terminated string it allocated with CoTaskMemAlloc;
        // it is read once and freed.
        unsafe {
            let out = text.to_string().unwrap_or_default();
            CoTaskMemFree(Some(text.0 as *const c_void));
            out
        }
    }

    fn cell_text(cell: &Cell) -> Option<String> {
        if cell.status != DBSTATUS_S_OK.0 as u32 {
            return None;
        }
        let chars = (cell.length / 2).min(CHARS);
        Some(String::from_utf16_lossy(&cell.value[..chars]))
    }

    /// Up to `max` files and folders matching `query`, best first.
    pub fn search(query: &str, max: usize) -> AppResult<Vec<FileHit>> {
        let query = clean_query(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        crate::apps::win::com_init();
        // SAFETY: COM calls on objects owned by this function. The row buffer is laid out to
        // match the bindings given to CreateAccessor; rows and the accessor are released before
        // their objects go.
        unsafe {
            let manager: ISearchManager = CoCreateInstance(&CSearchManager, None, CLSCTX_LOCAL_SERVER)?;
            let catalog = manager.GetCatalog(&HSTRING::from("SystemIndex"))?;
            let helper = catalog.GetQueryHelper()?;
            helper.SetQuerySelectColumns(&HSTRING::from(
                "System.ItemNameDisplay,System.ItemPathDisplay,System.ItemType",
            ))?;
            helper.SetQueryMaxResults(max as i32)?;
            helper.SetQueryWhereRestrictions(&HSTRING::from("AND SCOPE='file:'"))?;
            helper.SetQuerySorting(&HSTRING::from("System.Search.Rank DESC"))?;
            let sql = take_string(helper.GenerateSQLFromUserQuery(&HSTRING::from(query))?);
            let connection = take_string(helper.ConnectionString()?);

            let init: IDataInitialize = CoCreateInstance(&MSDAINITIALIZE, None, CLSCTX_INPROC_SERVER)?;
            let mut source = None;
            init.GetDataSource(
                None,
                CLSCTX_INPROC_SERVER.0,
                &HSTRING::from(connection),
                &IDBInitialize::IID,
                &mut source,
            )?;
            let db: IDBInitialize = source
                .ok_or_else(|| AppError::Platform("search index unavailable".into()))?
                .cast()?;
            db.Initialize()?;
            let session = db
                .cast::<IDBCreateSession>()?
                .CreateSession(None, &IDBCreateCommand::IID)?;
            let command = session
                .cast::<IDBCreateCommand>()?
                .CreateCommand(None, &ICommandText::IID)?;
            let text: ICommandText = command.cast()?;
            text.SetCommandText(&DBGUID_DEFAULT, &HSTRING::from(sql))?;
            let mut rowset = None;
            text.Execute(None, &IRowset::IID, None, None, Some(&mut rowset))?;
            let rowset: IRowset = rowset
                .ok_or_else(|| AppError::Platform("search returned nothing".into()))?
                .cast()?;
            let accessor: IAccessor = rowset.cast()?;

            let cell_size = std::mem::size_of::<Cell>();
            let bindings: Vec<DBBINDING> = (0..COLUMNS)
                .map(|i| DBBINDING {
                    iOrdinal: i + 1,
                    obValue: i * cell_size + std::mem::offset_of!(Cell, value),
                    obLength: i * cell_size + std::mem::offset_of!(Cell, length),
                    obStatus: i * cell_size + std::mem::offset_of!(Cell, status),
                    dwPart: (DBPART_VALUE.0 | DBPART_LENGTH.0 | DBPART_STATUS.0) as u32,
                    dwMemOwner: DBMEMOWNER_CLIENTOWNED.0 as u32,
                    cbMaxLen: CHARS * 2,
                    wType: DBTYPE_WSTR.0 as u16,
                    ..Default::default()
                })
                .collect();
            let mut handle = HACCESSOR::default();
            accessor.CreateAccessor(
                DBACCESSOR_ROWDATA.0 as u32,
                bindings.len(),
                bindings.as_ptr(),
                std::mem::size_of::<Row>(),
                &mut handle,
                None,
            )?;

            let mut hits = Vec::new();
            let mut row: Box<Row> = Box::new(std::mem::zeroed());
            let mut handles = [0usize; BATCH];
            'rows: while hits.len() < max {
                let mut obtained = 0usize;
                let mut list = handles.as_mut_ptr();
                // Called through the table: the generated wrapper passes the handle array the
                // wrong way round for a caller-provided buffer.
                let hr = (Interface::vtable(&rowset).GetNextRows)(
                    rowset.as_raw(),
                    0,
                    0,
                    BATCH as isize,
                    &mut obtained,
                    &mut list,
                );
                if hr.is_err() || obtained == 0 {
                    break;
                }
                for &hrow in &handles[..obtained] {
                    if rowset
                        .GetData(hrow, handle, (&mut *row as *mut Row).cast())
                        .is_err()
                    {
                        continue;
                    }
                    let [name, path, kind] = &row.cells;
                    if let (Some(name), Some(path)) = (cell_text(name), cell_text(path)) {
                        let folder = cell_text(kind).is_some_and(|k| k.eq_ignore_ascii_case("directory"));
                        hits.push(FileHit { name, path, folder });
                    }
                }
                let _ = rowset.ReleaseRows(
                    obtained,
                    handles.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                if obtained < BATCH {
                    break 'rows;
                }
            }
            let _ = accessor.ReleaseAccessor(handle, None);
            hits.truncate(max);
            Ok(hits)
        }
    }

    #[cfg(test)]
    mod tests {
        /// Runs a real query on this PC's index: `cargo test search_live -- --ignored`.
        #[test]
        #[ignore]
        fn search_live() {
            let hits = super::search("windows", 5).unwrap();
            println!("{hits:#?}");
            assert!(!hits.is_empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::clean_query;

    #[test]
    fn queries_are_one_trimmed_line() {
        assert_eq!(clean_query("  budget\r\n2026 \t"), "budget  2026");
        assert_eq!(clean_query("   "), "");
        assert_eq!(clean_query(&"x".repeat(500)).len(), 200);
    }
}
