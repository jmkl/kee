use std::collections::HashMap;

use anyhow::bail;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub struct TKeePair {
    pub key: String,
    pub func: String,
}
impl TKeePair {
    pub fn new(key: impl Into<String>, func: impl Into<String>) -> TKeePair {
        TKeePair {
            key: key.into(),
            func: func.into(),
        }
    }
}
// Custom deserialization from map format to Vec<TKeePair>
impl<'de> Deserialize<'de> for TKeePair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // This is for individual TKeePair, but we need custom logic for Vec
        #[derive(Deserialize)]
        struct TKeePairHelper {
            key: String,
            func: String,
        }

        let helper = TKeePairHelper::deserialize(deserializer)?;
        Ok(TKeePair {
            key: helper.key,
            func: helper.func,
        })
    }
}

impl Serialize for TKeePair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TKeePairHelper<'a> {
            key: &'a str,
            func: &'a str,
        }

        TKeePairHelper {
            key: &self.key,
            func: &self.func,
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TKeePairList(pub Vec<TKeePair>);

impl<'de> Deserialize<'de> for TKeePairList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, String>::deserialize(deserializer)?;
        let pairs: Vec<TKeePair> = map
            .into_iter()
            .map(|(key, func)| TKeePair { key, func })
            .collect();

        Ok(TKeePairList(pairs))
    }
}

impl Serialize for TKeePairList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert Vec<TKeePair> to HashMap<String, String>
        let map: HashMap<&str, &str> = self
            .0
            .iter()
            .map(|kp| (kp.key.as_str(), kp.func.as_str()))
            .collect();

        map.serialize(serializer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TKey {
    // Alphabet
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Numbers (regular row)
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    // Numpad
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpReturn,
    KpMinus,
    KpPlus,
    KpMultiply,
    KpDivide,
    KpDelete,
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    // Modifiers
    Alt,
    AltGr,
    ControlLeft,
    ControlRight,
    ShiftLeft,
    ShiftRight,
    MetaLeft,
    MetaRight,
    // Special keys
    Return,
    Space,
    Escape,
    Tab,
    Backspace,
    Delete,
    Insert,
    // Arrow keys
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    // Navigation
    Home,
    End,
    PageUp,
    PageDown,
    // Locks and controls
    CapsLock,
    NumLock,
    ScrollLock,
    Pause,
    PrintScreen,
    // Symbols and punctuation
    BackQuote,
    Minus,
    Equal,
    LeftBracket,
    RightBracket,
    BackSlash,
    IntlBackslash,
    SemiColon,
    Quote,
    Comma,
    Dot,
    Slash,
    // Media keys
    VolumeUp,
    VolumeDown,
    VolumeMute,
    BrightnessUp,
    BrightnessDown,
    PreviousTrack,
    PlayPause,
    PlayCd,
    NextTrack,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    #[inline]
    pub fn new() -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        }
    }

    #[inline]
    pub fn to_flags(&self) -> u32 {
        (self.ctrl as u32) * 0x0002
            + (self.shift as u32) * 0x0004
            + (self.alt as u32) * 0x0001
            + (self.meta as u32) * 0x0008
    }
}

#[derive(Debug, Clone)]
pub struct TsckKeeBinding {
    pub key: TKey,
    pub modifiers: Modifiers,
}

impl TsckKeeBinding {
    pub fn parse(input: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = input.split('-').collect();

        if parts.is_empty() {
            bail!("Empty hotkey string")
        }

        let mut modifiers = Modifiers::new();
        let key_part = if parts.len() == 1 {
            parts[0]
        } else {
            for part in &parts[..parts.len() - 1] {
                match part.as_bytes() {
                    b"C" => modifiers.ctrl = true,
                    b"S" => modifiers.shift = true,
                    b"A" => modifiers.alt = true,
                    b"M" | b"W" => modifiers.meta = true,
                    _ => {
                        bail!(format!("Unknown modifier: {}", part));
                    }
                }
            }
            parts[parts.len() - 1]
        };

        let key = Self::parse_key(key_part)?;
        Ok(Self { key, modifiers })
    }

