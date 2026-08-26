use std::{fs::File, io, io::Write, path::Path};

use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Copy)]
pub enum FixtureCompression {
    Stored,
    Deflated,
}

impl FixtureCompression {
    const fn method(self) -> CompressionMethod {
        match self {
            Self::Stored => CompressionMethod::Stored,
            Self::Deflated => CompressionMethod::Deflated,
        }
    }
}

pub fn write_zip_package(
    path: &Path,
    manifest: &[u8],
    payloads: &[(&str, &[u8])],
    compression: FixtureCompression,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(compression.method())
        .unix_permissions(0o644);
    writer.start_file("mod.json", options)?;
    writer.write_all(manifest)?;
    for (name, data) in payloads {
        writer.start_file(name, options)?;
        writer.write_all(data)?;
    }
    writer.finish()?.sync_all()?;
    Ok(())
}

pub struct RawZIPEntry<'a> {
    name: &'a [u8],
    data: &'a [u8],
    flags: u16,
    compression: u16,
    unix_mode: u32,
    declared_compressed_bytes: Option<u32>,
    declared_uncompressed_bytes: Option<u32>,
}

impl<'a> RawZIPEntry<'a> {
    pub const fn file(name: &'a [u8], data: &'a [u8]) -> Self {
        Self {
            name,
            data,
            flags: 0,
            compression: 0,
            unix_mode: 0o100_644,
            declared_compressed_bytes: None,
            declared_uncompressed_bytes: None,
        }
    }

    pub const fn with_flags(mut self, flags: u16) -> Self {
        self.flags = flags;
        self
    }

    pub const fn with_compression(mut self, compression: u16) -> Self {
        self.compression = compression;
        self
    }

    pub const fn with_unix_mode(mut self, unix_mode: u32) -> Self {
        self.unix_mode = unix_mode;
        self
    }

    pub const fn with_declared_uncompressed_bytes(mut self, bytes: u32) -> Self {
        self.declared_uncompressed_bytes = Some(bytes);
        self
    }
}

pub fn write_raw_zip(path: &Path, entries: &[RawZIPEntry<'_>]) -> io::Result<()> {
    let mut bytes = Vec::new();
    let mut central_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let offset = u32_from_usize(bytes.len())?;
        let data_bytes = u32_from_usize(entry.data.len())?;
        let compressed = entry.declared_compressed_bytes.unwrap_or(data_bytes);
        let uncompressed = entry.declared_uncompressed_bytes.unwrap_or(data_bytes);
        let crc = crc32(entry.data);
        push_u32(&mut bytes, 0x0403_4b50);
        push_u16(&mut bytes, 20);
        push_u16(&mut bytes, entry.flags);
        push_u16(&mut bytes, entry.compression);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u32(&mut bytes, crc);
        push_u32(&mut bytes, compressed);
        push_u32(&mut bytes, uncompressed);
        push_u16(&mut bytes, u16_from_usize(entry.name.len())?);
        push_u16(&mut bytes, 0);
        bytes.extend_from_slice(entry.name);
        bytes.extend_from_slice(entry.data);
        central_entries.push((entry, offset, crc, compressed, uncompressed));
    }

    let central_offset = u32_from_usize(bytes.len())?;
    for (entry, offset, crc, compressed, uncompressed) in &central_entries {
        push_u32(&mut bytes, 0x0201_4b50);
        push_u16(&mut bytes, 0x0314);
        push_u16(&mut bytes, 20);
        push_u16(&mut bytes, entry.flags);
        push_u16(&mut bytes, entry.compression);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u32(&mut bytes, *crc);
        push_u32(&mut bytes, *compressed);
        push_u32(&mut bytes, *uncompressed);
        push_u16(&mut bytes, u16_from_usize(entry.name.len())?);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u32(&mut bytes, entry.unix_mode << 16);
        push_u32(&mut bytes, *offset);
        bytes.extend_from_slice(entry.name);
    }

    let central_bytes = u32_from_usize(bytes.len())?
        .checked_sub(central_offset)
        .ok_or_else(|| io::Error::other("central directory offset exceeded output"))?;
    let entry_count = u16_from_usize(entries.len())?;
    push_u32(&mut bytes, 0x0605_4b50);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, entry_count);
    push_u16(&mut bytes, entry_count);
    push_u32(&mut bytes, central_bytes);
    push_u32(&mut bytes, central_offset);
    push_u16(&mut bytes, 0);
    std::fs::write(path, bytes)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn u16_from_usize(value: usize) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| io::Error::other("ZIP fixture field exceeded u16"))
}

fn u32_from_usize(value: usize) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| io::Error::other("ZIP fixture field exceeded u32"))
}
