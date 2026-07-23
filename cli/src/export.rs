use std::fmt;
use std::io;

use xlm_ns_sdk::types::RegistryEntry as NameRecord;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PortfolioRecord {
    pub name: String,
    pub owner: String,
    pub resolver: String,
    pub target_address: String,
    pub registered_at: i64,
    pub expires_at: i64,
    pub grace_period_ends_at: i64,
    pub status: String,
}

fn record_status(record: &NameRecord, now_unix: i64) -> String {
    let expires_at = record.expires_at as i64;
    let grace_period_ends_at = record.grace_period_ends_at as i64;

    if now_unix <= expires_at {
        "active".to_string()
    } else if now_unix <= grace_period_ends_at {
        "grace".to_string()
    } else {
        "expired".to_string()
    }
}

impl PortfolioRecord {
    pub fn from_name_record(record: &NameRecord, now_unix: i64) -> Self {
        Self {
            name: record.name.to_string(),
            owner: record.owner.to_string(),
            resolver: record.resolver.clone().unwrap_or_default(),
            target_address: record.target_address.clone().unwrap_or_default(),
            registered_at: record.registered_at as i64,
            expires_at: record.expires_at as i64,
            grace_period_ends_at: record.grace_period_ends_at as i64,
            status: record_status(record, now_unix),
        }
    }
}

pub fn write_json<T: serde::Serialize>(
    records: &[T],
    writer: &mut impl io::Write,
) -> Result<(), ExportError> {
    serde_json::to_writer_pretty(writer, records)?;
    Ok(())
}

pub fn write_csv<T: serde::Serialize>(
    records: &[T],
    writer: &mut impl io::Write,
) -> Result<(), ExportError> {
    let mut wtr = csv::Writer::from_writer(writer);
    for record in records {
        wtr.serialize(record)?;
    }
    wtr.flush()?;
    Ok(())
}

#[derive(Debug)]
pub enum ExportError {
    Json(serde_json::Error),
    Csv(csv::Error),
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "json export failed: {err}"),
            Self::Csv(err) => write!(f, "csv export failed: {err}"),
            Self::Io(err) => write!(f, "export I/O failed: {err}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<serde_json::Error> for ExportError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<csv::Error> for ExportError {
    fn from(err: csv::Error) -> Self {
        Self::Csv(err)
    }
}

impl From<io::Error> for ExportError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_record(expires_at: u64, grace_period_ends_at: u64) -> NameRecord {
        NameRecord {
            name: "timmy.xlm".to_string(),
            owner: "GDRA...OWNER".to_string(),
            resolver: Some("CABC...RESOLVER".to_string()),
            target_address: Some("GDRA...TARGET".to_string()),
            metadata_uri: None,
            ttl_seconds: 3600,
            registered_at: 1_700_000_000,
            expires_at,
            grace_period_ends_at,
            transfer_count: 0,
        }
    }

    #[test]
    fn test_portfolio_record_status_active() {
        let record = name_record(1_700_001_000, 1_700_002_000);
        let export = PortfolioRecord::from_name_record(&record, 1_700_000_000);

        assert_eq!(export.status, "active");
    }

    #[test]
    fn test_portfolio_record_status_grace() {
        let record = name_record(1_700_000_000, 1_700_002_000);
        let export = PortfolioRecord::from_name_record(&record, 1_700_001_000);

        assert_eq!(export.status, "grace");
    }

    #[test]
    fn test_portfolio_record_status_expired() {
        let record = name_record(1_700_000_000, 1_700_001_000);
        let export = PortfolioRecord::from_name_record(&record, 1_700_002_000);

        assert_eq!(export.status, "expired");
    }

    #[test]
    fn test_write_json_output_is_valid() {
        let record = name_record(1_700_001_000, 1_700_002_000);
        let records = vec![PortfolioRecord::from_name_record(&record, 1_700_000_000)];
        let mut buf = Vec::new();

        write_json(&records, &mut buf).unwrap();
        let parsed = serde_json::from_slice::<Vec<serde_json::Value>>(&buf).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["name"], "timmy.xlm");
        assert!(parsed[0]["status"].is_string());
    }

    #[test]
    fn test_write_csv_output_has_header_and_row() {
        let record = name_record(1_700_001_000, 1_700_002_000);
        let records = vec![PortfolioRecord::from_name_record(&record, 1_700_000_000)];
        let mut buf = Vec::new();

        write_csv(&records, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines = output.trim_end().lines().collect::<Vec<_>>();

        assert_eq!(
            lines[0],
            "name,owner,resolver,target_address,registered_at,expires_at,grace_period_ends_at,status"
        );
        assert!(lines[1].starts_with("timmy.xlm,"));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_write_csv_multiple_records() {
        let first = name_record(1_700_001_000, 1_700_002_000);
        let second = name_record(1_700_003_000, 1_700_004_000);
        let third = name_record(1_700_005_000, 1_700_006_000);
        let records = vec![
            PortfolioRecord::from_name_record(&first, 1_700_000_000),
            PortfolioRecord::from_name_record(&second, 1_700_000_000),
            PortfolioRecord::from_name_record(&third, 1_700_000_000),
        ];
        let mut buf = Vec::new();

        write_csv(&records, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines = output.trim_end().lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 4);
    }
}