    #[inline]
    fn parse_key(key_str: &str) -> anyhow::Result<TKey> {
        match key_str.as_bytes() {
            // Letters
            b"a" => Ok(TKey::A),
            b"b" => Ok(TKey::B),
            b"c" => Ok(TKey::C),
            b"d" => Ok(TKey::D),
            b"e" => Ok(TKey::E),
            b"f" => Ok(TKey::F),
            b"g" => Ok(TKey::G),
            b"h" => Ok(TKey::H),
            b"i" => Ok(TKey::I),
            b"j" => Ok(TKey::J),
            b"k" => Ok(TKey::K),
            b"l" => Ok(TKey::L),
            b"m" => Ok(TKey::M),
            b"n" => Ok(TKey::N),
            b"o" => Ok(TKey::O),
            b"p" => Ok(TKey::P),
            b"q" => Ok(TKey::Q),
            b"r" => Ok(TKey::R),
            b"s" => Ok(TKey::S),
            b"t" => Ok(TKey::T),
            b"u" => Ok(TKey::U),
            b"v" => Ok(TKey::V),
            b"w" => Ok(TKey::W),
            b"x" => Ok(TKey::X),
            b"y" => Ok(TKey::Y),
            b"z" => Ok(TKey::Z),
            // Numbers
            b"0" => Ok(TKey::Num0),
            b"1" => Ok(TKey::Num1),
            b"2" => Ok(TKey::Num2),
            b"3" => Ok(TKey::Num3),
            b"4" => Ok(TKey::Num4),
            b"5" => Ok(TKey::Num5),
            b"6" => Ok(TKey::Num6),
            b"7" => Ok(TKey::Num7),
            b"8" => Ok(TKey::Num8),
            b"9" => Ok(TKey::Num9),
            // Numpad
            b"kp0" => Ok(TKey::Kp0),
            b"kp1" => Ok(TKey::Kp1),
            b"kp2" => Ok(TKey::Kp2),
            b"kp3" => Ok(TKey::Kp3),
            b"kp4" => Ok(TKey::Kp4),
            b"kp5" => Ok(TKey::Kp5),
            b"kp6" => Ok(TKey::Kp6),
            b"kp7" => Ok(TKey::Kp7),
            b"kp8" => Ok(TKey::Kp8),
            b"kp9" => Ok(TKey::Kp9),
            b"kpreturn" => Ok(TKey::KpReturn),
            b"kpminus" => Ok(TKey::KpMinus),
            b"kpplus" => Ok(TKey::KpPlus),
            b"kpmultiply" => Ok(TKey::KpMultiply),
            b"kpdivide" => Ok(TKey::KpDivide),
            b"kpdelete" => Ok(TKey::KpDelete),
            // Function keys
            b"f1" => Ok(TKey::F1),
            b"f2" => Ok(TKey::F2),
            b"f3" => Ok(TKey::F3),
            b"f4" => Ok(TKey::F4),
            b"f5" => Ok(TKey::F5),
            b"f6" => Ok(TKey::F6),
            b"f7" => Ok(TKey::F7),
            b"f8" => Ok(TKey::F8),
            b"f9" => Ok(TKey::F9),
            b"f10" => Ok(TKey::F10),
            b"f11" => Ok(TKey::F11),
            b"f12" => Ok(TKey::F12),
            b"f13" => Ok(TKey::F13),
            b"f14" => Ok(TKey::F14),
            b"f15" => Ok(TKey::F15),
            b"f16" => Ok(TKey::F16),
            b"f17" => Ok(TKey::F17),
            b"f18" => Ok(TKey::F18),
            b"f19" => Ok(TKey::F19),
            b"f20" => Ok(TKey::F20),
            b"f21" => Ok(TKey::F21),
            b"f22" => Ok(TKey::F22),
            b"f23" => Ok(TKey::F23),
            b"f24" => Ok(TKey::F24),
            // Modifiers
            b"alt" => Ok(TKey::Alt),
            b"altgr" => Ok(TKey::AltGr),
            b"ctrl" | b"controlleft" => Ok(TKey::ControlLeft),
            b"ctrlright" | b"controlright" => Ok(TKey::ControlRight),
            b"shift" | b"shiftleft" => Ok(TKey::ShiftLeft),
            b"shiftright" => Ok(TKey::ShiftRight),
            b"meta" | b"metaleft" => Ok(TKey::MetaLeft),
            b"metaright" => Ok(TKey::MetaRight),
            // Special
            b"return" | b"enter" => Ok(TKey::Return),
            b"space" => Ok(TKey::Space),
            b"escape" | b"esc" => Ok(TKey::Escape),
            b"tab" => Ok(TKey::Tab),
            b"backspace" => Ok(TKey::Backspace),
            b"delete" | b"del" => Ok(TKey::Delete),
            b"insert" => Ok(TKey::Insert),
            // Arrows
            b"up" => Ok(TKey::UpArrow),
            b"down" => Ok(TKey::DownArrow),
            b"left" => Ok(TKey::LeftArrow),
            b"right" => Ok(TKey::RightArrow),
            // Navigation
            b"home" => Ok(TKey::Home),
            b"end" => Ok(TKey::End),
            b"pageup" => Ok(TKey::PageUp),
            b"pagedown" => Ok(TKey::PageDown),
            // Locks
            b"capslock" => Ok(TKey::CapsLock),
            b"numlock" => Ok(TKey::NumLock),
            b"scrolllock" => Ok(TKey::ScrollLock),
            b"pause" => Ok(TKey::Pause),
            b"printscreen" => Ok(TKey::PrintScreen),
            // Symbols
            b"backquote" | b"`" => Ok(TKey::BackQuote),
            b"minus" => Ok(TKey::Minus),
            b"equal" | b"=" => Ok(TKey::Equal),
            b"[" => Ok(TKey::LeftBracket),
            b"]" => Ok(TKey::RightBracket),
            b"\\" => Ok(TKey::BackSlash),
            b"intlbackslash" => Ok(TKey::IntlBackslash),
            b";" => Ok(TKey::SemiColon),
            b"'" => Ok(TKey::Quote),
            b"," => Ok(TKey::Comma),
            b"." => Ok(TKey::Dot),
            b"/" | b"slash" => Ok(TKey::Slash),
            // Media
            b"volumeup" => Ok(TKey::VolumeUp),
            b"volumedown" => Ok(TKey::VolumeDown),
            b"volumemute" => Ok(TKey::VolumeMute),
            b"brightnessup" => Ok(TKey::BrightnessUp),
            b"brightnessdown" => Ok(TKey::BrightnessDown),
            b"previoustrack" => Ok(TKey::PreviousTrack),
            b"playpause" => Ok(TKey::PlayPause),
            b"playcd" => Ok(TKey::PlayCd),
            b"nexttrack" => Ok(TKey::NextTrack),
            b"function" => Ok(TKey::Function),
            _ => bail!(format!("Unknown key: {}", key_str)),
        }
    }

