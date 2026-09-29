// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Encryption of cloud backups. Files are encrypted chunk by chunk from file to
//! file, so a backup of any size needs only a few chunks of memory.
//!
//! Version 2 uses XChaCha20-Poly1305. Every file gets a random 16 byte nonce
//! prefix and the rest of the nonce counts its chunks, so the thousands of
//! artwork files of an account never share a nonce. Every chunk is bound to
//! whether it is the last one, so a file cut short does not decrypt. Version 1
//! archives, ChaCha20-Poly1305 with a 4 byte prefix, can still be read.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce, XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::constants::{BACKUP_KEY_BYTES, ENCRYPTION_CHUNK_BYTES, HASH_BUFFER_BYTES};
use crate::error::{Result, VaultimeError};

/// Name of the current scheme in backup metadata.
pub const SCHEME: &str = "xchacha20poly1305-chunked-v2";

const MAGIC_V1: &[u8; 8] = b"VTENC01\n";
const MAGIC_V2: &[u8; 8] = b"VTENC02\n";
const AAD_V1: &[u8] = b"vaultime-cloud-backup";
const AAD_V2: &[u8] = b"vaultime-cloud-backup-v2";
const TAG_BYTES: usize = 16;
const CHUNK_INDEX_BYTES: usize = 8;
const V1_NONCE_BYTES: usize = 12;
const V1_NONCE_PREFIX_BYTES: usize = V1_NONCE_BYTES - CHUNK_INDEX_BYTES;
const V2_NONCE_BYTES: usize = 24;
const V2_NONCE_PREFIX_BYTES: usize = V2_NONCE_BYTES - CHUNK_INDEX_BYTES;
const LAST_CHUNK: u8 = 1;
/// Derives the key that names artwork from the backup key.
const ARTWORK_ID_LABEL: &[u8] = b"vaultime artwork id v1";
const KEY_CHECK_LABEL: &[u8] = b"vaultime key check v1";
/// Bytes of the key check kept, enough to tell two keys apart.
const KEY_CHECK_BYTES: usize = 16;

/// Encrypts `input_path` into `output_path` with the current scheme.
pub fn encrypt_file(
    input_path: &Path,
    output_path: &Path,
    key: &[u8; BACKUP_KEY_BYTES],
) -> Result<()> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut input = BufReader::new(File::open(input_path).map_err(io)?);
    let mut output = BufWriter::new(File::create(output_path).map_err(io)?);
    let mut prefix = [0_u8; V2_NONCE_PREFIX_BYTES];
    getrandom::fill(&mut prefix)
        .map_err(|error| VaultimeError::Backup(format!("no randomness for encryption: {error}")))?;
    output.write_all(MAGIC_V2).map_err(io)?;
    output.write_all(&prefix).map_err(io)?;

    let mut current = vec![0_u8; ENCRYPTION_CHUNK_BYTES];
    let mut next = vec![0_u8; ENCRYPTION_CHUNK_BYTES];
    let mut current_len = read_full(&mut input, &mut current)?;
    let mut index = 0_u64;
    loop {
        // A short chunk is the end of the file, a full one may be followed by more.
        let next_len = if current_len < ENCRYPTION_CHUNK_BYTES {
            0
        } else {
            read_full(&mut input, &mut next)?
        };
        let flags = if next_len == 0 { LAST_CHUNK } else { 0 };
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce::<V2_NONCE_BYTES>(&prefix, index)),
                Payload {
                    msg: &current[..current_len],
                    aad: &v2_aad(flags),
                },
            )
            .map_err(|error| VaultimeError::Backup(format!("failed to encrypt: {error}")))?;
        write_chunk(&mut output, flags, current_len, &ciphertext)?;
        if flags == LAST_CHUNK {
            break;
        }
        std::mem::swap(&mut current, &mut next);
        current_len = next_len;
        index += 1;
    }
    output.flush().map_err(io)
}

