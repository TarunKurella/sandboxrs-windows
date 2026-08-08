#[cfg(not(windows))]
pub(crate) fn resolve_create_process_in_sandbox() -> Result<bool, String> {
    Ok(false)
}

#[cfg(windows)]
pub(crate) fn resolve_create_process_in_sandbox() -> Result<bool, String> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryW,
    };

    const PROCESSMODEL_DLL: &[u16] = &[
        'p' as u16, 'r' as u16, 'o' as u16, 'c' as u16, 'e' as u16,
        's' as u16, 's' as u16, 'm' as u16, 'o' as u16, 'd' as u16,
        'e' as u16, 'l' as u16, '.' as u16, 'd' as u16, 'l' as u16,
        'l' as u16, 0,
    ];
    const EXPORT: &[u8] = b"Experimental_CreateProcessInSandbox\0";

    // SAFETY: Loading a system DLL by its documented name and resolving an
    // exported function pointer is a normal library-loader operation.
    unsafe {
        let module = LoadLibraryW(PROCESSMODEL_DLL.as_ptr());
        if module.is_null() {
            return Err(format!(
                "LoadLibraryW(processmodel.dll) failed, Win32 error {}",
                GetLastError()
            ));
        }
        let symbol = GetProcAddress(module, EXPORT.as_ptr().cast());
        Ok(symbol.is_some())
    }
}
