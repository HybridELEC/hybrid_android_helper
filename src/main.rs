use std::{fs::{read_dir, DirEntry, File, ReadDir}, io::Read, os::unix::ffi::OsStrExt, path::Path};

use clap::{command, Parser};

#[derive(Debug)]
enum Error {
    IOError (String),
    InvalidSystem,
    IllegalID,
}

type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(format!("{value}"))
    }
}

type FATFS = fatfs::FileSystem<File>;
type FATDir<'a> = fatfs::Dir<'a, File>;
type FATEntry<'a> = fatfs::DirEntry<'a, File>;
type FATFile<'a> = fatfs::File<'a, File>;

fn file_open_checked<P: AsRef<Path>>(path: P) -> Result<File> {
    File::open(&path).map_err(|e|{
        eprintln!("Failed to open file '{}': {}", path.as_ref().display(), e);
        e.into()
    })
}

fn fatfs_open<P: AsRef<Path>>(path: P) -> Result<FATFS> {
    fatfs::FileSystem::new(
        file_open_checked(&path)?, fatfs::FsOptions::new()
    ).map_err(|e|{
        eprintln!("Failed to open FAT filesystem '{}': {}", 
                    path.as_ref().display(), e);
        e.into()
    })
}

fn read_dir_checked<P: AsRef<Path>>(path: P) -> Result<ReadDir> {
    read_dir(&path).map_err(|e|{
        eprintln!("Failed to read dir '{}': {}", path.as_ref().display(), e);
        e.into()
    })
}

#[derive(Clone, Copy)]
enum SubSystem {
    OfficialCoreELEC,
    OfficialEmuELEC,
    HybridCoreELEC,
    HybridEmuELEC,
}

impl SubSystem {
    fn as_str(&self) -> &'static str {
        match self {
            SubSystem::OfficialCoreELEC => "Official CoreELEC",
            SubSystem::OfficialEmuELEC => "Official EmuELEC",
            SubSystem::HybridCoreELEC => "Hybrid CoreELEC",
            SubSystem::HybridEmuELEC => "Hybrid EmuELEC",
        }
    }

    fn iterator() -> std::slice::Iter<'static, Self> {
        const SUBSYSTEMS: [SubSystem; 4] = [
            SubSystem::OfficialCoreELEC, 
            SubSystem::OfficialEmuELEC,
            SubSystem::HybridCoreELEC,
            SubSystem::HybridEmuELEC];
        SUBSYSTEMS.iter()
    }

    fn cfgload_flag(&self) -> &[u8] {
        match self {
            SubSystem::OfficialCoreELEC => b"CoreELEC on eMMC",
            SubSystem::OfficialEmuELEC => b"EmuELEC on eMMC",
            SubSystem::HybridCoreELEC => b"HybridELEC (CE) on eMMC",
            SubSystem::HybridEmuELEC => b"HybridELEC (EE) on eMMC",
        }
    }
}

fn check_buffer_cfgload_system(buffer: &[u8]) -> Option<SubSystem> {
    for subsystem in SubSystem::iterator() {
        let cfgload_flag = subsystem.cfgload_flag();
        if buffer.windows(cfgload_flag.len()).position(
            |window|window == cfgload_flag).is_some() 
        {
            return Some(*subsystem)
        }
    }
    None
}

fn check_fat_file_cfgload_system(fatfile: &mut FATFile) 
    -> Result<Option<SubSystem>> 
{
    let mut buffer = Vec::new();
    fatfile.read_to_end(&mut buffer)?;
    Ok(check_buffer_cfgload_system(&buffer))
}

fn check_fat_entry_cfgload_system(cfgload: &FATEntry) 
    -> Result<Option<SubSystem>> 
{
    check_fat_file_cfgload_system(&mut cfgload.to_file())
}

