use std::fs;
use std::path::{PathBuf, Path};
use std::collections::HashMap;

use windows::Win32::Storage::ProjectedFileSystem;
use widestring::{WideCStr, WideCString};

use super::base::{ProjFSProvider, EnumerationState, MatchType, FILE_TRANSFER_CHUNK_SIZE};

struct ProviderState {
    provider: Box<dyn ProjFSProvider>,
    enumerations: std::sync::RwLock<HashMap<windows::core::GUID, std::sync::RwLock<Box<dyn EnumerationState>>>>,
}

#[derive(Default)]
struct GlobalState {
    providers: HashMap<isize, ProviderState>,
}

lazy_static! {
    static ref GLOBAL_STATE: std::sync::RwLock<GlobalState> = std::sync::RwLock::new(GlobalState::default());
}

extern "system" fn start_dir_enum_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, enumerationid: *const windows::core::GUID) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };
    
    let file_path : PathBuf = unsafe {
        WideCStr::from_ptr_str((*callbackdata).FilePathName.0).to_os_string().into()
    };
    
    let enum_id = unsafe { *enumerationid };

    let mut enumerations = state.enumerations.write().unwrap();
    if enumerations.insert(enum_id, std::sync::RwLock::new(state.provider.new_enumeration(enum_id, &file_path))).is_some() {
        windows::Win32::Foundation::E_INVALIDARG
    } else {
        windows::Win32::Foundation::S_OK
    }
}

extern "system" fn end_dir_enum_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, enumerationid: *const windows::core::GUID) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };

    let enum_id = unsafe { *enumerationid };

    let mut enumerations = state.enumerations.write().unwrap();
    match enumerations.remove(&enum_id) {
        None => windows::Win32::Foundation::E_INVALIDARG,
        Some(v) => {
            let mut enumeration = v.write().unwrap();
            enumeration.end();
            windows::Win32::Foundation::S_OK
        }
    }
}

extern "system" fn get_dir_enum_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, enumerationid: *const windows::core::GUID, searchexpression: windows::core::PCWSTR, direntrybufferhandle: ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };

    let enum_id = unsafe { *enumerationid };

    let enumerations_lock = state.enumerations.read().unwrap();
    let mut enumeration = match enumerations_lock.get(&enum_id) {
        None => {
            return windows::Win32::Foundation::E_INVALIDARG;
        }
        Some(v) => {
            v.write().unwrap()
        }
    };

    let flags = unsafe {
        (*callbackdata).Flags
    };
    match enumeration.get_search() {
        None => {
            let search = unsafe {
                if searchexpression.is_null() || *searchexpression.0 == 0 {
                    MatchType::All
                } else if ProjectedFileSystem::PrjDoesNameContainWildCards(searchexpression) != windows::Win32::Foundation::BOOLEAN(0) {
                    MatchType::Wildcards(WideCStr::from_ptr_str(searchexpression.0).to_os_string())
                } else {
                    MatchType::Exact(WideCStr::from_ptr_str(searchexpression.0).to_os_string())
                }
            };
            println!("Starting enumeration for {enum_id:?} with expression {search:?}");
            enumeration.set_search(search);
        }
        Some(og_se) => {
            if flags.0 & ProjectedFileSystem::PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN.0 != 0 {
                println!("Resetting enumeration for {enum_id:?}");
                let search = unsafe {
                    if searchexpression.is_null() || *searchexpression.0 == 0 {
                        MatchType::All
                    } else if ProjectedFileSystem::PrjDoesNameContainWildCards(searchexpression) != windows::Win32::Foundation::BOOLEAN(0) {
                        MatchType::Wildcards(WideCStr::from_ptr_str(searchexpression.0).to_os_string())
                    } else {
                        MatchType::Exact(WideCStr::from_ptr_str(searchexpression.0).to_os_string())
                    }
                };
                if *og_se != search {
                    println!("Search expression changed from {og_se:?} to {search:?}");
                } else {
                    println!("Search expression stayed {search:?}");
                }
                enumeration.set_search(search);
            } else {
                println!("Continuing enumeration for {enum_id:?} with search expression {og_se:?}");
            }
        }
    }

    enumeration.enumerate(callbackdata, direntrybufferhandle)
}