/// Decrypts a file of either scheme into `output_path`.
pub fn decrypt_file(
    input_path: &Path,
    output_path: &Path,
    key: &[u8; BACKUP_KEY_BYTES],
) -> Result<()> {
    let mut input = BufReader::new(File::open(input_path).map_err(io)?);
    let mut output = BufWriter::new(File::create(output_path).map_err(io)?);
    let mut magic = [0_u8; MAGIC_V2.len()];
    input.read_exact(&mut magic).map_err(io)?;

    if &magic == MAGIC_V2 {
        let cipher = XChaCha20Poly1305::new(key.into());
        let mut prefix = [0_u8; V2_NONCE_PREFIX_BYTES];
        input.read_exact(&mut prefix).map_err(io)?;
        decrypt_chunks(&mut input, &mut output, |index, flags, ciphertext| {
            cipher.decrypt(
                &XNonce::from(nonce::<V2_NONCE_BYTES>(&prefix, index)),
                Payload {
                    msg: ciphertext,
                    aad: &v2_aad(flags),
                },
            )
        })
    } else if &magic == MAGIC_V1 {
        let cipher = ChaCha20Poly1305::new(key.into());
        let mut prefix = [0_u8; V1_NONCE_PREFIX_BYTES];
        input.read_exact(&mut prefix).map_err(io)?;
        decrypt_chunks(&mut input, &mut output, |index, _, ciphertext| {
            cipher.decrypt(
                &Nonce::from(nonce::<V1_NONCE_BYTES>(&prefix, index)),
                Payload {
                    msg: ciphertext,
                    aad: AAD_V1,
                },
            )
        })
    } else {
        Err(VaultimeError::Backup(
            "the backup has an unknown encryption header".into(),
        ))
    }
}

/// Names artwork by its content, keyed with the backup key. The same image
/// always gets the same id, so it is stored once, and without the key an id
/// tells nothing about the image.
#[derive(Clone)]
pub struct ArtworkNamer {
    mac: Hmac<Sha256>,
}

impl ArtworkNamer {
    pub fn new(key: &[u8; BACKUP_KEY_BYTES]) -> Result<Self> {
        let mut derive = new_mac(key)?;
        derive.update(ARTWORK_ID_LABEL);
        Ok(Self {
            mac: new_mac(&derive.finalize().into_bytes())?,
        })
    }

    pub fn id(&self, path: &Path) -> Result<String> {
        let mut file = File::open(path).map_err(io)?;
        let mut mac = self.mac.clone();
        let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
        loop {
            let read = file.read(&mut buffer).map_err(io)?;
            if read == 0 {
                break;
            }
            mac.update(&buffer[..read]);
        }
        Ok(crate::hex::encode(&mac.finalize().into_bytes()))
    }
}

/// A short value that tells whether two backup keys are the same, stored with
/// every cloud backup. Without the key it reveals nothing about it.
pub fn key_check(key: &[u8; BACKUP_KEY_BYTES]) -> Result<String> {
    let mut mac = new_mac(key)?;
    mac.update(KEY_CHECK_LABEL);
    Ok(crate::hex::encode(
        &mac.finalize().into_bytes()[..KEY_CHECK_BYTES],
    ))
}

fn new_mac(key: &[u8]) -> Result<Hmac<Sha256>> {
    <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(key)
        .map_err(|error| VaultimeError::Backup(format!("invalid artwork key: {error}")))
}

fn decrypt_chunks(
    input: &mut impl Read,
    output: &mut impl Write,
    open: impl Fn(u64, u8, &[u8]) -> std::result::Result<Vec<u8>, chacha20poly1305::Error>,
) -> Result<()> {
    let mut ciphertext = Vec::new();
    let mut index = 0_u64;
    loop {
        let mut flags = [0_u8; 1];
        input.read_exact(&mut flags).map_err(io)?;
        let plaintext_len = read_len(input)?;
        let ciphertext_len = read_len(input)?;
        // The limit keeps a damaged header from asking for any amount of memory.
        if plaintext_len > ENCRYPTION_CHUNK_BYTES
            || ciphertext_len > ENCRYPTION_CHUNK_BYTES + TAG_BYTES
        {
            return Err(VaultimeError::Backup(
                "the backup has an oversized chunk".into(),
            ));
        }
        ciphertext.resize(ciphertext_len, 0);
        input.read_exact(&mut ciphertext).map_err(io)?;

        let plaintext = open(index, flags[0], &ciphertext).map_err(|_| {
            VaultimeError::Backup(
                "the backup does not decrypt, it is damaged or the passphrase differs".into(),
            )
        })?;
        if plaintext.len() != plaintext_len {
            return Err(VaultimeError::Backup(
                "a decrypted backup chunk has the wrong length".into(),
            ));
        }
        output.write_all(&plaintext).map_err(io)?;

        if flags[0] & LAST_CHUNK == LAST_CHUNK {
            break;
        }
        index += 1;
    }

    if input.read(&mut [0_u8; 1]).map_err(io)? != 0 {
        return Err(VaultimeError::Backup(
            "the backup has data after its last chunk".into(),
        ));
    }
    output.flush().map_err(io)
}

