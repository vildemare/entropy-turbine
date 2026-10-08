use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

pub fn write_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("wav.partial");
    let mut file = File::create(&temporary)?;
    write_body(&mut file, sample_rate, samples)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn write_body(file: &mut File, sample_rate: u32, samples: &[f32]) -> io::Result<()> {
    let data_bytes = samples.len() as u32 * 2;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    file.write_all(&header)?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        file.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}