extern "system" fn get_placeholder_info_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };
    let file_path : PathBuf = unsafe {
        WideCStr::from_ptr_str((*callbackdata).FilePathName.0).to_os_string().into()
    };

    let placeholder_info = match state.provider.get_placeholder_info(&file_path) {
        Ok(p) => p,
        Err(e) => {
            return e;
        }
    };

    let placeholder_info_size = std::mem::size_of::<ProjectedFileSystem::PRJ_PLACEHOLDER_INFO>() as u32;

    unsafe {
        // Use PrjWritePlaceholderInfo2 for symlink support
        ProjectedFileSystem::PrjWritePlaceholderInfo((*callbackdata).NamespaceVirtualizationContext, (*callbackdata).FilePathName, &placeholder_info, placeholder_info_size)
    }.map_err(|e| e.code()).err().unwrap_or(windows::Win32::Foundation::S_OK)
}

fn block_align_truncate(p : u64, v : u64) -> u64 {
    (p) & (0-v)
}

extern "system" fn get_file_data_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, byteoffset: u64, length: u32) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };
    let file_path : PathBuf = unsafe {
        WideCStr::from_ptr_str((*callbackdata).FilePathName.0).to_os_string().into()
    };

    let mut reader = match state.provider.get_file_data(&file_path) {
        Ok(r) => r,
        Err(e) => {
            return e;
        }
    };

    if reader.seek(std::io::SeekFrom::Start(byteoffset)).is_err() {
        return windows::Win32::Foundation::E_ABORT;
    }
    
    let (write_start_offset, mut write_length) = if length as u64 <= FILE_TRANSFER_CHUNK_SIZE {
        // Read the entire chunk in one go
        (byteoffset, length)
    } else {
        let instance_info = unsafe {
            ProjectedFileSystem::PrjGetVirtualizationInstanceInfo((*callbackdata).NamespaceVirtualizationContext)
        }.unwrap();

        let write_end_offset = block_align_truncate(byteoffset + FILE_TRANSFER_CHUNK_SIZE, instance_info.WriteAlignment as u64);

        (write_end_offset, (write_end_offset - byteoffset) as u32)
    };


    let write_buffer = unsafe {
        ProjectedFileSystem::PrjAllocateAlignedBuffer((*callbackdata).NamespaceVirtualizationContext, write_length as usize)
    };
    if write_buffer.is_null() {
        return windows::Win32::Foundation::E_OUTOFMEMORY;
    }
    let write_slice = unsafe {
        std::slice::from_raw_parts_mut(write_buffer as *mut u8, write_length as usize)
    };

    let mut cur_length = length;
    while write_length > 0 {
        // TODO: This might need to check if less was read
        if reader.read(write_slice).is_err() {
            return windows::Win32::Foundation::E_ABORT;
        }
        unsafe {
            if let Err(e) = ProjectedFileSystem::PrjWriteFileData((*callbackdata).NamespaceVirtualizationContext, &(*callbackdata).DataStreamId, write_buffer, write_start_offset, write_length) {
                ProjectedFileSystem::PrjFreeAlignedBuffer(write_buffer);
                return e.code();
            }
        }
        cur_length -= write_length;
        if cur_length < write_length {
            write_length = cur_length;
        }
    }

    unsafe {
        ProjectedFileSystem::PrjFreeAlignedBuffer(write_buffer)
    };
    //windows::Win32::Foundation::ERROR_ACCESS_DENIED.into()

    windows::Win32::Foundation::S_OK
}

extern "system" fn query_file_name_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA) -> windows::core::HRESULT { 
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };
    let file_path : PathBuf = unsafe {
        WideCStr::from_ptr_str((*callbackdata).FilePathName.0).to_os_string().into()
    };

    state.provider.query_file_name(&file_path)
}

// extern "system" fn cancel_command_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA) {
//     println!("cancel_command");
// }

extern "system" fn notification_callback(callbackdata: *const ProjectedFileSystem::PRJ_CALLBACK_DATA, is_directory: windows::Win32::Foundation::BOOLEAN, notification: ProjectedFileSystem::PRJ_NOTIFICATION, destinationfilename: windows::core::PCWSTR, operationparameters: *mut ProjectedFileSystem::PRJ_NOTIFICATION_PARAMETERS) -> windows::core::HRESULT {
    let context = unsafe {
        (*callbackdata).NamespaceVirtualizationContext
    };
    let data = GLOBAL_STATE.read().unwrap();

    let state = match data.providers.get(&context.0) {
        Some(v) => v,
        None => {
            return windows::Win32::Foundation::E_INVALIDARG
        }
    };
    let is_directory = is_directory != windows::Win32::Foundation::BOOLEAN(0);
    
    state.provider.notification(callbackdata, is_directory, notification, destinationfilename, operationparameters)
}
pub struct ProjFSRunner {
    root: PathBuf,
    id: windows::core::GUID,
    instance: ProjectedFileSystem::PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
}

