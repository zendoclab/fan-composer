use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct MixerStatus {
    pub endpoint_id: String,
    pub master_volume: f32,
    pub master_db: f32,
    pub master_muted: bool,
    pub session_volume: Option<f32>,
    pub session_muted: Option<bool>,
    pub endpoint_peak: Option<f32>,
}
/// Read-only COM query on its own MTA thread, never an audio callback.
pub fn output_mixer(id: Option<String>, process_id: u32) -> Result<MixerStatus, String> {
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || query(id, process_id))
            .join()
            .map_err(|_| "Windows mixer query panicked".to_owned())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (id, process_id);
        Err("Windows mixer unavailable on this OS".into())
    }
}
#[cfg(target_os = "windows")]
fn query(id: Option<String>, process_id: u32) -> Result<MixerStatus, String> {
    use windows::{
        Win32::{
            Media::Audio::{
                Endpoints::{IAudioEndpointVolume, IAudioMeterInformation},
                IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
                ISimpleAudioVolume, MMDeviceEnumerator, eConsole, eRender,
            },
            System::Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
        },
        core::{Interface, PCWSTR},
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let read = || -> windows::core::Result<MixerStatus> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let _apartment = Apartment;
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let endpoint = if let Some(id) = id.filter(|s| !s.is_empty()) {
                let plain = id.strip_prefix("wasapi:").unwrap_or(&id);
                let utf16: Vec<u16> = plain.encode_utf16().chain(std::iter::once(0)).collect();
                enumerator.GetDevice(PCWSTR(utf16.as_ptr()))?
            } else {
                enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?
            };
            let endpoint_volume: IAudioEndpointVolume = endpoint.Activate(CLSCTX_ALL, None)?;
            let native_id = endpoint.GetId()?;
            let endpoint_id = native_id.to_string()?;
            CoTaskMemFree(Some(native_id.0.cast()));
            let manager: IAudioSessionManager2 = endpoint.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            let mut session_volume = None;
            let mut session_muted = None;
            for index in 0..sessions.GetCount()? {
                let control = sessions.GetSession(index)?;
                if let Ok(control2) = control.cast::<IAudioSessionControl2>()
                    && control2.GetProcessId()? == process_id
                {
                    let volume: ISimpleAudioVolume = control.cast()?;
                    session_volume = Some(volume.GetMasterVolume()?);
                    session_muted = Some(volume.GetMute()?.as_bool());
                    break;
                }
            }
            let endpoint_peak = endpoint
                .Activate::<IAudioMeterInformation>(CLSCTX_ALL, None)
                .ok()
                .and_then(|m| m.GetPeakValue().ok());
            Ok(MixerStatus {
                endpoint_id,
                master_volume: endpoint_volume.GetMasterVolumeLevelScalar()?,
                master_db: endpoint_volume.GetMasterVolumeLevel()?,
                master_muted: endpoint_volume.GetMute()?.as_bool(),
                session_volume,
                session_muted,
                endpoint_peak,
            })
        }
    };
    read().map_err(|e| e.to_string())
}
