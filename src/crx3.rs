use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Debug)]
#[allow(dead_code)]
/// A CRX₃ file is a binary file of the following format:
/// [4 octets]: "Cr24", a magic number.
/// [4 octets]: The version of the *.crx file format used (currently 3).
/// [4 octets]: N, little-endian, the length of the header section.
/// [N octets]: The header (the binary encoding of a CrxFileHeader).
/// [M octets]: The ZIP archive.
///
/// See https://chromium.googlesource.com/chromium/src/+/HEAD/components/crx_file/crx3.proto
pub struct CrxFile {
    /// Crx magic string
    pub magic: [u8; 4],
    /// The version of the *.crx file format used (currently 3).
    pub version: u32,
    /// Length of the header section
    pub length: u32,
    /// CrxFileHeader (unparsed)
    pub header: Vec<u8>,
    pub zip_archive: Vec<u8>,
}

pub async fn parse_file<P: AsRef<Path>>(path: P) -> Result<CrxFile> {
    let mut file = File::open(path).await?;

    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).await?;

    // Check if the file signature matches "Cr24"
    if &magic != b"Cr24" {
        return Err(anyhow!("Invalid CRX file signature"));
    }

    let version = read_u32(&mut file).await?;
    let length = read_u32(&mut file).await?;

    let mut header = vec![0u8; length as usize];
    file.read_exact(&mut header).await?;

    // Read the rest of the file as ZIP archive
    let mut zip_archive = Vec::new();
    file.read_to_end(&mut zip_archive).await?;

    Ok(CrxFile {
        magic,
        version,
        length,
        header,
        zip_archive,
    })
}

#[inline(always)]
async fn read_u32(file: &mut File) -> Result<u32> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).await?;
    Ok(u32::from_le_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;
    use tokio::{fs, io::AsyncWriteExt};

    #[tokio::test]
    async fn test_parse_file() {
        let path = "tests/fixtures/dbepggeogbaibhgnhhndojpepiihcmeb.crx";
        let crx_file = parse_file(path).await.unwrap();
        assert_eq!(crx_file.magic, *b"Cr24");
        assert_eq!(crx_file.version, 3);
        assert_eq!(crx_file.length, 1049);
        assert_eq!(crx_file.zip_archive.len(), 277495);

        let metadata = fs::metadata(path).await.unwrap();
        assert_eq!(
            metadata.size() as u32,
            12 + crx_file.length + crx_file.zip_archive.len() as u32,
        )
    }

    #[tokio::test]
    async fn test_parse_file_invalid() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("invalid.crx3");
        {
            let mut file = File::create(&path).await.unwrap();
            file.write_all(b"hello world").await.unwrap();
        }
        assert!(parse_file(&path).await.is_err());
    }
}
