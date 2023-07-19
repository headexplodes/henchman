use std::array::TryFromSliceError;
use std::convert::TryInto;
use std::mem::size_of;
use std::num::NonZeroU32;

use hex;
use lazy_static::lazy_static;
use ring::pbkdf2;
use ring::rand::SecureRandom;

lazy_static! {
    static ref RNG: ring::rand::SystemRandom = ring::rand::SystemRandom::new();
}

struct PasswordFormatV1;

impl PasswordFormatV1 {
    pub const VERSION: u8 = 1;
    pub const SALT_LEN: usize = 16;
    pub const OUTPUT_LEN: usize = ring::digest::SHA256_OUTPUT_LEN;
}

static PBKDF2_ALG: pbkdf2::Algorithm = ring::pbkdf2::PBKDF2_HMAC_SHA256;

const DEFAULT_ITERATIONS: u32 = 10_000;

pub struct PasswordParts {
    // if we change structure/algorithm in the future
    version: u8,
    iterations: u32,
    salt: Vec<u8>,
    hash: Vec<u8>,
}

pub fn hash_password(password: &str) -> String {
    let mut salt = [0u8; PasswordFormatV1::SALT_LEN];
    RNG.fill(&mut salt).expect("Error generating random number");

    let iterations = DEFAULT_ITERATIONS;

    let mut hash = [0u8; PasswordFormatV1::OUTPUT_LEN];
    pbkdf2::derive(PBKDF2_ALG, iterations.try_into().expect("Iterations is zero"), &salt, password.as_bytes(), &mut hash);

    let bin_encoded: Vec<u8> = [
        &PasswordFormatV1::VERSION.to_be_bytes()[..],
        &iterations.to_be_bytes()[..],
        &salt[..],
        &hash[..]
    ].concat();

    let hex_encoded = hex::encode(&bin_encoded);

    hex_encoded
}

#[derive(Debug)]
pub enum VerifyError {
    MalformedInput,
    UnsupportedVersion,
}

impl From<TryFromSliceError> for VerifyError {
    fn from(_: TryFromSliceError) -> Self {
        VerifyError::MalformedInput
    }
}

pub fn parse_password(hex_str: &str) -> Result<PasswordParts, VerifyError> {
    let buf = hex::decode(hex_str).map_err(|_| VerifyError::MalformedInput)?;

    let (version_bytes, buf) = buf.split_at(size_of::<u8>());
    let version = u8::from_be_bytes(version_bytes.try_into()?);

    let (iterations_bytes, buf) = buf.split_at(size_of::<u32>());
    let iterations = u32::from_be_bytes(iterations_bytes.try_into()?);

    let (salt, buf) = buf.split_at(PasswordFormatV1::SALT_LEN);
    if salt.len() != PasswordFormatV1::SALT_LEN {
        return Err(VerifyError::MalformedInput);
    }

    let (hash, buf) = buf.split_at(PasswordFormatV1::OUTPUT_LEN);
    if hash.len() != PasswordFormatV1::OUTPUT_LEN {
        return Err(VerifyError::MalformedInput);
    }

    if buf.len() != 0 {
        return Err(VerifyError::MalformedInput);
    }

    Ok(PasswordParts {
        version,
        iterations,
        salt: salt.into(),
        hash: hash.into(),
    })
}

pub fn verify_password(attempt: &str, expected_hex: &str) -> Result<bool, VerifyError> {
    let expected = parse_password(expected_hex)?;

    verify_password_parts(attempt, &expected)
}

pub fn verify_password_parts(attempt: &str, expected: &PasswordParts) -> Result<bool, VerifyError> {
    if expected.version != PasswordFormatV1::VERSION {
        return Err(VerifyError::UnsupportedVersion);
    }

    pbkdf2::verify(PBKDF2_ALG,
                   NonZeroU32::new(expected.iterations).ok_or_else(|| VerifyError::MalformedInput)?,
                   &expected.salt,
                   attempt.as_bytes(),
                   &expected.hash)
        .map_or(Ok(false), |_| Ok(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let hashed = hash_password("secret");

        // assert_eq!(hashed.len(), 170);
        assert_eq!(hashed.len(), 106);

        assert_eq!(verify_password("secret", &hashed).ok(), Some(true));
        assert_eq!(verify_password("wrong", &hashed).ok(), Some(false));
    }
}