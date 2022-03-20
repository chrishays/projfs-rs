
use std::path::{Path, PathBuf};

use windows::Win32::Storage::ProjectedFileSystem;

pub const FILE_TRANSFER_CHUNK_SIZE: u64 = 10*1024*1024;

#[derive(Clone, Debug, PartialEq)]
pub enum MatchType {
    All,
    Exact(std::ffi::OsString),
    Wildcards(std::ffi::OsString),
}

pub trait EnumerationState: Send + Sync {
    fn get_search(&self) -> Option<&MatchType>;
    fn set_search(&mut self, search: MatchType);
    fn enumerate(&mut self, callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, direntrybufferhandle: ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE) -> windows::core::HRESULT;
    fn end(&mut self);
}

pub struct NotificationMapping {
    pub bit_mask: ProjectedFileSystem::PRJ_NOTIFY_TYPES,
    pub root: PathBuf,
}

#[derive(Default)]
pub struct VirtualizationOptions {
    pub notification_mappings: Vec<NotificationMapping>,
}

pub trait SeekRead: std::io::Seek + std::io::Read {}

impl<T: std::io::Seek + std::io::Read> SeekRead for T {}

pub trait ProjFSProvider: Send + Sync {
    fn init(&mut self, root: &Path) -> Result<VirtualizationOptions, Box<dyn std::error::Error>>;
    fn start(&mut self, context: ProjectedFileSystem::PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT) -> Result<(), Box<dyn std::error::Error>>;
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    fn new_enumeration(&self, id: windows::core::GUID, file_path: &Path) -> Box<dyn EnumerationState>;
    fn get_placeholder_info(&self, file_path: &Path) -> Result<ProjectedFileSystem::PRJ_PLACEHOLDER_INFO, windows::core::HRESULT>;
    fn get_file_data(&self, file_path: &Path) -> Result<Box<dyn SeekRead>, windows::core::HRESULT>;
    fn query_file_name(&self, file_path: &Path) -> windows::core::HRESULT;
    fn notification(&self, callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, is_directory: bool, notification: ProjectedFileSystem::PRJ_NOTIFICATION, destinationfilename: windows::core::PCWSTR, operationparameters: *mut ProjectedFileSystem::PRJ_NOTIFICATION_PARAMETERS) -> windows::core::HRESULT;
}
