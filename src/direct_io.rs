//! Direct I/O — bypass OS file cache for honest disk benchmarking.
//!
//! This is the core of UBench. Without it, all benchmarks are lies:
//! - Read benchmarks return memory speed (5+ GB/s) instead of disk speed
//! - 4K random read IOPS is inflated 100-300x
//!
//! Implementation:
//! - Windows: CreateFileW with FILE_FLAG_NO_BUFFERING + FILE_FLAG_WRITE_THROUGH
//!   (same flags Microsoft DiskSpd / CrystalDiskMark use internally)
//! - macOS: F_NOCACHE fcntl on opened file descriptor
//!
//! Constraints when NO_BUFFERING is on (Windows):
//! - Buffer addresses must be sector-aligned (we use page-aligned, safer)
//! - Buffer sizes must be multiples of the volume sector size (typ. 512 or 4096)
//! - File offsets must be multiples of sector size
//!
//! See: https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering


/// Sector alignment for direct I/O operations.
/// 4096 covers all common drives (512n, 512e, 4Kn) safely.
pub const SECTOR_SIZE: usize = 4096;

#[cfg(windows)]
mod imp {
    use super::SECTOR_SIZE;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, SetFilePointerEx, WriteFile, CREATE_ALWAYS, FILE_BEGIN,
        FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };

    /// File handle opened with NO_BUFFERING — truly bypasses OS cache.
    pub struct DirectFile {
        handle: HANDLE,
    }

    impl DirectFile {
        pub fn create(path: &Path) -> io::Result<Self> {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    CREATE_ALWAYS,
                    FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
                    None,
                )
                .map_err(|e| io::Error::other(format!("CreateFileW (write): {}", e)))?
            };
            Ok(DirectFile { handle })
        }

        pub fn open_read(path: &Path) -> io::Result<Self> {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    GENERIC_READ.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_NO_BUFFERING,
                    None,
                )
                .map_err(|e| io::Error::other(format!("CreateFileW (read): {}", e)))?
            };
            Ok(DirectFile { handle })
        }

        pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            assert!(
                buf.len() % SECTOR_SIZE == 0,
                "direct write buffer must be sector-aligned ({} bytes)",
                SECTOR_SIZE
            );
            let mut written: u32 = 0;
            unsafe {
                WriteFile(self.handle, Some(buf), Some(&mut written), None)
                    .map_err(|e| io::Error::other(format!("WriteFile: {}", e)))?;
            }
            if written as usize != buf.len() {
                return Err(io::Error::other(format!(
                    "Short write: wrote {} of {} bytes",
                    written,
                    buf.len()
                )));
            }
            Ok(())
        }

        pub fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
            assert!(
                buf.len() % SECTOR_SIZE == 0,
                "direct read buffer must be sector-aligned ({} bytes)",
                SECTOR_SIZE
            );
            let mut read: u32 = 0;
            unsafe {
                ReadFile(self.handle, Some(buf), Some(&mut read), None)
                    .map_err(|e| io::Error::other(format!("ReadFile: {}", e)))?;
            }
            if read as usize != buf.len() {
                return Err(io::Error::other(format!(
                    "Short read: got {} of {} bytes",
                    read,
                    buf.len()
                )));
            }
            Ok(())
        }

        pub fn seek(&mut self, offset: u64) -> io::Result<()> {
            assert!(
                offset as usize % SECTOR_SIZE == 0,
                "direct seek offset must be sector-aligned ({} bytes)",
                SECTOR_SIZE
            );
            unsafe {
                SetFilePointerEx(self.handle, offset as i64, None, FILE_BEGIN)
                    .map_err(|e| io::Error::other(format!("SetFilePointerEx: {}", e)))?;
            }
            Ok(())
        }
    }

    impl Drop for DirectFile {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    /// Allocate a sector-aligned buffer (required for FILE_FLAG_NO_BUFFERING).
    /// VirtualAlloc returns page-aligned (4096-byte) memory which satisfies all known sector sizes.
    pub fn aligned_buffer(size: usize) -> AlignedBuffer {
        assert!(size % SECTOR_SIZE == 0, "buffer size must be sector-aligned");
        use windows::Win32::System::Memory::{
            VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
        };
        let ptr = unsafe {
            VirtualAlloc(
                None,
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        assert!(!ptr.is_null(), "VirtualAlloc failed");
        AlignedBuffer { ptr: ptr as *mut u8, size }
    }

    pub struct AlignedBuffer {
        ptr: *mut u8,
        size: usize,
    }

    impl AlignedBuffer {
        pub fn as_slice(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
        }
        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
        }
        pub fn len(&self) -> usize {
            self.size
        }
        pub fn fill_random(&mut self, seed: u64) {
            let mut state = if seed == 0 { 1 } else { seed };
            for chunk in self.as_mut_slice().chunks_mut(8) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let bytes = state.to_le_bytes();
                let n = chunk.len();
                chunk.copy_from_slice(&bytes[..n]);
            }
        }
    }

    impl Drop for AlignedBuffer {
        fn drop(&mut self) {
            use windows::Win32::System::Memory::{VirtualFree, MEM_RELEASE};
            unsafe {
                let _ = VirtualFree(self.ptr as _, 0, MEM_RELEASE);
            }
        }
    }

    // Safety: AlignedBuffer holds a unique allocation; raw pointer makes it !Send by default,
    // but the allocation is owned exclusively, so manual transfer between threads is safe.
    unsafe impl Send for AlignedBuffer {}
}

#[cfg(unix)]
mod imp {
    use super::SECTOR_SIZE;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    pub struct DirectFile {
        file: File,
    }

    impl DirectFile {
        pub fn create(path: &Path) -> io::Result<Self> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;
            Self::set_nocache(&file)?;
            Ok(DirectFile { file })
        }
        pub fn open_read(path: &Path) -> io::Result<Self> {
            let file = OpenOptions::new().read(true).open(path)?;
            Self::set_nocache(&file)?;
            Ok(DirectFile { file })
        }
        #[cfg(target_os = "macos")]
        fn set_nocache(file: &File) -> io::Result<()> {
            // F_NOCACHE = 48 on macOS
            const F_NOCACHE: i32 = 48;
            let r = unsafe { libc::fcntl(file.as_raw_fd(), F_NOCACHE, 1) };
            if r == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        fn set_nocache(_file: &File) -> io::Result<()> {
            // Linux: O_DIRECT is the equivalent but requires open-time flag.
            // For now, posix_fadvise(POSIX_FADV_DONTNEED) is best-effort.
            Ok(())
        }
        pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            self.file.write_all(buf)
        }
        pub fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
            self.file.read_exact(buf)
        }
        pub fn seek(&mut self, offset: u64) -> io::Result<()> {
            self.file.seek(SeekFrom::Start(offset)).map(|_| ())
        }
    }

    pub fn aligned_buffer(size: usize) -> AlignedBuffer {
        let v = vec![0u8; size];
        AlignedBuffer { data: v }
    }

    pub struct AlignedBuffer {
        data: Vec<u8>,
    }
    impl AlignedBuffer {
        pub fn as_slice(&self) -> &[u8] {
            &self.data
        }
        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.data
        }
        pub fn len(&self) -> usize {
            self.data.len()
        }
        pub fn fill_random(&mut self, seed: u64) {
            let mut state = if seed == 0 { 1 } else { seed };
            for chunk in self.data.chunks_mut(8) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let bytes = state.to_le_bytes();
                let n = chunk.len();
                chunk.copy_from_slice(&bytes[..n]);
            }
        }
    }
}

pub use imp::{aligned_buffer, AlignedBuffer, DirectFile};
