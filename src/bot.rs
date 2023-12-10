use crate::{hooks, utils};
use anyhow::Result;
use egui::{pos2, vec2, Align2, Color32, Direction, Key, KeyboardShortcut, Modifiers, RichText};
use egui_keybind::{Bind, Keybind, Shortcut};
use egui_modal::{Icon, Modal};
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};
use geometrydash::{
    fmod::{
        FMOD_System_CreateSound, FMOD_CREATESOUNDEXINFO, FMOD_DEFAULT, FMOD_SOUND,
        FMOD_SOUND_FORMAT_PCMFLOAT, FMOD_SYSTEM,
    },
    AddressUtils, FMODAudioEngine, PlayLayer, PlayerObject,
};
use kittyaudio::{Device, Mixer, PlaybackRate, Sound, SoundHandle, StreamSettings};
use once_cell::sync::Lazy;
use rand::{prelude::SliceRandom, Rng};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, Once},
    time::{Duration, Instant},
};
use windows::Win32::System::Console::{AllocConsole, FreeConsole};

/// Global bot state
pub static mut BOT: Lazy<Box<Bot>> = Lazy::new(|| Box::new(Bot::default()));

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Timings {
    pub hard: f32,
    pub regular: f32,
    pub soft: f32,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            hard: 2.0,
            regular: 0.15,
            soft: 0.025,
            // lower = microclicks
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Pitch {
    pub from: f64,
    pub to: f64,
    #[serde(default = "f64::default")]
    pub step: f64,
}

impl Default for Pitch {
    fn default() -> Self {
        Self {
            from: 0.95,
            to: 1.05,
            step: 0.001,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct VolumeSettings {
    pub enabled: bool,
    pub spam_time: f32,
    pub spam_vol_offset_factor: f32,
    pub max_spam_vol_offset: f32,
    pub change_releases_volume: bool,
    pub global_volume: f32,
    pub volume_var: f32,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            spam_time: 0.3,
            spam_vol_offset_factor: 0.9,
            max_spam_vol_offset: 0.3,
            change_releases_volume: false,
            global_volume: 1.0,
            volume_var: 0.2,
        }
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum ClickType {
    HardClick,
    HardRelease,
    Click,
    Release,
    SoftClick,
    SoftRelease,
    MicroClick,
    MicroRelease,
    #[default]
    None,
}

impl ClickType {
    pub fn from_time(push: bool, time: f32, timings: &Timings) -> Self {
        if time > timings.hard {
            if push {
                Self::HardClick
            } else {
                Self::HardRelease
            }
        } else if time > timings.regular {
            if push {
                Self::Click
            } else {
                Self::Release
            }
        } else if time > timings.soft {
            if push {
                Self::SoftClick
            } else {
                Self::SoftRelease
            }
        } else if push {
            Self::MicroClick
        } else {
            Self::MicroRelease
        }
    }

    #[rustfmt::skip]
    pub fn preferred(self) -> [Self; 8] {
        use ClickType::*;

        // this is perfect
        match self {
            HardClick =>    [HardClick,    Click,        SoftClick,   MicroClick  , HardRelease,  Release,      SoftRelease, MicroRelease],
            HardRelease =>  [HardRelease,  Release,      SoftRelease, MicroRelease, HardRelease,  Release,      SoftRelease, MicroRelease],
            Click =>        [Click,        HardClick,    SoftClick,   MicroClick  , Release,      HardRelease,  SoftRelease, MicroRelease],
            Release =>      [Release,      HardRelease,  SoftRelease, MicroRelease, Release,      HardRelease,  SoftRelease, MicroRelease],
            SoftClick =>    [SoftClick,    MicroClick,   Click,       HardClick   , SoftRelease,  MicroRelease, Release,     HardRelease ],
            SoftRelease =>  [SoftRelease,  MicroRelease, Release,     HardRelease , SoftRelease,  MicroRelease, Release,     HardRelease ],
            MicroClick =>   [MicroClick,   SoftClick,    Click,       HardClick   , MicroRelease, SoftRelease,  Release,     HardRelease ],
            MicroRelease => [MicroRelease, SoftRelease,  Release,     HardRelease , MicroRelease, SoftRelease,  Release,     HardRelease ],
            None =>         [None,         None,         None,        None        , None,         None,         None,        None        ],
        }
    }

    #[inline]
    pub const fn is_release(self) -> bool {
        matches!(
            self,
            ClickType::HardRelease
                | ClickType::Release
                | ClickType::SoftRelease
                | ClickType::MicroRelease
        )
    }
}

#[inline]
fn fmod_system() -> *mut FMOD_SYSTEM {
    FMODAudioEngine::shared().system()
}

#[derive(Clone)]
pub struct SoundWrapper {
    sound: Sound,
    fmod_sound: *mut FMOD_SOUND,
}

impl SoundWrapper {
    pub fn from_sound(sound: Sound) -> Self {
        use std::mem::size_of;
        let mut exinfo: FMOD_CREATESOUNDEXINFO = unsafe { std::mem::zeroed() };
        exinfo.cbsize = size_of::<FMOD_CREATESOUNDEXINFO>() as i32;
        exinfo.numchannels = 2;
        exinfo.format = FMOD_SOUND_FORMAT_PCMFLOAT;
        exinfo.defaultfrequency = sound.sample_rate() as i32;
        // 2 channels, f32 sound
        exinfo.length = sound.frames.len() as u32 * size_of::<f32>() as u32 * 2;

        let mut fmod_sound: *mut FMOD_SOUND = std::ptr::null_mut();
        unsafe {
            if FMOD_System_CreateSound(
                fmod_system(),
                sound.frames.as_ptr() as *const i8,
                FMOD_DEFAULT,
                &mut exinfo,
                &mut fmod_sound,
            ) != 0
            {
                log::error!("failed to create fmod sound!");
            }
        };

        Self { sound, fmod_sound }
    }
}

impl Deref for SoundWrapper {
    type Target = Sound;

    fn deref(&self) -> &Self::Target {
        &self.sound
    }
}

impl DerefMut for SoundWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sound
    }
}

#[derive(Default)]
pub struct Sounds {
    pub hardclicks: Vec<SoundWrapper>,
    pub hardreleases: Vec<SoundWrapper>,
    pub clicks: Vec<SoundWrapper>,
    pub releases: Vec<SoundWrapper>,
    pub softclicks: Vec<SoundWrapper>,
    pub softreleases: Vec<SoundWrapper>,
    pub microclicks: Vec<SoundWrapper>,
    pub microreleases: Vec<SoundWrapper>,
}

fn read_clicks_in_directory(dir: &Path) -> Vec<SoundWrapper> {
    let Ok(dir) = dir.read_dir() else {
        log::warn!("can't find directory {dir:?}, skipping");
        return vec![];
    };
    let mut sounds = vec![];
    for entry in dir {
        let path = entry.unwrap().path();
        if path.is_file() {
            let sound = Sound::from_path(path.clone()).map(|s| SoundWrapper::from_sound(s));
            if let Ok(sound) = sound {
                sounds.push(sound);
            } else if let Err(e) = sound {
                log::error!("failed to load '{path:?}': {e}");
            }
        }
    }
    sounds
}

pub fn find_noise_file(dir: &Path) -> Option<PathBuf> {
    let Ok(dir) = dir.read_dir() else {
        return None;
    };
    for entry in dir {
        let path = entry.unwrap().path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        // if it's a noise*, etc file we should try to load it
        if path.is_file()
            && (filename.starts_with("noise")
                || filename.starts_with("whitenoise")
                || filename.starts_with("pcnoise")
                || filename.starts_with("background"))
        {
            return Some(path);
        }
    }
    None
}

impl Sounds {
    pub fn from_path(path: &Path) -> Self {
        let mut sounds = Self::default();

        for (dir, clicks) in [
            ("hardclicks", &mut sounds.hardclicks),
            ("hardreleases", &mut sounds.hardreleases),
            ("clicks", &mut sounds.clicks),
            ("releases", &mut sounds.releases),
            ("softclicks", &mut sounds.softclicks),
            ("softreleases", &mut sounds.softreleases),
            ("microclicks", &mut sounds.microclicks),
            ("microreleases", &mut sounds.microreleases),
        ] {
            let mut pathbuf = path.to_path_buf();
            pathbuf.push(dir);
            clicks.extend(read_clicks_in_directory(&pathbuf));
        }

        if !sounds.has_sounds() {
            log::warn!("no sounds found, assuming there's no subdirectories");
            sounds.clicks = read_clicks_in_directory(path);
        }

        sounds
    }

    #[inline]
    pub fn num_sounds(&self) -> usize {
        [
            &self.hardclicks,
            &self.hardreleases,
            &self.clicks,
            &self.releases,
            &self.softclicks,
            &self.softreleases,
            &self.microclicks,
            &self.microreleases,
        ]
        .iter()
        .map(|c| c.len())
        .sum()
    }

    #[inline]
    pub fn has_sounds(&self) -> bool {
        self.num_sounds() > 0
    }

    pub fn random_sound(&self, typ: ClickType) -> Option<(SoundWrapper, ClickType)> {
        let thread_rng = &mut rand::thread_rng();
        for typ in typ.preferred() {
            let sound = match typ {
                ClickType::HardClick => self.hardclicks.choose(thread_rng),
                ClickType::HardRelease => self.hardreleases.choose(thread_rng),
                ClickType::Click => self.clicks.choose(thread_rng),
                ClickType::Release => self.releases.choose(thread_rng),
                ClickType::SoftClick => self.softclicks.choose(thread_rng),
                ClickType::SoftRelease => self.softreleases.choose(thread_rng),
                ClickType::MicroClick => self.microclicks.choose(thread_rng),
                ClickType::MicroRelease => self.microreleases.choose(thread_rng),
                _ => None,
            };
            if let Some(sound) = sound {
                return Some((sound.clone(), typ));
            }
        }
        None
    }

    pub fn extend_with(&mut self, other: &Self) {
        for (s, o) in [
            (&mut self.hardclicks, &other.hardclicks),
            (&mut self.hardreleases, &other.hardreleases),
            (&mut self.clicks, &other.clicks),
            (&mut self.releases, &other.releases),
            (&mut self.softclicks, &other.softclicks),
            (&mut self.softreleases, &other.softreleases),
            (&mut self.microclicks, &other.microclicks),
            (&mut self.microreleases, &other.microreleases),
        ] {
            s.extend_from_slice(o);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Shortcuts {
    pub toggle_menu: Shortcut,
    pub toggle_bot: Shortcut,
    pub toggle_noise: Shortcut,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle_menu: Shortcut::new(
                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Num1)),
                None,
            ),
            toggle_bot: Shortcut::new(
                Some(KeyboardShortcut::new(Modifiers::NONE, Key::Num2)),
                None,
            ),
            toggle_noise: Shortcut::NONE,
        }
    }
}

#[inline]
fn true_value() -> bool {
    true
}

#[inline]
fn default_buffer_size() -> u32 {
    512
}

#[inline]
fn f32_one() -> f32 {
    1.0
}

// clickpack, options, audio
#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Copy)]
pub enum Stage {
    #[default]
    Clickpack,
    Audio,
    Options,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub pitch_enabled: bool,
    pub pitch: Pitch,
    pub timings: Timings,
    pub volume_settings: VolumeSettings,
    #[serde(default = "Shortcuts::default")]
    pub shortcuts: Shortcuts,
    #[serde(default = "String::new", skip_serializing_if = "String::is_empty")]
    pub selected_device: String,
    #[serde(default = "true_value")]
    pub enabled: bool,
    #[serde(default = "bool::default")]
    pub hidden: bool,
    #[serde(default = "default_buffer_size")]
    pub buffer_size: u32,
    #[serde(default = "bool::default")]
    pub play_noise: bool,
    #[serde(default = "f32_one")]
    pub noise_volume: f32,
    #[serde(default = "bool::default")]
    pub use_alternate_hook: bool,
    #[serde(default = "bool::default")]
    pub show_console: bool,
    #[serde(default = "Stage::default")]
    pub stage: Stage,
    #[serde(default = "bool::default")]
    pub use_fmod: bool,
    #[serde(default = "bool::default")]
    pub use_playlayer_time: bool,
}

impl Config {
    pub fn fixup(mut self) -> Self {
        self.buffer_size = self.buffer_size.max(1);
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pitch_enabled: true,
            pitch: Pitch::default(),
            timings: Timings::default(),
            volume_settings: VolumeSettings::default(),
            shortcuts: Shortcuts::default(),
            selected_device: String::new(),
            enabled: true,
            hidden: false,
            buffer_size: default_buffer_size(),
            play_noise: false,
            noise_volume: 1.0,
            use_alternate_hook: false,
            show_console: false,
            stage: Stage::default(),
            use_fmod: false,
            use_playlayer_time: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut path = PathBuf::from(".zcb/");
        log::debug!("creating directory {path:?}");
        std::fs::create_dir_all(&path)?;
        path.push("config.json");

        // try to read config
        log::debug!("trying to read config at {path:?}");
        if let Ok(f) = std::fs::File::open(&path) {
            let config = serde_json::from_reader(f)
                .map_err(|e| log::error!("failed to deserialize config at {path:?}: {e}"));
            if let Ok(config) = config {
                log::debug!("successfully read config at {path:?}");
                return Ok(config);
            }
        }

        // failed to read config, write default config
        let config = Self::default();
        log::debug!("creating file {path:?}");
        let f = std::fs::File::create(&path)?;
        log::debug!("writing default config to {path:?}");
        serde_json::to_writer_pretty(f, &config)?;
        Ok(config)
    }

    pub fn save(&self) {
        let Ok(f) = std::fs::File::create(".zcb/config.json") else {
            log::error!("failed to create config.json!");
            return;
        };
        let _ = serde_json::to_writer_pretty(f, self)
            .map_err(|e| log::error!("failed to write config: {e}"))
            .map(|_| log::debug!("successfully saved config to \".zcb/config.json\""));
    }
}

pub struct Bot {
    pub conf: Config,
    pub players: (Sounds, Sounds),
    pub noise: Option<Sound>,
    pub mixer: Mixer,
    pub playlayer: PlayLayer,
    pub prev_time: f64,
    pub is_loading_clickpack: bool,
    pub num_sounds: (usize, usize),
    pub selected_clickpack: String,
    pub selected_device: String,
    pub devices: Arc<Mutex<Vec<String>>>,
    pub last_conf_save: Instant,
    pub prev_conf: Config,
    pub prev_click_type: ClickType,
    pub prev_resolved_click_type: ClickType,
    pub prev_pitch: f64,
    pub prev_volume: f32,
    pub prev_spam_offset: f32,
    pub buffer_size_changed: bool,
    pub noise_sound: Option<SoundHandle>,
    pub show_alternate_hook_warning: bool,
    pub did_reset_config: bool,
    pub clickpacks: Vec<PathBuf>,
    pub last_clickpack_reload: Instant,
    pub level_start: Instant,
    pub used_alternate_hook: bool,
}

impl Default for Bot {
    fn default() -> Self {
        let conf = Config::load().unwrap_or_default().fixup();
        let use_alternate_hook = conf.use_alternate_hook;
        Self {
            conf: conf.clone(),
            players: (Sounds::default(), Sounds::default()),
            noise: None,
            mixer: Mixer::new(),
            playlayer: PlayLayer::from_address(0),
            prev_time: 0.0,
            is_loading_clickpack: false,
            num_sounds: (0, 0),
            selected_clickpack: String::new(),
            selected_device: String::new(),
            devices: Arc::new(Mutex::new(vec![])),
            last_conf_save: Instant::now(),
            prev_conf: conf,
            prev_click_type: ClickType::None,
            prev_resolved_click_type: ClickType::None,
            prev_pitch: f64::NAN,
            prev_volume: f32::NAN,
            prev_spam_offset: f32::NAN,
            buffer_size_changed: false,
            noise_sound: None,
            show_alternate_hook_warning: false,
            did_reset_config: false,
            clickpacks: vec![],
            last_clickpack_reload: Instant::now(),
            level_start: Instant::now(),
            used_alternate_hook: use_alternate_hook,
        }
    }
}

const PLAYER_DIRNAMES: [(&str, &str); 7] = [
    ("player1", "player2"),
    ("player 1", "player 2"),
    ("sounds1", "sounds2"),
    ("sounds 1", "sounds 2"),
    ("p1", "p2"),
    ("1", "2"),
    ("", ""),
];

fn help_text<R>(ui: &mut egui::Ui, help: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) {
    if help.is_empty() {
        add_contents(ui); // don't show help icon if there's no help text
        return;
    }
    ui.horizontal(|ui| {
        add_contents(ui);
        ui.add_enabled_ui(false, |ui| ui.label("(?)").on_disabled_hover_text(help));
    });
}

/// Value is always min clamped with 1.
fn u32_edit_field_min1(ui: &mut egui::Ui, value: &mut u32) -> egui::Response {
    let mut tmp_value = format!("{value}");
    let res = ui.text_edit_singleline(&mut tmp_value);
    if let Ok(result) = tmp_value.parse::<u32>() {
        *value = result.max(1);
    }
    res
}

impl Bot {
    fn unload_clickpack(&mut self) {
        log::debug!("unloading clickpack");
        self.num_sounds = (0, 0);
        self.players = (Sounds::default(), Sounds::default());
        self.noise = None;
        if let Some(noise_sound) = self.noise_sound.take() {
            noise_sound.seek_to_end();
            noise_sound.set_loop_enabled(false);
            noise_sound.set_playback_rate(PlaybackRate::Factor(0.0));
        }
        self.selected_clickpack.clear();
    }

    pub fn load_clickpack(&mut self, clickpack_dir: &Path) -> Result<()> {
        // reset current clickpack
        self.unload_clickpack();

        for player_dirnames in PLAYER_DIRNAMES {
            let mut player1_path = clickpack_dir.to_path_buf();
            player1_path.push(player_dirnames.0);
            let mut player2_path = clickpack_dir.to_path_buf();
            player2_path.push(player_dirnames.1);

            // load for both players
            self.players
                .0
                .extend_with(&Sounds::from_path(&player1_path));
            self.load_noise(&player1_path);
            if !player_dirnames.1.is_empty() {
                self.players
                    .1
                    .extend_with(&Sounds::from_path(&player2_path));
                self.load_noise(&player2_path);
            }
        }

        self.load_noise(&clickpack_dir);

        anyhow::ensure!(self.has_sounds(), "no sounds found in clickpack");

        self.num_sounds = (self.players.0.num_sounds(), self.players.1.num_sounds());
        self.selected_clickpack = clickpack_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        log::info!(
            "loaded clickpack \"{}\", {} sounds",
            self.selected_clickpack,
            self.num_sounds.0 + self.num_sounds.1
        );
        log::info!("{} player 1 sounds", self.num_sounds.0);
        log::info!("{} player 2 sounds", self.num_sounds.1);
        log::info!("has noise: {}", self.noise.is_some());

        // start playing the new noise file if the one from the previous clickpack
        // was playing
        if self.conf.play_noise {
            self.play_noise(false);
        }

        Ok(())
    }

    fn load_noise(&mut self, dir: &Path) {
        let Some(path) = find_noise_file(dir) else {
            return;
        };
        // try to load noise
        self.noise = Sound::from_path(path).ok();
    }

    pub fn has_sounds(&self) -> bool {
        self.players.0.has_sounds() || self.players.1.has_sounds()
    }

    fn get_random_click(&self, typ: ClickType, player2: bool) -> (SoundWrapper, ClickType) {
        if player2 {
            self.players
                .1
                .random_sound(typ)
                .unwrap_or_else(|| self.players.0.random_sound(typ).unwrap())
        } else {
            self.players
                .0
                .random_sound(typ)
                .unwrap_or_else(|| self.players.1.random_sound(typ).unwrap())
        }
    }

    fn maybe_init_kittyaudio(&mut self) {
        if self.conf.use_fmod {
            return;
        }
        log::debug!("starting kittyaudio playback thread");
        self.mixer = Mixer::new();
        let device = self.get_device();

        self.mixer.init_ex(
            device,
            StreamSettings {
                buffer_size: Some(self.conf.buffer_size),
                ..Default::default()
            },
        );
    }

    pub fn init(&mut self) {
        // update thread
        #[cfg(feature = "special")]
        {
            let devices_arc = self.devices.clone();
            std::thread::spawn(move || {
                let mut prev_devices = vec![];
                loop {
                    if unsafe { BOT.conf.use_fmod } {
                        continue;
                    }
                    if let Ok(devices) = kittyaudio::device_names() {
                        // only lock when device lists do not match
                        if devices != prev_devices {
                            log::trace!("updated device list: {devices:?}");
                            *devices_arc.lock().unwrap() = devices.clone();
                            prev_devices = devices;
                        }
                    }
                    std::thread::sleep(Duration::from_secs(4));
                }
            });
        }

        // init audio playback
        self.maybe_init_kittyaudio();

        // reload clickpacks
        let _ = self
            .reload_clickpacks()
            .map_err(|e| log::error!("failed to reload clickpacks: {e}"));

        // init game hooks
        log::debug!("initializing hooks");
        unsafe { hooks::init_hooks() };
    }

    /// Return whether a given [PlayerObject] is player 2. If playlayer is null,
    /// always return false.
    #[inline]
    pub fn is_player2_obj(&self, player: PlayerObject) -> bool {
        !self.playlayer.is_null() && self.playlayer.player2() == player
    }

    fn get_pitch(&self) -> f64 {
        if self.conf.pitch_enabled {
            rand::thread_rng().gen_range(self.conf.pitch.from..=self.conf.pitch.to)
        } else {
            1.0
        }
    }

    pub fn on_action(&mut self, push: bool, player2: bool) {
        if self.num_sounds == (0, 0) || self.playlayer.is_null() {
            return;
        }

        #[cfg(not(feature = "special"))]
        if self.playlayer.is_dead() {
            return;
        }

        if (!self.playlayer.level_settings().is_2player() && player2)
            || self.playlayer.is_paused()
            || (!push && self.playlayer.time() == 0.0)
        {
            return;
        }

        let now = self.time();
        let dt = (now - self.prev_time).abs() as f32;
        let click_type = ClickType::from_time(push, dt, &self.conf.timings);

        // get click
        let (mut click, resolved_click_type) = self.get_random_click(click_type, player2);
        let pitch = self.get_pitch();
        click.set_playback_rate(PlaybackRate::Factor(pitch));

        // compute & change volume
        {
            let vol = &self.conf.volume_settings;
            let mut volume = 1.0;
            if vol.volume_var != 0.0 {
                volume += rand::thread_rng().gen_range(-vol.volume_var..=vol.volume_var);
            }

            // calculate spam volume change
            if vol.enabled
                && dt < vol.spam_time
                && !(!vol.change_releases_volume && resolved_click_type.is_release())
            {
                let offset = (vol.spam_time - dt) * vol.spam_vol_offset_factor;
                self.prev_spam_offset = offset;
                volume -= offset.min(vol.max_spam_vol_offset);
            } else {
                self.prev_spam_offset = 0.0;
            }

            // multiply by global volume after all of the changes
            volume *= vol.global_volume;

            click.set_volume(volume);
            self.prev_volume = volume;
        }

        self.mixer.play(click.sound);
        self.prev_time = now;
        self.prev_click_type = click_type;
        self.prev_resolved_click_type = resolved_click_type;
        self.prev_pitch = pitch;
    }

    pub fn oninit(&mut self) {
        self.prev_time = 0.0;
        self.prev_click_type = ClickType::None;
        self.prev_resolved_click_type = ClickType::None;
        self.prev_pitch = 0.0;
        self.prev_volume = self.conf.volume_settings.global_volume;
        self.prev_spam_offset = 0.0;
        self.level_start = Instant::now();
    }

    pub fn onreset(&mut self) {
        self.level_start = Instant::now();
    }

    #[inline]
    fn time(&self) -> f64 {
        if self.conf.use_playlayer_time {
            self.playlayer
                .to_option()
                .map_or_else(|| self.level_start.elapsed().as_secs_f64(), |p| p.time())
        } else {
            self.level_start.elapsed().as_secs_f64()
        }
    }

    fn open_clickbot_toggle_toast(&self, toasts: &mut Toasts) {
        toasts.add(Toast {
            kind: ToastKind::Info,
            text: if self.conf.enabled {
                "Enabled clickbot".into()
            } else {
                "Disabled clickbot".into()
            },
            options: ToastOptions::default().duration_in_seconds(2.0),
        });
    }

    fn reload_clickpacks(&mut self) -> Result<()> {
        let path = Path::new(".zcb/clickpacks");
        std::fs::create_dir_all(path)?;
        let dir = path.read_dir()?;
        self.clickpacks.clear();
        for entry in dir {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.clickpacks.push(path);
            }
        }
        Ok(())
    }

    pub fn draw_ui(&mut self, ctx: &egui::Context) {
        // process hotkeys
        let wants_keyboard = ctx.wants_keyboard_input();
        let (toggle_menu, toggle_bot, toggle_noise) = ctx.input_mut(|i| {
            // for some reason it deadlocks when i put `ctx.wants_keyboard_input()` here?
            if wants_keyboard {
                (false, false, false)
            } else {
                (
                    self.conf.shortcuts.toggle_menu.pressed(i),
                    self.conf.shortcuts.toggle_bot.pressed(i),
                    self.conf.shortcuts.toggle_noise.pressed(i),
                )
            }
        });
        if toggle_menu {
            self.conf.hidden = !self.conf.hidden;
        }
        if toggle_bot {
            self.conf.enabled = !self.conf.enabled;
        }
        if toggle_noise {
            self.conf.play_noise = !self.conf.play_noise;
            self.play_noise(false);
        }

        // auto-save config
        if self.last_conf_save.elapsed() > Duration::from_secs(3) {
            if self.conf != self.prev_conf && !self.did_reset_config {
                self.conf.save();
                self.last_conf_save = Instant::now();
                self.prev_conf = self.conf.clone();
            }
        }

        // don't draw/autosave if not open
        if self.conf.hidden {
            return;
        }

        // clickpack reloading
        if self.last_clickpack_reload.elapsed() > Duration::from_secs(3) {
            let _ = self
                .reload_clickpacks()
                .map_err(|e| log::error!("failed to reload clickpacks: {e}"));
            self.last_clickpack_reload = Instant::now();
        }

        // draw overlay
        let modal = Arc::new(Mutex::new(Modal::new(ctx, "global_modal")));
        let mut toasts = Toasts::new()
            .anchor(Align2::RIGHT_BOTTOM, pos2(-16.0, -16.0))
            .direction(Direction::BottomUp);

        if toggle_bot {
            self.open_clickbot_toggle_toast(&mut toasts);
        }
        if toggle_noise {
            self.open_noise_toggle_toast(&mut toasts);
        }

        egui::Window::new("ZCB Live").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.conf.stage, Stage::Clickpack, "Clickpack");
                ui.selectable_value(&mut self.conf.stage, Stage::Audio, "Audio");
                ui.selectable_value(&mut self.conf.stage, Stage::Options, "Options");
            });
            ui.separator();

            egui::ScrollArea::both().show(ui, |ui| {
                match self.conf.stage {
                    Stage::Clickpack => self.show_clickpack_window(ui, modal.clone()),
                    Stage::Audio => {
                        if ui
                            .checkbox(&mut self.conf.enabled, "Enable clickbot")
                            .changed()
                        {
                            self.open_clickbot_toggle_toast(&mut toasts);
                        }

                        // ui.separator();
                        ui.add_enabled_ui(self.conf.enabled, |ui| {
                            self.show_audio_window(ui, &mut toasts);
                        });
                    }
                    Stage::Options => self.show_options_window(ui, modal.clone(), &mut toasts),
                };
            });
        });

        toasts.show(ctx);
        modal.lock().unwrap().show_dialog();
    }

    pub fn maybe_alloc_console(&self) {
        if self.conf.show_console {
            unsafe { AllocConsole().unwrap() };
            static INIT_ONCE: Once = Once::new();
            INIT_ONCE.call_once(|| {
                simple_logger::SimpleLogger::new()
                    .init()
                    .expect("failed to initialize simple_logger");
            });
        }
    }

    fn show_options_window(
        &mut self,
        ui: &mut egui::Ui,
        modal: Arc<Mutex<Modal>>,
        toasts: &mut Toasts,
    ) {
        ui.collapsing("Shortcuts", |ui| {
            ui.add(
                Keybind::new(&mut self.conf.shortcuts.toggle_menu, "toggle_menu_keybind")
                    .with_text("Toggle menu"),
            );
            ui.add(
                Keybind::new(&mut self.conf.shortcuts.toggle_bot, "toggle_bot_keybind")
                    .with_text("Toggle bot"),
            );
            ui.add(
                Keybind::new(
                    &mut self.conf.shortcuts.toggle_noise,
                    "toggle_noise_keybind",
                )
                .with_text("Toggle noise"),
            );
        });
        ui.collapsing("Configuration", |ui| {
            ui.horizontal(|ui| {
                help_text(
                    ui,
                    "Use an alternate pushbutton/releasebutton hook for bot compatibility",
                    |ui| {
                        if ui
                            .checkbox(&mut self.conf.use_alternate_hook, "Use alternate hook")
                            .changed()
                        {
                            self.show_alternate_hook_warning = !self.show_alternate_hook_warning;
                            if self.show_alternate_hook_warning {
                                toasts.add(Toast {
                                    kind: ToastKind::Info,
                                    text: "Changing this option requires a game restart!".into(),
                                    options: ToastOptions::default().duration_in_seconds(2.0),
                                });
                            }
                        }
                    },
                );
                if self.show_alternate_hook_warning {
                    ui.label(RichText::new("Requires restart!").color(Color32::YELLOW));
                }
            });
            help_text(ui, "Show debug console", |ui| {
                if ui
                    .checkbox(&mut self.conf.show_console, "Show console")
                    .changed()
                {
                    if self.conf.show_console {
                        self.maybe_alloc_console();
                    } else {
                        let _ = unsafe { FreeConsole() };
                    }
                }
            });
            help_text(
                ui,
                "Synchronize actions with the timestep of the game",
                |ui| ui.checkbox(&mut self.conf.use_playlayer_time, "Use PlayLayer time"),
            );

            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 4.0;
                if ui
                    .button("Save")
                    .on_hover_text(
                        "Save the current configuration.\n\
                        This happens automatically, unless you reset your config!",
                    )
                    .clicked()
                {
                    self.conf.save();
                    self.did_reset_config = false;
                    self.prev_conf = self.conf.clone();
                    toasts.add(Toast {
                        kind: ToastKind::Success,
                        text: "Saved configuration to .zcb/config.json".into(),
                        options: ToastOptions::default().duration_in_seconds(2.0),
                    });
                }
                if self.conf != self.prev_conf {
                    ui.style_mut().spacing.item_spacing.x = 4.0;
                    ui.label("(!)").on_hover_text("Unsaved changes");
                }
                ui.style_mut().spacing.item_spacing.x = 4.0;
                if ui
                    .button("Load")
                    .on_hover_text("Load the config from .zcb/config.json")
                    .clicked()
                {
                    let conf = Config::load();
                    if let Ok(conf) = conf {
                        let prev_bufsize = self.conf.buffer_size;
                        self.conf = conf;
                        self.apply_config(prev_bufsize);
                        toasts.add(Toast {
                            kind: ToastKind::Success,
                            text: "Loaded configuration from .zcb/config.json".into(),
                            options: ToastOptions::default().duration_in_seconds(2.0),
                        });
                    } else if let Err(e) = conf {
                        modal
                            .lock()
                            .unwrap()
                            .dialog()
                            .with_title("Failed to load config!")
                            .with_body(utils::capitalize_first_letter(&e.to_string()))
                            .with_icon(Icon::Error)
                            .open();
                    }
                }
                ui.style_mut().spacing.item_spacing.x = 4.0;
                if ui
                    .button("Reset")
                    .on_hover_text("Reset the current configuration to defaults")
                    .clicked()
                {
                    let prev_bufsize = self.conf.buffer_size;
                    let prev_stage = self.conf.stage;
                    self.conf = Config::default();
                    self.conf.stage = prev_stage; // don't switch current tab
                    self.did_reset_config = true;
                    self.apply_config(prev_bufsize);
                    toasts.add(Toast {
                        kind: ToastKind::Info,
                        text: "Reset configuration to defaults".into(),
                        options: ToastOptions::default().duration_in_seconds(2.0),
                    });
                }
                if ui
                    .button("Open folder")
                    .on_hover_text("Open .zcb folder in explorer")
                    .clicked()
                {
                    Command::new("explorer").arg(".zcb").spawn().unwrap();
                }
            });
            ui.label(format!(
                "Last saved {:.2?}s ago",
                self.last_conf_save.elapsed().as_secs_f32()
            ));
        });
        ui.allocate_space(ui.available_size() - vec2(0.0, 280.0));
    }

    fn get_device(&mut self) -> Device {
        if !self.conf.selected_device.is_empty() {
            self.selected_device = self.conf.selected_device.clone();
            Device::from_name(&self.conf.selected_device).unwrap_or_default()
        } else {
            log::debug!("using default device");
            self.selected_device = Device::Default.name().unwrap_or_default();
            Device::Default
        }
    }

    fn play_noise(&mut self, new_mixer: bool) {
        if let Some(mut noise) = self.noise.clone() {
            if new_mixer || self.noise_sound.is_none() {
                // the mixer was recreated or noise has never started
                if !self.conf.play_noise {
                    noise.set_playback_rate(PlaybackRate::Factor(0.0));
                }

                // update noise speed and play the sound
                noise.set_volume(self.conf.noise_volume);
                noise.set_loop_enabled(true);
                noise.set_loop_index(0..=noise.frames.len().saturating_sub(1));
                self.noise_sound = Some(self.mixer.play(noise));
            } else if let Some(noise_sound) = &self.noise_sound {
                // noise is already playing, mixer has not been recreated
                noise.set_volume(self.conf.noise_volume);
                noise_sound.set_playback_rate(PlaybackRate::Factor(if self.conf.play_noise {
                    1.0
                } else {
                    0.0
                }));
            }
        }
    }

    fn open_noise_toggle_toast(&self, toasts: &mut Toasts) {
        toasts.add(Toast {
            kind: ToastKind::Info,
            text: if self.conf.play_noise {
                "Playing noise".into()
            } else {
                "Stopped playing noise".into()
            },
            options: ToastOptions::default().duration_in_seconds(2.0),
        });
    }

    #[cfg(feature = "special")]
    fn show_device_switcher(&mut self, ui: &mut egui::Ui, toasts: &mut Toasts) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Output device")
                .selected_text(&self.selected_device)
                .show_ui(ui, |ui| {
                    let devices = self.devices.lock().unwrap().clone();
                    for device in &devices {
                        let is_selected = &self.selected_device == device;
                        if ui
                            .selectable_value(&mut self.selected_device, device.clone(), device)
                            .clicked()
                            && !is_selected
                        {
                            // start a new mixer on new device
                            log::info!("switching audio device to \"{device}\"");
                            self.maybe_init_kittyaudio();
                            self.play_noise(true);
                            toasts.add(Toast {
                                kind: ToastKind::Success,
                                text: format!("Switched device to \"{device}\"").into(),
                                options: ToastOptions::default().duration_in_seconds(3.0),
                            });
                        }
                    }
                })
                .response
                .on_disabled_hover_text("Not available with FMOD");
            if ui
                .button("Reset")
                .on_hover_text("Reset to the default audio device")
                .clicked()
            {
                self.mixer = Mixer::new();
                self.mixer.init();
                if let Ok(name) = Device::Default.name() {
                    self.selected_device = name.clone();
                    toasts.add(Toast {
                        kind: ToastKind::Success,
                        text: format!("Switched device to \"{name}\"").into(),
                        options: ToastOptions::default().duration_in_seconds(3.0),
                    });
                }
                self.play_noise(true);
                log::debug!("reset audio device");
            }
        });
    }

    fn show_audio_window(&mut self, ui: &mut egui::Ui, toasts: &mut Toasts) {
        ui.add_enabled_ui(self.noise.is_some() && !self.is_loading_clickpack, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.conf.play_noise, "Play noise")
                    .on_disabled_hover_text("Your clickpack doesn't have a noise file")
                    .on_hover_text("Play the noise file")
                    .changed()
                {
                    self.play_noise(false);
                    self.open_noise_toggle_toast(toasts);
                }

                if ui
                    .add(
                        egui::DragValue::new(&mut self.conf.noise_volume)
                            .speed(0.01)
                            .clamp_range(0.0..=f32::INFINITY),
                    )
                    .changed()
                {
                    if let Some(noise_sound) = &self.noise_sound {
                        noise_sound.set_volume(self.conf.noise_volume);
                    }
                }
                ui.label("Noise volume");
            });
        });

        help_text(
            ui,
            "Use the internal audio engine for integration with internal recorders",
            |ui| ui.checkbox(&mut self.conf.use_fmod, "Use FMOD"),
        );

        #[cfg(feature = "special")]
        ui.add_enabled_ui(!self.conf.use_fmod, |ui| {
            self.show_device_switcher(ui, toasts)
        });

        ui.separator();

        ui.collapsing("Timings", |ui| {
            let timings_copy = self.conf.timings.clone();
            let timings = &mut self.conf.timings;
            let fields = [
                (
                    "Anything above this time between clicks plays hardclicks/hardreleases",
                    &mut timings.hard,
                    timings_copy.regular..=f32::INFINITY,
                    "Hard timing",
                ),
                (
                    "Anything above this time between clicks plays clicks/releases",
                    &mut timings.regular,
                    timings_copy.soft..=timings_copy.hard,
                    "Regular timing",
                ),
                (
                    "Anything above this time between clicks plays softclicks/softreleases",
                    &mut timings.soft,
                    0.0..=timings_copy.regular,
                    "Soft timing",
                ),
            ];
            for field in fields {
                help_text(ui, field.0, |ui| {
                    ui.horizontal(|ui| {
                        let dragged = ui
                            .add(
                                egui::DragValue::new(field.1)
                                    .clamp_range(field.2.clone())
                                    .speed(0.01),
                            )
                            .dragged();
                        let mut text = RichText::new(field.3);
                        if dragged && (*field.1 == *field.2.start() || *field.1 == *field.2.end()) {
                            text = text.color(Color32::LIGHT_RED);
                        }
                        ui.label(text);
                    });
                });
            }
            ui.label(format!(
                "Any value smaller than {:.2?} plays microclicks/microreleases",
                Duration::from_secs_f32(timings.soft),
            ))
        });

        ui.collapsing("Pitch variation", |ui| {
            ui.label(
                "Pitch variation can make clicks sound more realistic by \
                    changing their pitch randomly.",
            );
            ui.checkbox(&mut self.conf.pitch_enabled, "Enable pitch variation");
            ui.add_enabled_ui(self.conf.pitch_enabled, |ui| {
                let p = &mut self.conf.pitch;
                help_text(ui, "Minimum pitch value. 1.0 means no change", |ui| {
                    ui.horizontal(|ui| {
                        let dragged = ui
                            .add(
                                egui::DragValue::new(&mut p.from)
                                    .clamp_range(0.0..=p.to)
                                    .speed(0.01),
                            )
                            .dragged();
                        let mut text = RichText::new("Minimum pitch");
                        if dragged && (p.from == 0.0 || p.from == p.to) {
                            text = text.color(Color32::LIGHT_RED);
                        }
                        ui.label(text);
                    });
                });
                help_text(ui, "Maximum pitch value. 1.0 means no change", |ui| {
                    ui.horizontal(|ui| {
                        let dragged = ui
                            .add(
                                egui::DragValue::new(&mut p.to)
                                    .clamp_range(p.from..=f64::INFINITY)
                                    .speed(0.01),
                            )
                            .dragged();
                        let mut text = RichText::new("Maximum pitch");
                        if dragged && p.to == p.from {
                            text = text.color(Color32::LIGHT_RED);
                        }
                        ui.label(text);
                    });
                });
            });
        });

        ui.collapsing("Volume settings", |ui| {
            let vol = &mut self.conf.volume_settings;
            let fields = [
                (
                    "Constant volume multiplier for all sounds",
                    &mut vol.global_volume,
                    "Global volume",
                ),
                (
                    "Random volume variation (+/-)",
                    &mut vol.volume_var,
                    "Volume variation",
                ),
            ];
            for field in fields {
                help_text(ui, field.0, |ui| {
                    ui.horizontal(|ui| {
                        let dragged = ui
                            .add(
                                egui::DragValue::new(field.1)
                                    .clamp_range(0.0..=f64::INFINITY)
                                    .speed(0.01),
                            )
                            .dragged();
                        let mut text = RichText::new(field.2);
                        if dragged && *field.1 == 0.0 {
                            text = text.color(Color32::LIGHT_RED);
                        }
                        ui.label(text);
                    });
                });
            }
        });

        ui.collapsing("Spam volume changes", |ui| {
            ui.label("This can be used to lower volume in spams");
            let vol = &mut self.conf.volume_settings;
            let fields = [
                (
                    "Time between clicks which are considered spam clicks",
                    &mut vol.spam_time,
                    "Spam time",
                    true,
                ),
                (
                    "The value which the volume offset factor is multiplied by",
                    &mut vol.spam_vol_offset_factor,
                    "Spam volume offset factor",
                    false,
                ),
                (
                    "The maximum value of the volume offset",
                    &mut vol.max_spam_vol_offset,
                    "Maximum volume offset",
                    false,
                ),
            ];
            for field in fields {
                help_text(ui, field.0, |ui| {
                    ui.horizontal(|ui| {
                        let dragged = ui
                            .add(
                                egui::DragValue::new(field.1)
                                    .clamp_range(if field.3 {
                                        0.0..=f64::INFINITY
                                    } else {
                                        f64::NEG_INFINITY..=f64::INFINITY
                                    })
                                    .speed(0.01),
                            )
                            .dragged();
                        let mut text = RichText::new(field.2);
                        if dragged && *field.1 == 0.0 {
                            text = text.color(Color32::LIGHT_RED);
                        }
                        ui.label(text);
                    });
                });
            }
        });

        ui.collapsing("Advanced", |ui| {
            let prev_bufsize = self.conf.buffer_size;
            help_text(
                ui,
                "Audio buffer size in samples.\nLower value means lower latency",
                |ui| {
                    ui.label("Buffer size");
                },
            );
            if u32_edit_field_min1(ui, &mut self.conf.buffer_size).changed() {
                self.buffer_size_changed = prev_bufsize != self.conf.buffer_size;
            }

            if self.buffer_size_changed {
                ui.horizontal(|ui| {
                    if ui
                        .button("Apply")
                        .on_hover_text("Apply buffer size changes")
                        .clicked()
                    {
                        self.maybe_init_kittyaudio();
                        self.buffer_size_changed = false;
                        self.play_noise(true);
                    }

                    if self.conf.buffer_size > 300_000 {
                        ui.label(
                            RichText::new("WARN: Using a high buffer size might cause instability")
                                .color(Color32::YELLOW),
                        );
                    }
                });
            }
        });

        ui.allocate_space(vec2(100.0, 0.0));
    }

    fn apply_config(&mut self, prev_bufsize: u32) {
        if prev_bufsize != self.conf.buffer_size {
            self.maybe_init_kittyaudio();
        }
    }

    fn load_clickpack_thread(modal: Arc<Mutex<Modal>>, dir: &Path) {
        unsafe { BOT.is_loading_clickpack = true };
        if let Err(e) = unsafe { BOT.load_clickpack(&dir) } {
            log::error!("failed to load clickpack: {e}");
            modal
                .lock()
                .unwrap()
                .dialog()
                .with_title("Failed to load clickpack!")
                .with_body(utils::capitalize_first_letter(&e.to_string()))
                .with_icon(Icon::Error)
                .open();
        }
        unsafe { BOT.is_loading_clickpack = false };
    }

    fn select_clickpack_combobox(&mut self, ui: &mut egui::Ui, modal: Arc<Mutex<Modal>>) {
        egui::ComboBox::from_label("Select clickpack")
            .selected_text(&self.selected_clickpack)
            .show_ui(ui, |ui| {
                for path in &self.clickpacks {
                    let dirname = path.file_name().unwrap().to_str().unwrap();
                    if ui
                        .selectable_label(self.selected_clickpack == dirname, dirname)
                        .clicked()
                    {
                        let modal_moved = modal.clone();
                        let path = path.clone();
                        std::thread::spawn(move || {
                            Self::load_clickpack_thread(modal_moved, &path);
                        });
                    }
                }
            });
    }

    fn select_clickpack_button(&mut self, ui: &mut egui::Ui, modal: Arc<Mutex<Modal>>) {
        if !self.clickpacks.is_empty() {
            self.select_clickpack_combobox(ui, modal);
            return;
        }
        ui.horizontal(|ui| {
            if ui
                .button("Select clickpack")
                .on_disabled_hover_text("Please wait...")
                .clicked()
            {
                std::thread::spawn(move || {
                    let Some(dir) = FileDialog::new().pick_folder() else {
                        return;
                    };
                    log::debug!("selected clickpack {:?}", dir);
                    Self::load_clickpack_thread(modal, &dir);
                });
            }
            if self.num_sounds != (0, 0) {
                ui.label(format!(
                    "Selected clickpack: \"{}\"",
                    self.selected_clickpack
                ));
            } else {
                ui.label("...or put clickpacks in .zcb/clickpacks");
            }
        });
    }

    fn show_clickpack_window(&mut self, ui: &mut egui::Ui, modal: Arc<Mutex<Modal>>) {
        if self.is_loading_clickpack {
            ui.horizontal(|ui| {
                ui.label("Loading clickpack...");
                ui.add(egui::Spinner::new());
            });
        }
        let has_sounds = self.num_sounds != (0, 0);

        ui.add_enabled_ui(!self.is_loading_clickpack, |ui| {
            if !self.clickpacks.is_empty() {
                help_text(
                    ui,
                    "If there's no folders inside .zcb/clickpacks,\n\
                    there will be an option to choose the clickpack manually",
                    |ui| {
                        ui.label("Put clickpacks in .zcb/clickpacks");
                    },
                );
            }
            ui.horizontal(|ui| {
                self.select_clickpack_button(ui, modal);
                if !self.selected_clickpack.is_empty() {
                    if ui.button("🗙").on_hover_text("Unload clickpack").clicked() {
                        self.unload_clickpack();
                    }
                }
            });
        });

        if has_sounds {
            help_text(
                ui,
                if self.num_sounds.1 == 0 {
                    "To add player 2 sounds, make a folder called \"player2\"\n\
                    and put sounds for the second player there"
                } else {
                    "" // will not be shown
                },
                |ui| {
                    ui.label(format!(
                        "{} player 1 sounds, {} player 2 sounds ({} in total)",
                        self.num_sounds.0,
                        self.num_sounds.1,
                        self.num_sounds.0 + self.num_sounds.1
                    ));
                },
            );
        }

        if !self.is_loading_clickpack && has_sounds && !self.playlayer.is_null() {
            ui.separator();
            ui.collapsing("Debug", |ui| {
                let dur = Duration::from_secs_f64(self.prev_time);
                let ago = self.time() - dur.as_secs_f64();
                help_text(ui, &format!("{dur:?} since the start of the level"), |ui| {
                    ui.label(format!("Last action time: {dur:.2?} ({ago:.2}s ago)"));
                });
                if self.prev_resolved_click_type != ClickType::None {
                    ui.label(format!(
                        "Last click type: {:?} (resolved to {:?})",
                        self.prev_click_type, self.prev_resolved_click_type
                    ));
                } else {
                    ui.label(format!("Last click type: {:?}", self.prev_click_type));
                }
                ui.label(format!(
                    "Last pitch: {:.4} ({} => {})",
                    self.prev_pitch, self.conf.pitch.from, self.conf.pitch.to
                ));
                ui.label(format!(
                    "Last volume: {:.4} (+/- {} * {})",
                    self.prev_volume,
                    self.conf.volume_settings.volume_var,
                    self.conf.volume_settings.global_volume
                ));
                ui.label(format!(
                    "Last spam volume offset: {:.4}",
                    self.prev_spam_offset
                ));
            });
        }
    }
}