impl ProjFSRunner {
    pub fn new() -> ProjFSRunner {
        ProjFSRunner {
            root: PathBuf::new(),
            id: windows::core::GUID::zeroed(),
            instance: ProjectedFileSystem::PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT::default(),
        }
    }

    pub fn start(&mut self, root: &Path, mut provider: Box<dyn ProjFSProvider>) -> Result<(), Box<dyn std::error::Error>> {
        self.root = root.to_path_buf();
        // Since PrjMarkDirectoryAsPlaceholder marks this as a reparse point it can't be ran two times in a row (maybe just with a different id?)
        //fs::remove_dir(&root).unwrap();
        self.root = if !root.exists() {
            // TODO: Delete the directory on all error paths
            fs::create_dir_all(&root)?;
            
            // Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart
            self.id = windows::core::GUID::new()?;
            //let proj_version_info = ProjectedFileSystem::PRJ_PLACEHOLDER_VERSION_INFO::default();
            let projection = fs::canonicalize(&root)?;
            unsafe {
                ProjectedFileSystem::PrjMarkDirectoryAsPlaceholder(projection.as_os_str(), windows::core::PCWSTR::default(), std::ptr::null(), &self.id)?;
            }
            projection
        } else {
            fs::canonicalize(&root)?
        };

        let prov_options = provider.init(root)?;
        let callbacks = ProjectedFileSystem::PRJ_CALLBACKS {
            // Required
            StartDirectoryEnumerationCallback: Some(start_dir_enum_callback),
            EndDirectoryEnumerationCallback: Some(end_dir_enum_callback),
            GetDirectoryEnumerationCallback: Some(get_dir_enum_callback),
            GetPlaceholderInfoCallback: Some(get_placeholder_info_callback),
            GetFileDataCallback: Some(get_file_data_callback),
            // Optional
            QueryFileNameCallback: Some(query_file_name_callback),
            //QueryFileNameCallback: None, // If this is not set directory enumeration is used for file existence checks (no good examples of this)
            NotificationCallback: Some(notification_callback),
            //NotificationCallback: None, // Noisy for now
            //CancelCommandCallback: Some(cancel_command_callback),
            CancelCommandCallback: None, // If this is not set callbacks are synchronously called
        };

        let mut options = ProjectedFileSystem::PRJ_STARTVIRTUALIZING_OPTIONS {
            Flags: ProjectedFileSystem::PRJ_FLAG_NONE,
            ConcurrentThreadCount: 0,  // Twice the logical processors
            PoolThreadCount: 0,  // Twice the ConcurrentThreadCount
            NotificationMappings: std::ptr::null_mut(),
            NotificationMappingsCount: 0,
        };
        self.instance = unsafe {
            let mut mappings = Vec::with_capacity(prov_options.notification_mappings.len());
            let mut wide_strs :Vec<WideCString> = Vec::with_capacity(prov_options.notification_mappings.len());
            if !prov_options.notification_mappings.is_empty() {
                for m in &prov_options.notification_mappings {
                    wide_strs.push(WideCString::from_os_str_unchecked(m.root.as_os_str()));
                    let not_root: windows::core::PCWSTR = if let Some(s) = wide_strs.last() {
                        windows::core::PCWSTR(s.as_ptr())
                    } else {
                        windows::core::PCWSTR::default()
                    };
                    mappings.push(ProjectedFileSystem::PRJ_NOTIFICATION_MAPPING {
                        NotificationBitMask: m.bit_mask,
                        NotificationRoot: not_root,
                    });
                }
                options.NotificationMappings = mappings.as_mut_ptr();
                options.NotificationMappingsCount = mappings.len() as u32;
            }
            // Unfortunately this is a race condition. The value we need for future calls is the return result of the function that can spawn the calls
            ProjectedFileSystem::PrjStartVirtualizing(self.root.as_os_str(), &callbacks, std::ptr::null(), &options)?
        };

        let state = ProviderState{
            provider,
            enumerations: std::sync::RwLock::new(HashMap::new()),
        };
        {
            let mut data = GLOBAL_STATE.write()?;
            if data.providers.insert(self.instance.0, state).is_some() {
                return Err("A provider already exists".into());
            }
            if let Some(v) = data.providers.get_mut(&self.instance.0) {
                v.provider.start(self.instance)?;
            }
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut data = GLOBAL_STATE.write()?;
        match data.providers.remove(&self.instance.0) {
            Some(mut p) => {
                p.provider.stop()?;
            },
            None => {
                return Err("A provider doesn't".into());
            }
        }

        unsafe {
            ProjectedFileSystem::PrjStopVirtualizing(&self.instance);
        }

        println!("Shut down");

        Ok(())
    }
}