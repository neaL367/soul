//! Mark of the Web (MOTW) NTFS Alternate Data Stream security tagging for Windows downloads.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Attaches Windows Zone Identifier (Zone 3 = Internet) to downloaded files via NTFS ADS.
///
/// Format:
/// ```ini
/// [ZoneTransfer]
/// ZoneId=3
/// HostUrl=https://example.com/file.zip
/// ```
///
/// # Errors
///
/// Returns `std::io::Error` if NTFS Alternate Data Stream writing fails.
pub fn attach_zone_identifier(file_path: &Path, source_url: &str) -> std::io::Result<()> {
    let mut ads_os = file_path.as_os_str().to_os_string();
    ads_os.push(":Zone.Identifier");
    let ads_path = Path::new(&ads_os);

    let content = format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={source_url}\r\n");

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(ads_path)?;

    file.write_all(content.as_bytes())?;
    file.flush()?;
    Ok(())
}