fn nonce<const N: usize>(prefix: &[u8], index: u64) -> [u8; N] {
    let mut nonce = [0_u8; N];
    let split = N - CHUNK_INDEX_BYTES;
    nonce[..split].copy_from_slice(&prefix[..split]);
    nonce[split..].copy_from_slice(&index.to_be_bytes());
    nonce
}

fn v2_aad(flags: u8) -> Vec<u8> {
    let mut aad = AAD_V2.to_vec();
    aad.push(flags);
    aad
}

fn write_chunk(
    output: &mut impl Write,
    flags: u8,
    plaintext_len: usize,
    ciphertext: &[u8],
) -> Result<()> {
    let len = |value: usize| {
        u32::try_from(value)
            .map_err(|_| VaultimeError::Backup("backup chunk length overflow".into()))
    };
    output.write_all(&[flags]).map_err(io)?;
    output
        .write_all(&len(plaintext_len)?.to_be_bytes())
        .map_err(io)?;
    output
        .write_all(&len(ciphertext.len())?.to_be_bytes())
        .map_err(io)?;
    output.write_all(ciphertext).map_err(io)
}

fn read_len(input: &mut impl Read) -> Result<usize> {
    let mut bytes = [0_u8; 4];
    input.read_exact(&mut bytes).map_err(io)?;
    usize::try_from(u32::from_be_bytes(bytes))
        .map_err(|_| VaultimeError::Backup("backup chunk length overflow".into()))
}

