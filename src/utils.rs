use anyhow::Result;
use geometrydash::fmod::{self, FMOD_RESULT};
use std::ffi::CString;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;

pub fn capitalize_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub trait IntoFmodResult {
    fn fmod_result(self) -> Result<()>;
}

impl IntoFmodResult for FMOD_RESULT {
    fn fmod_result(self) -> Result<()> {
        if self == fmod::FMOD_OK {
            return Ok(());
        }
        // errors from fmod_errors.h
        anyhow::bail!(match self {
            fmod::FMOD_ERR_BADCOMMAND =>                "Tried to call a function on a data type that does not allow this type of functionality (ie calling Sound::lock on a streaming sound).",
            fmod::FMOD_ERR_CHANNEL_ALLOC =>             "Error trying to allocate a channel.",
            fmod::FMOD_ERR_CHANNEL_STOLEN =>            "The specified channel has been reused to play another sound.",
            fmod::FMOD_ERR_DMA =>                       "DMA Failure.  See debug output for more information.",
            fmod::FMOD_ERR_DSP_CONNECTION =>            "DSP connection error.  Connection possibly caused a cyclic dependency or connected dsps with incompatible buffer counts.",
            fmod::FMOD_ERR_DSP_DONTPROCESS =>           "DSP return code from a DSP process query callback.  Tells mixer not to call the process callback and therefore not consume CPU.  Use this to optimize the DSP graph.",
            fmod::FMOD_ERR_DSP_FORMAT =>                "DSP Format error.  A DSP unit may have attempted to connect to this network with the wrong format, or a matrix may have been set with the wrong size if the target unit has a specified channel map.",
            fmod::FMOD_ERR_DSP_INUSE =>                 "DSP is already in the mixer's DSP network. It must be removed before being reinserted or released.",
            fmod::FMOD_ERR_DSP_NOTFOUND =>              "DSP connection error.  Couldn't find the DSP unit specified.",
            fmod::FMOD_ERR_DSP_RESERVED =>              "DSP operation error.  Cannot perform operation on this DSP as it is reserved by the system.",
            fmod::FMOD_ERR_DSP_SILENCE =>               "DSP return code from a DSP process query callback.  Tells mixer silence would be produced from read, so go idle and not consume CPU.  Use this to optimize the DSP graph.",
            fmod::FMOD_ERR_DSP_TYPE =>                  "DSP operation cannot be performed on a DSP of this type.",
            fmod::FMOD_ERR_FILE_BAD =>                  "Error loading file.",
            fmod::FMOD_ERR_FILE_COULDNOTSEEK =>         "Couldn't perform seek operation.  This is a limitation of the medium (ie netstreams) or the file format.",
            fmod::FMOD_ERR_FILE_DISKEJECTED =>          "Media was ejected while reading.",
            fmod::FMOD_ERR_FILE_EOF =>                  "End of file unexpectedly reached while trying to read essential data (truncated?).",
            fmod::FMOD_ERR_FILE_ENDOFDATA =>            "End of current chunk reached while trying to read data.",
            fmod::FMOD_ERR_FILE_NOTFOUND =>             "File not found.",
            fmod::FMOD_ERR_FORMAT =>                    "Unsupported file or audio format.",
            fmod::FMOD_ERR_HEADER_MISMATCH =>           "There is a version mismatch between the FMOD header and either the FMOD Studio library or the FMOD Low Level library.",
            fmod::FMOD_ERR_HTTP =>                      "A HTTP error occurred. This is a catch-all for HTTP errors not listed elsewhere.",
            fmod::FMOD_ERR_HTTP_ACCESS =>               "The specified resource requires authentication or is forbidden.",
            fmod::FMOD_ERR_HTTP_PROXY_AUTH =>           "Proxy authentication is required to access the specified resource.",
            fmod::FMOD_ERR_HTTP_SERVER_ERROR =>         "A HTTP server error occurred.",
            fmod::FMOD_ERR_HTTP_TIMEOUT =>              "The HTTP request timed out.",
            fmod::FMOD_ERR_INITIALIZATION =>            "FMOD was not initialized correctly to support this function.",
            fmod::FMOD_ERR_INITIALIZED =>               "Cannot call this command after System::init.",
            fmod::FMOD_ERR_INTERNAL =>                  "An error occurred that wasn't supposed to.  Contact support.",
            fmod::FMOD_ERR_INVALID_FLOAT =>             "Value passed in was a NaN, Inf or denormalized float.",
            fmod::FMOD_ERR_INVALID_HANDLE =>            "An invalid object handle was used.",
            fmod::FMOD_ERR_INVALID_PARAM =>             "An invalid parameter was passed to this function.",
            fmod::FMOD_ERR_INVALID_POSITION =>          "An invalid seek position was passed to this function.",
            fmod::FMOD_ERR_INVALID_SPEAKER =>           "An invalid speaker was passed to this function based on the current speaker mode.",
            fmod::FMOD_ERR_INVALID_SYNCPOINT =>         "The syncpoint did not come from this sound handle.",
            fmod::FMOD_ERR_INVALID_THREAD =>            "Tried to call a function on a thread that is not supported.",
            fmod::FMOD_ERR_INVALID_VECTOR =>            "The vectors passed in are not unit length, or perpendicular.",
            fmod::FMOD_ERR_MAXAUDIBLE =>                "Reached maximum audible playback count for this sound's soundgroup.",
            fmod::FMOD_ERR_MEMORY =>                    "Not enough memory or resources.",
            fmod::FMOD_ERR_MEMORY_CANTPOINT =>          "Can't use FMOD_OPENMEMORY_POINT on non PCM source data, or non mp3/xma/adpcm data if FMOD_CREATECOMPRESSEDSAMPLE was used.",
            fmod::FMOD_ERR_NEEDS3D =>                   "Tried to call a command on a 2d sound when the command was meant for 3d sound.",
            fmod::FMOD_ERR_NEEDSHARDWARE =>             "Tried to use a feature that requires hardware support.",
            fmod::FMOD_ERR_NET_CONNECT =>               "Couldn't connect to the specified host.",
            fmod::FMOD_ERR_NET_SOCKET_ERROR =>          "A socket error occurred.  This is a catch-all for socket-related errors not listed elsewhere.",
            fmod::FMOD_ERR_NET_URL =>                   "The specified URL couldn't be resolved.",
            fmod::FMOD_ERR_NET_WOULD_BLOCK =>           "Operation on a non-blocking socket could not complete immediately.",
            fmod::FMOD_ERR_NOTREADY =>                  "Operation could not be performed because specified sound/DSP connection is not ready.",
            fmod::FMOD_ERR_OUTPUT_ALLOCATED =>          "Error initializing output device, but more specifically, the output device is already in use and cannot be reused.",
            fmod::FMOD_ERR_OUTPUT_CREATEBUFFER =>       "Error creating hardware sound buffer.",
            fmod::FMOD_ERR_OUTPUT_DRIVERCALL =>         "A call to a standard soundcard driver failed, which could possibly mean a bug in the driver or resources were missing or exhausted.",
            fmod::FMOD_ERR_OUTPUT_FORMAT =>             "Soundcard does not support the specified format.",
            fmod::FMOD_ERR_OUTPUT_INIT =>               "Error initializing output device.",
            fmod::FMOD_ERR_OUTPUT_NODRIVERS =>          "The output device has no drivers installed.  If pre-init, FMOD_OUTPUT_NOSOUND is selected as the output mode.  If post-init, the function just fails.",
            fmod::FMOD_ERR_PLUGIN =>                    "An unspecified error has been returned from a plugin.",
            fmod::FMOD_ERR_PLUGIN_MISSING =>            "A requested output, dsp unit type or codec was not available.",
            fmod::FMOD_ERR_PLUGIN_RESOURCE =>           "A resource that the plugin requires cannot be found. (ie the DLS file for MIDI playback)",
            fmod::FMOD_ERR_PLUGIN_VERSION =>            "A plugin was built with an unsupported SDK version.",
            fmod::FMOD_ERR_RECORD =>                    "An error occurred trying to initialize the recording device.",
            fmod::FMOD_ERR_REVERB_CHANNELGROUP =>       "Reverb properties cannot be set on this channel because a parent channelgroup owns the reverb connection.",
            fmod::FMOD_ERR_REVERB_INSTANCE =>           "Specified instance in FMOD_REVERB_PROPERTIES couldn't be set. Most likely because it is an invalid instance number or the reverb doesn't exist.",
            fmod::FMOD_ERR_SUBSOUNDS =>                 "The error occurred because the sound referenced contains subsounds when it shouldn't have, or it doesn't contain subsounds when it should have.  The operation may also not be able to be performed on a parent sound.",
            fmod::FMOD_ERR_SUBSOUND_ALLOCATED =>        "This subsound is already being used by another sound, you cannot have more than one parent to a sound.  Null out the other parent's entry first.",
            fmod::FMOD_ERR_SUBSOUND_CANTMOVE =>         "Shared subsounds cannot be replaced or moved from their parent stream, such as when the parent stream is an FSB file.",
            fmod::FMOD_ERR_TAGNOTFOUND =>               "The specified tag could not be found or there are no tags.",
            fmod::FMOD_ERR_TOOMANYCHANNELS =>           "The sound created exceeds the allowable input channel count.  This can be increased using the 'maxinputchannels' parameter in System::setSoftwareFormat.",
            fmod::FMOD_ERR_TRUNCATED =>                 "The retrieved string is too long to fit in the supplied buffer and has been truncated.",
            fmod::FMOD_ERR_UNIMPLEMENTED =>             "Something in FMOD hasn't been implemented when it should be! contact support!",
            fmod::FMOD_ERR_UNINITIALIZED =>             "This command failed because System::init or System::setDriver was not called.",
            fmod::FMOD_ERR_UNSUPPORTED =>               "A command issued was not supported by this object.  Possibly a plugin without certain callbacks specified.",
            fmod::FMOD_ERR_VERSION =>                   "The version number of this file format is not supported.",
            fmod::FMOD_ERR_EVENT_ALREADY_LOADED =>      "The specified bank has already been loaded.",
            fmod::FMOD_ERR_EVENT_LIVEUPDATE_BUSY =>     "The live update connection failed due to the game already being connected.",
            fmod::FMOD_ERR_EVENT_LIVEUPDATE_MISMATCH => "The live update connection failed due to the game data being out of sync with the tool.",
            fmod::FMOD_ERR_EVENT_LIVEUPDATE_TIMEOUT =>  "The live update connection timed out.",
            fmod::FMOD_ERR_EVENT_NOTFOUND =>            "The requested event, parameter, bus or vca could not be found.",
            fmod::FMOD_ERR_STUDIO_UNINITIALIZED =>      "The Studio::System object is not yet initialized.",
            fmod::FMOD_ERR_STUDIO_NOT_LOADED =>         "The specified resource is not loaded, so it can't be unloaded.",
            fmod::FMOD_ERR_INVALID_STRING =>            "An invalid string was passed to this function.",
            fmod::FMOD_ERR_ALREADY_LOCKED =>            "The specified resource is already locked.",
            fmod::FMOD_ERR_NOT_LOCKED =>                "The specified resource is not locked, so it can't be unlocked.",
            fmod::FMOD_ERR_RECORD_DISCONNECTED =>       "The specified recording driver has been disconnected.",
            fmod::FMOD_ERR_TOOMANYSAMPLES =>            "The length provided exceeds the allowable limit.",
            _ => "Unknown error.",
        });
    }
}

pub fn get_echo_base() -> Result<usize> {
    Ok(
        unsafe {
            GetModuleHandleA(windows::core::s!("Echo_v1.0.dll")).map(|hmod| hmod.0 as usize)?
        },
    )
}

pub fn get_echo_macro_name() -> Result<String> {
    unsafe {
        let echo_base = get_echo_base()?;
        // 940 - macro_name offset
        let macro_name = echo_base + 0x150448 + 940; // char macro_name[1000];
        Ok(CString::from_raw(macro_name as *mut i8).into_string()?)
    }
}

pub fn get_echo_fps() -> f32 {
    unsafe {
        let echo_base = get_echo_base().unwrap();
        let fps = echo_base + 0x150448 + 940 - 4;
        *(fps as *const f32)
    }
}
