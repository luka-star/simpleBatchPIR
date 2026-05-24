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
    pub country: Option<String>,
    pub genre: Option<String>,
    pub status: Option<String>,
}

impl Band {
    pub const SIZEOFRECORD: usize = 96;
    const ID_RANGE: std::ops::Range<usize> = 0..4;
    const NAME_RANGE: std::ops::Range<usize> = 4..32;
    const COUNTRY_RANGE: std::ops::Range<usize> = 32..52;
    const GENRE_RANGE: std::ops::Range<usize> = 52..88;
    const STATUS_RANGE: std::ops::Range<usize> = 88..96;

    pub fn pack_band_to_zp(band: &Band) -> Vec<Zp> {
        let mut buffer = vec![0u8; Self::SIZEOFRECORD];

        fn pack_string(slots: &mut [u8], s: &Option<String>) {
            if let Some(text) = s {
                let bytes = text.as_bytes();
                let len = bytes.len().min(slots.len());
                slots[..len].copy_from_slice(&bytes[..len]);
            }
        }

        buffer[Self::ID_RANGE].copy_from_slice(&(band.id as u32).to_le_bytes());

        pack_string(&mut buffer[Self::NAME_RANGE], &band.name);
        pack_string(&mut buffer[Self::COUNTRY_RANGE], &band.country);
        pack_string(&mut buffer[Self::GENRE_RANGE], &band.genre);
        pack_string(&mut buffer[Self::STATUS_RANGE], &band.status);

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
            id: u32::from_le_bytes(buffer[Self::ID_RANGE].try_into().unwrap()) as i32,
            name: unpack_string(&buffer[Self::NAME_RANGE]),
            country: unpack_string(&buffer[Self::COUNTRY_RANGE]),
            genre: unpack_string(&buffer[Self::GENRE_RANGE]),
            status: unpack_string(&buffer[Self::STATUS_RANGE]),
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
    if let Some(country) = band.country.as_deref() {
        push_tokens_from_text(country, &mut tokens, &mut seen);
    }
    if let Some(genre) = band.genre.as_deref() {
        for genre_part in genre.split(['/', ',', ';']) {
            push_tokens_from_text(genre_part, &mut tokens, &mut seen);
        }
    }
    if let Some(status) = band.status.as_deref() {
        push_tokens_from_text(status, &mut tokens, &mut seen);
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