/// Fills `buffer` unless the input ends first. Returns the bytes read.
fn read_full(input: &mut impl Read, buffer: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = input.read(&mut buffer[filled..]).map_err(io)?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

fn io(error: std::io::Error) -> VaultimeError {
    VaultimeError::Backup(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const KEY: [u8; BACKUP_KEY_BYTES] = [7_u8; BACKUP_KEY_BYTES];

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultime-crypto-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn round_trip(dir: &Path, bytes: &[u8]) -> Result<Vec<u8>> {
        let (plain, sealed, opened) = (dir.join("plain"), dir.join("sealed"), dir.join("opened"));
        fs::write(&plain, bytes).unwrap();
        encrypt_file(&plain, &sealed, &KEY)?;
        decrypt_file(&sealed, &opened, &KEY)?;
        Ok(fs::read(opened).unwrap())
    }

    #[test]
    fn files_of_any_length_round_trip() {
        let dir = scratch();
        for len in [
            0,
            1,
            ENCRYPTION_CHUNK_BYTES,
            ENCRYPTION_CHUNK_BYTES * 2 + 17,
        ] {
            let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            assert_eq!(round_trip(&dir, &bytes).unwrap(), bytes, "length {len}");
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_cut_after_a_chunk_does_not_decrypt() {
        let dir = scratch();
        let (plain, sealed, opened) = (dir.join("plain"), dir.join("sealed"), dir.join("opened"));
        fs::write(&plain, vec![1_u8; ENCRYPTION_CHUNK_BYTES * 2]).unwrap();
        encrypt_file(&plain, &sealed, &KEY).unwrap();

        // Keep the first chunk and mark it as the last one.
        let mut bytes = fs::read(&sealed).unwrap();
        let header = MAGIC_V2.len() + V2_NONCE_PREFIX_BYTES;
        bytes.truncate(header + 1 + 8 + ENCRYPTION_CHUNK_BYTES + TAG_BYTES);
        bytes[header] = LAST_CHUNK;
        fs::write(&sealed, &bytes).unwrap();
        assert!(decrypt_file(&sealed, &opened, &KEY).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tampering_and_wrong_keys_fail() {
        let dir = scratch();
        let (plain, sealed, opened) = (dir.join("plain"), dir.join("sealed"), dir.join("opened"));
        fs::write(&plain, b"vaultime backup").unwrap();
        encrypt_file(&plain, &sealed, &KEY).unwrap();
        assert!(decrypt_file(&sealed, &opened, &[8_u8; BACKUP_KEY_BYTES]).is_err());

        let mut bytes = fs::read(&sealed).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&sealed, &bytes).unwrap();
        assert!(decrypt_file(&sealed, &opened, &KEY).is_err());

        bytes[last] ^= 1;
        bytes.push(0);
        fs::write(&sealed, &bytes).unwrap();
        assert!(
            decrypt_file(&sealed, &opened, &KEY).is_err(),
            "trailing data"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_file_gets_its_own_nonce() {
        let dir = scratch();
        let plain = dir.join("plain");
        fs::write(&plain, b"same content").unwrap();
        encrypt_file(&plain, &dir.join("a"), &KEY).unwrap();
        encrypt_file(&plain, &dir.join("b"), &KEY).unwrap();
        assert_ne!(
            fs::read(dir.join("a")).unwrap(),
            fs::read(dir.join("b")).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// The version 1 writer, kept to prove old cloud backups still restore.
    fn encrypt_v1(plaintext: &[u8], prefix: [u8; V1_NONCE_PREFIX_BYTES]) -> Vec<u8> {
        let cipher = ChaCha20Poly1305::new((&KEY).into());
        let mut out = MAGIC_V1.to_vec();
        out.extend_from_slice(&prefix);
        let chunks: Vec<&[u8]> = plaintext.chunks(ENCRYPTION_CHUNK_BYTES).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let last = index + 1 == chunks.len();
            let sealed = cipher
                .encrypt(
                    &Nonce::from(nonce::<V1_NONCE_BYTES>(&prefix, index as u64)),
                    Payload {
                        msg: chunk,
                        aad: AAD_V1,
                    },
                )
                .unwrap();
            write_chunk(&mut out, u8::from(last), chunk.len(), &sealed).unwrap();
        }
        out
    }

    // Existing cloud backups must stay decryptable across crate upgrades.
    #[test]
    fn chunk_output_is_stable() {
        let v1 = ChaCha20Poly1305::new((&KEY).into())
            .encrypt(
                &Nonce::from(nonce::<V1_NONCE_BYTES>(&[1, 2, 3, 4], 3)),
                Payload {
                    msg: b"vaultime",
                    aad: AAD_V1,
                },
            )
            .unwrap();
        assert_eq!(
            crate::hex::encode(&v1),
            "8b520c0a28beb3c892b7e36e387f57ca4d0da19da09e9629"
        );
        let v2 = XChaCha20Poly1305::new((&KEY).into())
            .encrypt(
                &XNonce::from(nonce::<V2_NONCE_BYTES>(&[9; V2_NONCE_PREFIX_BYTES], 3)),
                Payload {
                    msg: b"vaultime",
                    aad: &v2_aad(LAST_CHUNK),
                },
            )
            .unwrap();
        assert_eq!(
            crate::hex::encode(&v2),
            "ed24a28318f282075b0d7e57953b01d9ea4b00ea56635d2b"
        );
    }

    #[test]
    fn version_1_archives_still_decrypt() {
        let dir = scratch();
        let bytes: Vec<u8> = (0..ENCRYPTION_CHUNK_BYTES + 99)
            .map(|i| (i % 13) as u8)
            .collect();
        fs::write(dir.join("sealed"), encrypt_v1(&bytes, [1, 2, 3, 4])).unwrap();
        decrypt_file(&dir.join("sealed"), &dir.join("opened"), &KEY).unwrap();
        assert_eq!(fs::read(dir.join("opened")).unwrap(), bytes);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_checks_tell_keys_apart() {
        let key = [7_u8; BACKUP_KEY_BYTES];
        let check = key_check(&key).unwrap();
        assert_eq!(check, key_check(&key).unwrap());
        assert_eq!(check.len(), KEY_CHECK_BYTES * 2);
        assert_ne!(check, key_check(&[8_u8; BACKUP_KEY_BYTES]).unwrap());
    }

    #[test]
    fn artwork_ids_follow_content_and_key() {
        let dir = scratch();
        fs::write(dir.join("a"), b"cover").unwrap();
        fs::write(dir.join("b"), b"cover").unwrap();
        fs::write(dir.join("c"), b"other").unwrap();
        let namer = ArtworkNamer::new(&KEY).unwrap();
        let id = namer.id(&dir.join("a")).unwrap();
        assert_eq!(id.len(), 64);
        assert_eq!(id, namer.id(&dir.join("b")).unwrap());
        assert_ne!(id, namer.id(&dir.join("c")).unwrap());
        let other_key = ArtworkNamer::new(&[8_u8; BACKUP_KEY_BYTES]).unwrap();
        assert_ne!(id, other_key.id(&dir.join("a")).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
