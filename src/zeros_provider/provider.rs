use std::path::{PathBuf, Path};

use widestring::WideCStr;
use windows::Win32::Storage::ProjectedFileSystem;

use crate::projfs_provider::{ProjFSProvider, EnumerationState, SeekRead, VirtualizationOptions, NotificationMapping};
use super::zero_reader::ZeroReader;
use super::virtual_files::VIRTUAL_FILES;
use super::enumeration::FakeFileEnumerateState;


#[derive(Default)]
pub struct ZerosProvider {
    start_time: i64,
    root: PathBuf,
    files_read: std::sync::atomic::AtomicUsize,
    file_names_read: std::sync::RwLock<std::vec::Vec<PathBuf>>,
}

impl ZerosProvider {
    pub fn new() -> ZerosProvider {
        ZerosProvider::default()
    }
}

impl ProjFSProvider for ZerosProvider {
    fn init(&mut self, root: &Path) -> Result<VirtualizationOptions, Box<dyn std::error::Error>> {
        self.root = root.to_path_buf();

        // Get all the notifications
        let mut options = VirtualizationOptions::default();
        options.notification_mappings.push(NotificationMapping{
            bit_mask: //ProjectedFileSystem::PRJ_NOTIFY_FILE_OPENED
                ProjectedFileSystem::PRJ_NOTIFY_NEW_FILE_CREATED
                | ProjectedFileSystem::PRJ_NOTIFY_FILE_OVERWRITTEN
                | ProjectedFileSystem::PRJ_NOTIFY_PRE_DELETE
                | ProjectedFileSystem::PRJ_NOTIFY_PRE_RENAME
                | ProjectedFileSystem::PRJ_NOTIFY_PRE_SET_HARDLINK
                | ProjectedFileSystem::PRJ_NOTIFY_HARDLINK_CREATED
                | ProjectedFileSystem::PRJ_NOTIFY_FILE_HANDLE_CLOSED_NO_MODIFICATION
                | ProjectedFileSystem::PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
                | ProjectedFileSystem::PRJ_NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED
                | ProjectedFileSystem::PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL,
            root: PathBuf::new(),
        });
        Ok(options)
    }
    fn start(&mut self, _instance: ProjectedFileSystem::PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT) -> Result<(), Box<dyn std::error::Error>> {
        let ftp :*mut i64 = &mut self.start_time;
        let ip = ftp as *mut windows::Win32::Foundation::FILETIME;
        unsafe {
            windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime(ip);
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn new_enumeration(&self, _id: windows::core::GUID, file_path: &Path) -> Box<dyn EnumerationState> {
        Box::new(FakeFileEnumerateState::new(self.start_time, file_path))
    }

    fn get_placeholder_info(&self, file_path: &Path) -> Result<ProjectedFileSystem::PRJ_PLACEHOLDER_INFO, windows::core::HRESULT> {
        let search_name = file_path.as_os_str();
        for vf in VIRTUAL_FILES {
            unsafe {
                let found_name = std::ffi::OsString::from(vf.0);
                if ProjectedFileSystem::PrjFileNameCompare(found_name, search_name) != 0 {
                    continue;
                }
            }
    
            return Ok(ProjectedFileSystem::PRJ_PLACEHOLDER_INFO {
                FileBasicInfo: ProjectedFileSystem::PRJ_FILE_BASIC_INFO {
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
                },
                ..Default::default()
            });
        }

        Err(windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.into())
    }

    fn get_file_data(&self, file_path: &Path) -> Result<Box<dyn SeekRead>, windows::core::HRESULT> {
        let search_name = file_path.as_os_str();
        for vf in VIRTUAL_FILES {
            let found_name = std::ffi::OsString::from(vf.0);
            unsafe {
                if ProjectedFileSystem::PrjFileNameCompare(found_name, search_name) != 0 {
                    continue;
                }
            }

            // Do thread safe mutations without mutability on self
            self.files_read.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            {
                let mut lock = self.file_names_read.write().unwrap();
                lock.push(file_path.to_path_buf());
            }
            return Ok(Box::new(ZeroReader::new(vf.2 as u64)));
        }

        Err(windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.into())
    }

    fn query_file_name(&self, file_path: &Path) -> windows::core::HRESULT {
        let search_name = file_path.as_os_str();
        for vf in VIRTUAL_FILES {
            unsafe {
                let found_name = std::ffi::OsString::from(vf.0);
                if ProjectedFileSystem::PrjFileNameCompare(found_name, search_name) == 0 {
                    return windows::Win32::Foundation::S_OK;
                }
            }
        }
    
        windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.into()
    }

    fn notification(&self, callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, is_directory: bool, notification: ProjectedFileSystem::PRJ_NOTIFICATION, destinationfilename: windows::core::PCWSTR, _operationparameters: *mut ProjectedFileSystem::PRJ_NOTIFICATION_PARAMETERS) -> windows::core::HRESULT {
        let file_path : PathBuf = unsafe {
            WideCStr::from_ptr_str((*callbackdata).FilePathName.0).to_os_string().into()
        };
        let triggering_file_name : PathBuf = unsafe {
            WideCStr::from_ptr_str((*callbackdata).TriggeringProcessImageFileName.0).to_os_string().into()
        };
        println!("notification: Path {file_path:?} (dir: {is_directory:?}) triggerd by {triggering_file_name:?}");
        match notification {
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_OPENED => {
                println!("PRJ_NOTIFICATION_FILE_OPENED");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_NEW_FILE_CREATED => {
                println!("PRJ_NOTIFICATION_NEW_FILE_CREATED");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_OVERWRITTEN => {
                println!("PRJ_NOTIFICATION_FILE_OVERWRITTEN");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_PRE_DELETE => {
                println!("PRJ_NOTIFICATION_PRE_DELETE");
                // Don't allow deletes
                return windows::Win32::Foundation::STATUS_CANNOT_DELETE.into();
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_PRE_RENAME => {
                println!("PRJ_NOTIFICATION_PRE_RENAME");
                // Don't allow renames
                return windows::Win32::Foundation::ERROR_ACCESS_DENIED.into();
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_PRE_SET_HARDLINK => {
                println!("PRJ_NOTIFICATION_PRE_SET_HARDLINK");
                // Don't allow hardlinks
                return windows::Win32::Foundation::ERROR_ACCESS_DENIED.into();
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_RENAMED => {
                let dest_file_name : PathBuf = unsafe {
                    WideCStr::from_ptr_str(destinationfilename.0).to_os_string().into()
                };
                println!("PRJ_NOTIFICATION_FILE_RENAMED to {dest_file_name:?}");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_HARDLINK_CREATED => {
                println!("PRJ_NOTIFICATION_HARDLINK_CREATED");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_NO_MODIFICATION => {
                println!("PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_NO_MODIFICATION");
                let full_file_path = self.root.join(&file_path);
                let state = unsafe {
                    match ProjectedFileSystem::PrjGetOnDiskFileState(full_file_path.as_os_str()) {
                        Ok(v) => {
                            v
                        },
                        Err(e) => {
                            // For some reason it can't fine the name specified
                            println!("PrjGetOnDiskFileState {file_path:?} error {e:?}");
                            return e.into()
                        }
                    }
                };
                let delete_states = ProjectedFileSystem::PRJ_FILE_STATE_PLACEHOLDER // On disk placeholder
                    | ProjectedFileSystem::PRJ_FILE_STATE_HYDRATED_PLACEHOLDER // File content written to disk
                    | ProjectedFileSystem::PRJ_FILE_STATE_DIRTY_PLACEHOLDER;  // Metadata modified
                if (state & delete_states).0 != 0 {
                    println!("Deleting {file_path:?} with state {state:?}");
                    let update_flags = ProjectedFileSystem::PRJ_UPDATE_ALLOW_DIRTY_DATA
                        | ProjectedFileSystem::PRJ_UPDATE_ALLOW_DIRTY_METADATA
                        | ProjectedFileSystem::PRJ_UPDATE_ALLOW_TOMBSTONE;
                    unsafe {
                        if let Err(e) = ProjectedFileSystem::PrjDeleteFile((*callbackdata).NamespaceVirtualizationContext, (*callbackdata).FilePathName, update_flags) {
                            return e.into();
                        }
                    }
                } else {
                    println!("Not deleting {file_path:?} with state {state:?}");
                }
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED => {
                println!("PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_MODIFIED");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_DELETED => {
                println!("PRJ_NOTIFICATION_FILE_HANDLE_CLOSED_FILE_DELETED");
            }
            ProjectedFileSystem::PRJ_NOTIFICATION_FILE_PRE_CONVERT_TO_FULL => {
                println!("PRJ_NOTIFICATION_FILE_PRE_CONVERT_TO_FULL");
            }
            _ => {
                println!("NOTIFICATION UNKNOWN - {notification:?}");
            }
        };
    
        windows::Win32::Foundation::S_OK
    }
}