    #[inline]
    pub fn to_tk(&self) -> u16 {
        match self.key {
            TKey::A => 0x41,
            TKey::B => 0x42,
            TKey::C => 0x43,
            TKey::D => 0x44,
            TKey::E => 0x45,
            TKey::F => 0x46,
            TKey::G => 0x47,
            TKey::H => 0x48,
            TKey::I => 0x49,
            TKey::J => 0x4A,
            TKey::K => 0x4B,
            TKey::L => 0x4C,
            TKey::M => 0x4D,
            TKey::N => 0x4E,
            TKey::O => 0x4F,
            TKey::P => 0x50,
            TKey::Q => 0x51,
            TKey::R => 0x52,
            TKey::S => 0x53,
            TKey::T => 0x54,
            TKey::U => 0x55,
            TKey::V => 0x56,
            TKey::W => 0x57,
            TKey::X => 0x58,
            TKey::Y => 0x59,
            TKey::Z => 0x5A,
            TKey::Num0 => 0x30,
            TKey::Num1 => 0x31,
            TKey::Num2 => 0x32,
            TKey::Num3 => 0x33,
            TKey::Num4 => 0x34,
            TKey::Num5 => 0x35,
            TKey::Num6 => 0x36,
            TKey::Num7 => 0x37,
            TKey::Num8 => 0x38,
            TKey::Num9 => 0x39,
            TKey::Kp0 => 0x60,
            TKey::Kp1 => 0x61,
            TKey::Kp2 => 0x62,
            TKey::Kp3 => 0x63,
            TKey::Kp4 => 0x64,
            TKey::Kp5 => 0x65,
            TKey::Kp6 => 0x66,
            TKey::Kp7 => 0x67,
            TKey::Kp8 => 0x68,
            TKey::Kp9 => 0x69,
            TKey::KpReturn => 0x0D,
            TKey::KpMinus => 0x6D,
            TKey::KpPlus => 0x6B,
            TKey::KpMultiply => 0x6A,
            TKey::KpDivide => 0x6F,
            TKey::KpDelete => 0x2E,
            TKey::F1 => 0x70,
            TKey::F2 => 0x71,
            TKey::F3 => 0x72,
            TKey::F4 => 0x73,
            TKey::F5 => 0x74,
            TKey::F6 => 0x75,
            TKey::F7 => 0x76,
            TKey::F8 => 0x77,
            TKey::F9 => 0x78,
            TKey::F10 => 0x79,
            TKey::F11 => 0x7A,
            TKey::F12 => 0x7B,
            TKey::F13 => 0x7C,
            TKey::F14 => 0x7D,
            TKey::F15 => 0x7E,
            TKey::F16 => 0x7F,
            TKey::F17 => 0x80,
            TKey::F18 => 0x81,
            TKey::F19 => 0x82,
            TKey::F20 => 0x83,
            TKey::F21 => 0x84,
            TKey::F22 => 0x85,
            TKey::F23 => 0x86,
            TKey::F24 => 0x87,
            TKey::Alt => 0x12,
            TKey::AltGr => 0xA5,
            TKey::ControlLeft => 0xA2,
            TKey::ControlRight => 0xA3,
            TKey::ShiftLeft => 0xA0,
            TKey::ShiftRight => 0xA1,
            TKey::MetaLeft => 0x5B,
            TKey::MetaRight => 0x5C,
            TKey::Return => 0x0D,
            TKey::Space => 0x20,
            TKey::Escape => 0x1B,
            TKey::Tab => 0x09,
            TKey::Backspace => 0x08,
            TKey::Delete => 0x2E,
            TKey::Insert => 0x2D,
            TKey::UpArrow => 0x26,
            TKey::DownArrow => 0x28,
            TKey::LeftArrow => 0x25,
            TKey::RightArrow => 0x27,
            TKey::Home => 0x24,
            TKey::End => 0x23,
            TKey::PageUp => 0x21,
            TKey::PageDown => 0x22,
            TKey::CapsLock => 0x14,
            TKey::NumLock => 0x90,
            TKey::ScrollLock => 0x91,
            TKey::Pause => 0x13,
            TKey::PrintScreen => 0x2C,
            TKey::BackQuote => 0xC0,
            TKey::Minus => 0xBD,
            TKey::Equal => 0xBB,
            TKey::LeftBracket => 0xDB,
            TKey::RightBracket => 0xDD,
            TKey::BackSlash => 0xDC,
            TKey::IntlBackslash => 0xE2,
            TKey::SemiColon => 0xBA,
            TKey::Quote => 0xDE,
            TKey::Comma => 0xBC,
            TKey::Dot => 0xBE,
            TKey::Slash => 0xBF,
            TKey::VolumeUp => 0xAF,
            TKey::VolumeDown => 0xAE,
            TKey::VolumeMute => 0xAD,
            TKey::BrightnessUp => 0xF6,
            TKey::BrightnessDown => 0xF5,
            TKey::PreviousTrack => 0xB1,
            TKey::PlayPause => 0xB3,
            TKey::PlayCd => 0xFA,
            TKey::NextTrack => 0xB0,
            TKey::Function => 0xFF,
        }
    }
}
