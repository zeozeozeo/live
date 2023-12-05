use crate::hooks;
use anyhow::Result;
use geometrydash::{AddressUtils, PlayLayer, PlayerObject};
use kittyaudio::{Mixer, Sound};
use once_cell::sync::Lazy;
use rand::prelude::SliceRandom;
use std::path::{Path, PathBuf};

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
    pub fn has_sounds(&self) -> bool {
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
        .any(|c| !c.is_empty())
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
}

impl Default for Bot {
    fn default() -> Self {
        Self {
            players: (Sounds::default(), Sounds::default()),
            noise: None,
            mixer: Mixer::new(),
            playlayer: PlayLayer::from_address(0),
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

impl Bot {
    pub fn load_clickpack(&mut self, clickpack_dir: &Path) -> Result<()> {
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

    pub fn run(&mut self) {
        // init audio playback
        self.mixer.init();

        // init game hooks
        unsafe { hooks::init_hooks() };

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
        let click = self.get_random_click(
            if push {
                ClickType::Click
            } else {
                ClickType::Release
            },
            player2,
        );

        self.mixer.play(click);
    }
}
