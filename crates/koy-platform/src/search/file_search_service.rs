//! Windows Arama dizininde dosya adı arar. Yeni bağımlılık gerektirmesin diye dizine Windows'un kendi ADO COM
//! nesnesiyle (geç bağlamalı) bağlanılır. Dizin kapalıysa boş sonuç döner.

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::search::{IFileSearchService, IndexedItem, ServiceError};
use koy_core::text;

use super::com_apartment::ComApartment;
use crate::com::{Arg, Dispatch};

const CONNECTION_STRING: &str = "Provider=Search.CollatorDSO;Extended Properties='Application=Windows';";
const DIRECTORY_ITEM_TYPE: &str = "Directory";

#[derive(Default)]
pub struct FileSearchService;

impl FileSearchService {
    /// Kullanıcının yazdığı metni dizin sözdiziminden arındırır; SQL'e yalnız harf, rakam ve güvenli işaretler girer.
    pub fn build_sql(term: &str, limit: i32) -> String {
        let words: Vec<String> = term.split(' ').filter(|w| !w.is_empty()).map(sanitize).filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            return String::new();
        }
        let conditions = words.iter().map(|w| format!("CONTAINS(System.FileName, '\"{w}*\"')")).collect::<Vec<_>>().join(" AND ");
        format!(
            "SELECT TOP {limit} System.ItemPathDisplay, System.ItemType FROM SYSTEMINDEX WHERE SCOPE='file:' AND {conditions} ORDER BY System.Search.Rank DESC"
        )
    }

    fn query(term: &str, limit: i32, ct: &CancellationToken) -> Result<Vec<IndexedItem>, ServiceError> {
        let sql = Self::build_sql(term, limit);
        if sql.is_empty() {
            return Ok(Vec::new());
        }
        let _mta = ComApartment::mta();
        match run_query(&sql, ct) {
            Ok(items) => Ok(items),
            Err(QueryError::Canceled) => Err(Box::new(OperationCanceled)),
            Err(QueryError::Com(e)) => {
                // Dizin servisi kapalı ya da sorgu reddedildi: arama sonuçsuz kalır, diğer kaynaklar çalışır.
                CrashLog::write("Dosya dizini", &CrashError::exception("System.Runtime.InteropServices.COMException", &e.message(), ""));
                Ok(Vec::new())
            }
        }
    }
}

/// C# `char.IsLetterOrDigit(c) || c is '-' or '_' or '.'`.
fn sanitize(word: &str) -> String {
    word.chars().filter(|&c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.')).collect()
}

enum QueryError {
    Canceled,
    Com(windows::core::Error),
}

impl From<windows::core::Error> for QueryError {
    fn from(e: windows::core::Error) -> Self {
        QueryError::Com(e)
    }
}

fn run_query(sql: &str, ct: &CancellationToken) -> Result<Vec<IndexedItem>, QueryError> {
    let Some(connection) = Dispatch::create("ADODB.Connection")? else { return Ok(Vec::new()) };
    connection.call("Open", &[Arg::Str(CONNECTION_STRING)])?;
    let Some(records) = connection.call("Execute", &[Arg::Str(sql)])?.as_dispatch() else { return Ok(Vec::new()) };
    let Some(fields) = records.get("Fields")?.as_dispatch() else { return Ok(Vec::new()) };

    let mut items = Vec::new();
    while records.get("EOF")?.as_bool() == Some(false) {
        if ct.is_cancellation_requested() {
            return Err(QueryError::Canceled);
        }
        if let Some(path) = field_value(&fields, 0)?.filter(|p| !p.is_empty()) {
            let kind = field_value(&fields, 1)?;
            items.push(IndexedItem::new(path, kind.is_some_and(|k| text::eq_ignore_case(&k, DIRECTORY_ITEM_TYPE))));
        }
        records.call("MoveNext", &[])?;
    }
    Ok(items)
}

fn field_value(fields: &Dispatch, index: i32) -> windows::core::Result<Option<String>> {
    let Some(field) = fields.get_indexed("Item", &[Arg::Int(index)])?.as_dispatch() else { return Ok(None) };
    Ok(field.get("Value")?.as_string())
}

impl IFileSearchService for FileSearchService {
    fn search(&self, term: &str, limit: i32, ct: &CancellationToken) -> Result<Vec<IndexedItem>, ServiceError> {
        // C#: Task.Run; burada çağıranın işçi iş parçacığında çalışır (COM için kendi MTA'sını kurar).
        Self::query(term, limit, ct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_sanitized_sql() {
        assert_eq!(FileSearchService::build_sql("  ", 5), "");
        assert_eq!(
            FileSearchService::build_sql("rapor' OR 1=1 2024", 10),
            "SELECT TOP 10 System.ItemPathDisplay, System.ItemType FROM SYSTEMINDEX WHERE SCOPE='file:' AND CONTAINS(System.FileName, '\"rapor*\"') AND CONTAINS(System.FileName, '\"OR*\"') AND CONTAINS(System.FileName, '\"11*\"') AND CONTAINS(System.FileName, '\"2024*\"') ORDER BY System.Search.Rank DESC"
        );
    }

    #[test]
    fn queries_windows_search_index_without_failing() {
        // Dizin kapalı olsa da hata değil boş sonuç dönmeli.
        let result = FileSearchService.search("windows", 3, &CancellationToken::none());
        assert!(result.is_ok());
    }
}
