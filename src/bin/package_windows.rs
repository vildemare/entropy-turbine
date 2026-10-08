//! Zip a Windows build with the assets Bevy loads beside the executable.
//!
//! ```text
//! cargo run --bin package_windows --features package -- path/to/entropy-turbine.exe
//! ```

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const PACKAGE: &str = "Entropy-Turbine";

fn main() -> ExitCode {
    match package() {
        Ok(output) => {
            println!("Created {}", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn package() -> io::Result<PathBuf> {
    let mut args = env::args().skip(1);
    let Some(executable) = args.next() else {
        return Err(io::Error::other(
            "usage: package_windows path/to/entropy-turbine.exe [output.zip]",
        ));
    };
    let output_arg = args.next();
    if args.next().is_some() {
        return Err(io::Error::other(
            "usage: package_windows path/to/entropy-turbine.exe [output.zip]",
        ));
    }

    let executable = PathBuf::from(executable);
    let is_exe = executable
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !executable.is_file() || !is_exe {
        return Err(io::Error::other(format!(
            "Windows executable not found: {}",
            executable.display()
        )));
    }
    if !is_windows_pe(&executable)? {
        return Err(io::Error::other(format!(
            "Not a Windows PE executable: {}",
            executable.display()
        )));
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets = root.join("assets");
    if !assets.is_dir() {
        return Err(io::Error::other(format!(
            "Assets directory not found: {}",
            assets.display()
        )));
    }
    let output = output_arg
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("dist/entropy-turbine-windows.zip"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(&output)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6));

    archive.start_file(format!("{PACKAGE}/entropy-turbine.exe"), options)?;
    io::copy(&mut File::open(&executable)?, &mut archive)?;

    let mut asset_files = files_under(&assets)?;
    asset_files.sort();
    for path in asset_files {
        let relative = path.strip_prefix(&assets).unwrap_or(&path);
        let name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        archive.start_file(format!("{PACKAGE}/assets/{name}"), options)?;
        io::copy(&mut File::open(&path)?, &mut archive)?;
    }

    archive.start_file(format!("{PACKAGE}/READ ME.txt"), options)?;
    archive.write_all(
        b"Extract the whole folder, then double-click entropy-turbine.exe.\n\
Keep the assets folder beside the executable.\n",
    )?;
    archive.finish()?;
    Ok(output)
}

fn is_windows_pe(path: &Path) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut magic = [0; 2];
    if file.read(&mut magic)? < 2 || &magic != b"MZ" {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(0x3C))?;
    let mut offset = [0; 4];
    file.read_exact(&mut offset)?;
    file.seek(SeekFrom::Start(u32::from_le_bytes(offset) as u64))?;
    let mut signature = [0; 4];
    match file.read_exact(&mut signature) {
        Ok(()) => Ok(&signature == b"PE\0\0"),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

fn files_under(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(current)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_file_is_not_a_windows_executable() {
        let path = std::env::temp_dir().join(format!("entropy-pe-{}", std::process::id()));
        fs::write(&path, b"not a pe file").unwrap();
        assert!(!is_windows_pe(&path).unwrap());
        fs::remove_file(path).unwrap();
    }
}