fn check_fat_dir_system(dir: &FATDir) -> Result<Option<SubSystem>> {
    // let mut cfgload = false;
    let mut config_ini = false;
    let mut device_trees = false;
    let mut kernel_img = false;
    let mut system = false;
    let mut subsystem = None;
    let mut cfgload = None;
    for entry in dir.iter() {
        let entry = entry.map_err(|e|{
            eprintln!("Failed to read FAT dir entry: {}", e);
            Error::from(e)
        })?;
        match entry.file_name().as_str() {
            "cfgload" => {
                cfgload = if entry.is_file() {
                    Some(entry)
                } else {
                    None
                };
            },
            "config.ini" => config_ini = entry.is_file(),
            "device_trees" => device_trees = entry.is_dir(),
            "kernel.img" => kernel_img = entry.is_file(),
            "SYSTEM" => system = entry.is_file(),
            _ => ()
        }
    }
    if config_ini && device_trees && kernel_img && system {
        if let Some(cfgload) = cfgload {
            subsystem = check_fat_entry_cfgload_system(&cfgload)?
        }
    }
    Ok (subsystem)
    
}

fn check_fat_fs_system(fatfs: &FATFS) -> Result<Option<SubSystem>> {
    check_fat_dir_system(&fatfs.root_dir())
}

fn check_file_system(file: File) -> Result<Option<SubSystem>> {
    check_fat_fs_system(
        &FATFS::new(file, fatfs::FsOptions::new())?)
}

fn check_path_system<P: AsRef<Path>>(path: P
) -> Result<Option<SubSystem>> 
{
    check_file_system(file_open_checked(path)?)
}

fn check_dir_entry_system(entry: DirEntry) -> Result<Option<SubSystem>> {
    check_path_system(&entry.path())
}

fn id_from_bytes(bytes: &[u8]) -> Option<usize> {
    let mut multiply = 1;
    let mut id = 0;
    for digit in bytes.iter().rev() {
        id += match *digit {
            b'0' => 0,
            b'1' => 1,
            b'2' => 2,
            b'3' => 3,
            b'4' => 4,
            b'5' => 5,
            b'6' => 6,
            b'7' => 7,
            b'8' => 8,
            b'9' => 9,
            _ => {
                eprintln!("Illegal character found when parsing ID: {:?}", 
                    String::from_utf8_lossy(&[*digit]));
                return None
            }
        } * multiply;
        multiply *= 10;
    }
    Some(id)
}

/// Scan /dev/block/[prefix] dev files, and report which 
fn scan(prefix: &str) -> Result<()> {
    let prefix = prefix.as_bytes();
    let len_prefix = prefix.len();
    let mut ce: usize = 0;
    let mut ee: usize = 0;
    for entry in read_dir_checked("/dev/block")?
    {
        let entry = entry.map_err(|e|{
            eprintln!("Failed to read dir entry under '/dev/block': {}", e);
            Error::from(e)
        })?;
        let name = entry.file_name();
        let name = name.as_bytes();
        if ! name.starts_with(prefix) {
            continue;
        }
        let id = match id_from_bytes(&name[len_prefix..]) {
            Some(id) => id,
            None => continue,
        };
        if id == 0 {
            continue
        }
        match check_dir_entry_system(entry) {
            Ok(Some(SubSystem::HybridCoreELEC)) => if ce == 0 || ce > id {
                ce = id
            },
            Ok(Some(SubSystem::HybridEmuELEC)) => if ee == 0 || ee > id {
                ee = id
            },
            _ => (),
        }
    }
    println!("{ce} {ee}");
    Ok(())
}


#[derive(clap::Subcommand, Debug, Clone)]
enum Action {
    /// Check if the file/dev at the path contains a CoreELEC/EmuELC system.
    /// Result is any of the following if a system is found:
    /// `Official CoreELEC`, `Official EmuELEC`, `Hybird CoreELEC`, 
    /// `Hybrid EmuELEC`
    CheckSystem {
        /// Path of file/dev to verify, containing a FAT fs, containing
        /// `cfgload`, `config.ini`, `device_trees/`, `kernel.img`, `SYSTEM`
        /// in which `cfgload` is specially for a subsystem
        path: String
    },
}

#[derive(Parser, Debug)]
#[command(version)]
struct Arg {
    #[command(subcommand)]
    action: Action
}

fn main() -> Result<()> {
    let arg = Arg::parse();
    match arg.action {
        Action::CheckSystem { path } => if let Some(system) = check_path_system(&path)? {
            println!("{}", system.as_str())
        } else {
            return Err(Error::InvalidSystem)
        },
    }
    Ok(())
}