use crate::{hooks, utils};
use anyhow::Result;
use egui_modal::{Icon, Modal};
use geometrydash::{AddressUtils, PlayLayer, PlayerObject};
use kittyaudio::{Device, Mixer, PlaybackRate, Sound, StreamSettings};
use once_cell::sync::Lazy;
use rand::{prelude::SliceRandom, Rng};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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

#[derive(Default, Clone, Copy)]
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
}

#[derive(Default)]
pub struct Sounds {
    pub hardclicks: Vec<Sound>,
    pub hardreleases: Vec<Sound>,
    pub clicks: Vec<Sound>,
    pub releases: Vec<Sound>,
    pub softclicks: Vec<Sound>,
    pub softreleases: Vec<Sound>,
    pub microclicks: Vec<Sound>,
    pub microreleases: Vec<Sound>,
}

fn read_clicks_in_directory(dir: &Path) -> Vec<Sound> {
    let Ok(dir) = dir.read_dir() else {
        log::warn!("can't find directory {dir:?}, skipping");
        return vec![];
    };
    let mut sounds = vec![];
    for entry in dir {
        let path = entry.unwrap().path();
        if path.is_file() {
            let sound = Sound::from_path(path.clone());
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

    pub fn random_sound(&self, typ: ClickType) -> Option<Sound> {
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
                return Some(sound.clone());
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

fn true_value() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub pitch_enabled: bool,
    pub pitch: Pitch,
    pub timings: Timings,
    #[serde(default = "String::new", skip_serializing_if = "String::is_empty")]
    pub selected_device: String,
    #[serde(default = "true_value")]
    pub enabled: bool,
    #[serde(default = "bool::default")]
    pub hidden: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pitch_enabled: true,
            pitch: Pitch::default(),
            timings: Timings::default(),
            selected_device: String::new(),
            enabled: true,
            hidden: false,
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
}

impl Default for Bot {
    fn default() -> Self {
        let conf = Config::load().unwrap_or_default();
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

impl Bot {
    pub fn load_clickpack(&mut self, clickpack_dir: &Path) -> Result<()> {
        // reset current clickpack
        self.num_sounds = (0, 0);
        self.players = (Sounds::default(), Sounds::default());

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

    fn get_random_click(&self, typ: ClickType, player2: bool) -> Sound {
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

    pub fn init(&mut self) {
        // if the config specifies a custom device, try to find it
        let device = if !self.conf.selected_device.is_empty() {
            self.selected_device = self.conf.selected_device.clone();
            Device::from_name(&self.conf.selected_device).unwrap_or_default()
        } else {
            log::debug!("using default device");
            self.selected_device = Device::Default.name().unwrap_or_default();
            Device::Default
        };

        // update thread
        #[cfg(feature = "special")]
        {
            let devices_arc = self.devices.clone();
            std::thread::spawn(move || {
                let mut prev_devices = vec![];
                loop {
                    if let Ok(devices) = kittyaudio::device_names() {
                        // only lock when device lists do not match
                        if devices != prev_devices {
                            log::trace!("updated device list: {devices:?}");
                            *devices_arc.lock().unwrap() = devices.clone();
                            prev_devices = devices;
                        }
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            });
        }

        // init audio playback
        log::debug!("starting kittyaudio playback thread");
        self.mixer.init_ex(device, StreamSettings::default());

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
            0.0
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

        let now = self.playlayer.time();
        let click_type =
            ClickType::from_time(push, (now - self.prev_time) as f32, &self.conf.timings);

        // get click
        let mut click = self.get_random_click(click_type, player2);
        click.set_playback_rate(PlaybackRate::Factor(self.get_pitch()));

        self.mixer.play(click);
        self.prev_time = now;
    }

    pub fn draw_ui(&mut self, ctx: &egui::Context) {
        if self.conf.hidden {
            return;
        }

        // auto-save config
        if self.last_conf_save.elapsed() > Duration::from_secs(3) && self.conf != self.prev_conf {
            self.conf.save();
            self.last_conf_save = Instant::now();
            self.prev_conf = self.conf.clone();
        }

        egui::Window::new("Clickpack").show(ctx, |ui| {
            let modal = Arc::new(Mutex::new(Modal::new(ctx, "clickpack_modal")));
            self.show_clickpack_window(ui, modal.clone());
            modal.lock().unwrap().show_dialog();
        });
        egui::Window::new("Audio").show(ctx, |ui| {
            ui.checkbox(&mut self.conf.enabled, "Enable clickbot");
            ui.separator();
            ui.add_enabled_ui(self.conf.enabled, |ui| {
                self.show_audio_window(ui);
            });
        });
        egui::Window::new("Options").show(ctx, |ui| self.show_options_window(ui));
    }

    fn show_options_window(&mut self, ui: &mut egui::Ui) {
        if ui
            .button("Close")
            .on_hover_text("Close this overlay")
            .clicked()
        {
            self.conf.hidden = true;
        }
    }

    fn show_audio_window(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "special")]
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
                            self.mixer = Mixer::new();
                            self.mixer
                                .init_ex(Device::Name(device.clone()), StreamSettings::default());
                        }
                    }
                });
            if ui
                .button("Reset")
                .on_hover_text("Reset to the default audio device")
                .clicked()
            {
                self.mixer = Mixer::new();
                self.mixer.init();
                if let Ok(name) = Device::Default.name() {
                    self.selected_device = name;
                }
                log::debug!("reset audio device");
            }
        });

        #[cfg(feature = "special")]
        ui.separator();

        ui.collapsing("Pitch variation", |ui| {
            ui.label(
                "Pitch variation can make clicks sound more realistic by \
                    changing their pitch randomly.",
            );
            ui.checkbox(&mut self.conf.pitch_enabled, "Enable pitch variation");
            ui.add_enabled_ui(self.conf.pitch_enabled, |ui| {
                let p = &mut self.conf.pitch;
                help_text(ui, "Minimum pitch value. 1.0 means no change", |ui| {
                    ui.add(egui::Slider::new(&mut p.from, 0.0..=p.to).text("Minimum pitch"));
                });
                help_text(ui, "Maximum pitch value. 1.0 means no change", |ui| {
                    ui.add(egui::Slider::new(&mut p.to, p.from..=50.0).text("Maxiumum pitch"));
                });
            });
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

                        // load clickpack on this thread
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
                    });
                }
                if has_sounds {
                    ui.label(format!(
                        "Selected clickpack: \"{}\"",
                        self.selected_clickpack
                    ));
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
    }
}
