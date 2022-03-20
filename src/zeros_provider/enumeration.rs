use std::path::{Path, PathBuf};

use windows::Win32::Storage::ProjectedFileSystem;

use super::virtual_files::VIRTUAL_FILES;
use crate::projfs_provider::{MatchType, EnumerationState};

pub struct FakeFileEnumerateState {
    file_path: PathBuf,
    start_time: i64,
    next_index: usize,
    search: Option<MatchType>,
}

impl FakeFileEnumerateState {
    pub fn new(start_time: i64, file_path: &Path) -> FakeFileEnumerateState {
        FakeFileEnumerateState {
            file_path: file_path.to_path_buf(),
            start_time,
            next_index: 0,
            search: None,
        }
    }
}

impl EnumerationState for FakeFileEnumerateState {
    fn get_search(&self) -> Option<&MatchType> {
        self.search.as_ref()
    }

    fn set_search(&mut self, search: MatchType) {
        self.search = Some(search);
    }

    fn enumerate(&mut self, _callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, direntrybufferhandle: ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE) -> windows::core::HRESULT {
        let search = match &self.search {
            Some(s) => s,
            None => {
                return windows::Win32::Foundation::E_INVALIDARG;
            }
        };
        let mut next_index = self.next_index;
        for vf in VIRTUAL_FILES.iter().skip(next_index) {
            next_index+=1;
            let p : PathBuf = vf.0.into();
            //println!("Testing {p:?}");
            if p == self.file_path {
                continue;
            }
            match p.parent() {
                Some(v) => {
                    if v != self.file_path {
                        continue;
                    }
                },
                None => {
                    continue;
                }
            };
            //println!("{p:?} is in dir {file_path:?}");
            let dir_listing_name = p.file_name().unwrap();
            match &search {
                MatchType::All => {},
                MatchType::Exact(s) => {
                    unsafe {
                        if ProjectedFileSystem::PrjFileNameCompare(s.as_os_str(), dir_listing_name) != 0 {
                            continue;
                        }
                    }
                },
                MatchType::Wildcards(w) => {
                    unsafe {
                        if ProjectedFileSystem::PrjFileNameMatch(dir_listing_name, w.as_os_str()) == windows::Win32::Foundation::BOOLEAN(0) {
                            continue;
                        }
                    }
                },
            }
            println!("Matched {p:?}");
    
            let file_info = ProjectedFileSystem::PRJ_FILE_BASIC_INFO {
                IsDirectory: if vf.1 {
                    windows::Win32::Foundation::BOOLEAN(1)
                } else {
                    windows::Win32::Foundation::BOOLEAN(0)
                },
                FileSize: vf.2 as i64,
                CreationTime: self.start_time,
                LastAccessTime: self.start_time,
                LastWriteTime: self.start_time,
                ChangeTime: self.start_time,
                FileAttributes: 0,
            };
    
            // NOTE: These must be sent in sorted order, but since the original list is already sorted this is fine. For correct way, use PrjFileNameCompare
            unsafe {
                // If the buffer fills up we stop here
                // If symlinks are needed, use PrjFillDirEntryBuffer2 
                if ProjectedFileSystem::PrjFillDirEntryBuffer(dir_listing_name, &file_info, direntrybufferhandle).is_err() {
                    println!("Oh no, we enumerated too much. Giving the caller some space...");
                    self.next_index = next_index - 1;
                    return windows::Win32::Foundation::S_OK;
                }
            }
        }
        self.next_index = next_index;
    
        println!("Ending enumeration...for now...");
        windows::Win32::Foundation::S_OK
    }

    fn end(&mut self) {
        self.next_index = 0;
        self.search = None;
    }
}
