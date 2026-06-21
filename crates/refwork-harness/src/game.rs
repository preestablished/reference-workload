#![forbid(unsafe_code)]

use std::fmt;

use refwork_emu::{Cartridge, CoreError};

pub const DEFAULT_MAPPER: &str = "lorom";

pub struct LoadedGame {
    pub cart: Cartridge,
    pub cart_hash: [u8; 32],
    pub mapper: String,
    pub sram_size: u32,
}

pub trait GameLoader {
    fn load_game(&mut self, dev_path: &str) -> Result<LoadedGame, GameLoadError>;
}

pub struct FilesystemGameLoader;

impl GameLoader for FilesystemGameLoader {
    fn load_game(&mut self, dev_path: &str) -> Result<LoadedGame, GameLoadError> {
        let rom = std::fs::read(dev_path).map_err(|source| GameLoadError::Read {
            path: dev_path.into(),
            source,
        })?;
        loaded_game_from_rom(rom)
    }
}

#[derive(Debug)]
pub enum GameLoadError {
    Read {
        path: String,
        source: std::io::Error,
    },
    Cart(CoreError),
}

impl fmt::Display for GameLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameLoadError::Read { path, source } => {
                write!(f, "cannot read game path `{path}`: {source}")
            }
            GameLoadError::Cart(err) => write!(f, "invalid game image: {err:?}"),
        }
    }
}

impl std::error::Error for GameLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GameLoadError::Read { source, .. } => Some(source),
            GameLoadError::Cart(_) => None,
        }
    }
}

impl From<CoreError> for GameLoadError {
    fn from(err: CoreError) -> Self {
        GameLoadError::Cart(err)
    }
}

pub fn loaded_game_from_rom(rom: Vec<u8>) -> Result<LoadedGame, GameLoadError> {
    let cart_hash = blake3::hash(&rom).into();
    let cart = Cartridge::from_rom(rom, None)?;
    Ok(LoadedGame {
        cart,
        cart_hash,
        mapper: DEFAULT_MAPPER.into(),
        sram_size: 0,
    })
}
