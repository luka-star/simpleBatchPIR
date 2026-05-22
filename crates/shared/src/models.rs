use crate::rings::Zp;
use crate::{keyword::RecordId, tokenize_text};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::num::Wrapping;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Band {
    pub id: i32,
    pub name: Option<String>,
    pub fans: i32,
    pub formed: i32,
    pub origin: Option<String>,
    pub split: i32,
    pub style: Option<String>,
}

impl Band {
    pub const SIZEOFRECORD: usize = 64;
    const ID_RANGE: std::ops::Range<usize> = 0..2;
    const NAME_RANGE: std::ops::Range<usize> = 2..22;
    const FANS_RANGE: std::ops::Range<usize> = 22..24;
    const FORMED_RANGE: std::ops::Range<usize> = 24..26;
    const ORIGIN_RANGE: std::ops::Range<usize> = 26..45;
    const SPLIT_RANGE: std::ops::Range<usize> = 45..47;
    const STYLE_RANGE: std::ops::Range<usize> = 47..64;

    pub fn pack_band_to_zp(band: &Band) -> Vec<Zp> {
        let mut buffer = vec![0u8; Self::SIZEOFRECORD];

        fn pack_string(slots: &mut [u8], s: &Option<String>) {
            if let Some(text) = s {
                let bytes = text.as_bytes();
                let len = bytes.len().min(slots.len());
                slots[..len].copy_from_slice(&bytes[..len]);
            }
        }

        buffer[Self::ID_RANGE].copy_from_slice(&(band.id as u16).to_le_bytes());
        buffer[Self::FANS_RANGE].copy_from_slice(&(band.fans as u16).to_le_bytes());
        buffer[Self::FORMED_RANGE].copy_from_slice(&(band.formed as u16).to_le_bytes());
        buffer[Self::SPLIT_RANGE].copy_from_slice(&(band.split as u16).to_le_bytes());

        pack_string(&mut buffer[Self::NAME_RANGE], &band.name);
        pack_string(&mut buffer[Self::ORIGIN_RANGE], &band.origin);
        pack_string(&mut buffer[Self::STYLE_RANGE], &band.style);

        buffer.into_iter().map(Wrapping).collect()
    }

    pub fn unpack_band_from_zp(data: &[Zp]) -> Band {
        assert!(data.len() >= Self::SIZEOFRECORD, "Input slice too small");

        let mut buffer = [0u8; Self::SIZEOFRECORD];
        for (i, val) in data.iter().take(Self::SIZEOFRECORD).enumerate() {
            buffer[i] = val.0;
        }

        fn unpack_string(slots: &[u8]) -> Option<String> {
            let trimmed: Vec<u8> = slots.iter().copied().take_while(|&b| b != 0).collect();

            if trimmed.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&trimmed).to_string())
            }
        }

        Band {
            id: u16::from_le_bytes(buffer[Self::ID_RANGE].try_into().unwrap()) as i32,
            fans: u16::from_le_bytes(buffer[Self::FANS_RANGE].try_into().unwrap()) as i32,
            formed: u16::from_le_bytes(buffer[Self::FORMED_RANGE].try_into().unwrap()) as i32,
            split: u16::from_le_bytes(buffer[Self::SPLIT_RANGE].try_into().unwrap()) as i32,
            name: unpack_string(&buffer[Self::NAME_RANGE]),
            origin: unpack_string(&buffer[Self::ORIGIN_RANGE]),
            style: unpack_string(&buffer[Self::STYLE_RANGE]),
        }
    }

    pub fn bands_to_matrix(bands: &[Band]) -> Array2<Zp> {
        let mut flat_data: Vec<Zp> = bands
            .iter()
            .flat_map(|band| Band::pack_band_to_zp(band))
            .collect();
        let total_elements = flat_data.len();
        let dim = (total_elements as f64).sqrt().ceil() as usize;
        flat_data.resize(dim * dim, Wrapping(0));
        Array2::from_shape_vec((dim, dim), flat_data).expect("Failed to reshape bands into matrix")
    }

    pub fn matrix_to_bands(matrix: &Array2<Zp>) -> Vec<Band> {
        let flat: Vec<Zp> = matrix.iter().cloned().collect();

        flat.chunks(Self::SIZEOFRECORD)
            .filter(|chunk| chunk.len() == Self::SIZEOFRECORD)
            .filter(|chunk| chunk.iter().any(|z| z.0 != 0))
            .map(Band::unpack_band_from_zp)
            .collect()
    }
}

fn push_tokens_from_text(text: &str, tokens: &mut Vec<String>, seen: &mut HashSet<String>) {
    for token in tokenize_text(text) {
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
}

pub fn tokenize_band(band: &Band) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut seen = HashSet::new();

    if let Some(name) = band.name.as_deref() {
        push_tokens_from_text(name, &mut tokens, &mut seen);
    }
    if let Some(origin) = band.origin.as_deref() {
        push_tokens_from_text(origin, &mut tokens, &mut seen);
    }
    if let Some(style) = band.style.as_deref() {
        for style_part in style.split(',') {
            push_tokens_from_text(style_part, &mut tokens, &mut seen);
        }
    }
    if band.formed > 0 {
        let token = band.formed.to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
    if band.split > 0 {
        let token = band.split.to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }

    tokens
}

pub fn construct_keyword_mapping(db: &[Band]) -> HashMap<String, Vec<RecordId>> {
    let mut mapping: HashMap<String, Vec<RecordId>> = HashMap::new();

    for band in db {
        for token in tokenize_band(band) {
            mapping.entry(token).or_default().push(band.id as usize);
        }
    }

    mapping
}
