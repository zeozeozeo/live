use crate::hooks;
use anyhow::Result;
use egui_modal::{Icon, Modal};
use geometrydash::{AddressUtils, PlayLayer, PlayerObject};
use kittyaudio::{Mixer, Sound};
use once_cell::sync::Lazy;
use rand::prelude::SliceRandom;
use rfd::FileDialog;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Global bot state
pub static mut BOT: Lazy<Box<Bot>> = Lazy::new(|| Box::new(Bot::default()));

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
        match typ {
            ClickType::HardClick => self.hardclicks.choose(thread_rng),
            ClickType::HardRelease => self.hardreleases.choose(thread_rng),
            ClickType::Click => self.clicks.choose(thread_rng),
            ClickType::Release => self.releases.choose(thread_rng),
            ClickType::SoftClick => self.softclicks.choose(thread_rng),
            ClickType::SoftRelease => self.softreleases.choose(thread_rng),
            ClickType::MicroClick => self.microclicks.choose(thread_rng),
            ClickType::MicroRelease => self.microreleases.choose(thread_rng),
            _ => None,
        }
        .cloned()
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

pub struct Bot {
    pub players: (Sounds, Sounds),
    pub noise: Option<Sound>,
    pub mixer: Mixer,
    pub playlayer: PlayLayer,
    pub prev_time: f64,
    pub timings: Timings,
    pub is_loading_clickpack: bool,
    pub num_sounds: (usize, usize),
    pub selected_clickpack: String,
}

impl Default for Bot {
    fn default() -> Self {
        Self {
            players: (Sounds::default(), Sounds::default()),
            noise: None,
            mixer: Mixer::new(),
            playlayer: PlayLayer::from_address(0),
            prev_time: 0.0,
            timings: Timings::default(),
            is_loading_clickpack: false,
            num_sounds: (0, 0),
            selected_clickpack: String::new(),
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
        // init audio playback
        self.mixer.init();

        // init game hooks
        unsafe { hooks::init_hooks() };
    }

    pub fn run(&mut self) {
        // run until end of time
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // unsafe { hooks::disable_hooks() };
    }

    /// Return whether a given [PlayerObject] is player 2. If playlayer is null,
    /// always return false.
    #[inline]
    pub fn is_player2_obj(&self, player: PlayerObject) -> bool {
        !self.playlayer.is_null() && self.playlayer.player2() == player
    }

    pub fn on_action(&mut self, push: bool, player2: bool) {
        return;
        let now = self.playlayer.time();
        let click_type = ClickType::from_time(push, (now - self.prev_time) as f32, &self.timings);
        let click = self.get_random_click(click_type, player2);

        self.mixer.play(click);
        self.prev_time = now;
    }

    pub fn draw_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Clickpack").show(ctx, |ui| {
            let modal = Modal::new(ctx, "clickpack_modal");
            let modal = Arc::new(Mutex::new(modal));
            self.show_clickpack_window(ui, modal);
        });
    }

    fn show_clickpack_window(&mut self, ui: &mut egui::Ui, modal: Arc<Mutex<Modal>>) {
        if self.is_loading_clickpack {
            ui.label("Loading clickpack...");
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
                        log::info!("selected clickpack {:?}", dir);

                        // load clickpack on this thread
                        unsafe { BOT.is_loading_clickpack = true };
                        if let Err(e) = unsafe { BOT.load_clickpack(&dir) } {
                            log::error!("failed to load clickpack: {e}");
                            modal
                                .lock()
                                .unwrap()
                                .dialog()
                                .with_title("Failed to load clickpack!")
                                .with_body(e)
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
                    "To add player 2 sounds, make a folder called \"player2\" and put\n\
                    sounds for the second player there"
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
        ui.separator();
    }
}
