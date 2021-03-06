// Copyright 2015 MaidSafe.net limited.
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

/// Common utility functions for writting test cases
pub mod test_utils;

use ::rand::Rng;

/// Combined Asymmetric and Symmetric encryption. The data is encrypted using random Key and
/// IV with Xsalsa-symmetric encryption. Random IV ensures that same plain text produces different
/// cipher-texts for each fresh symmetric encryption. The Key and IV are then asymmetrically
/// enrypted using Public-MAID and the whole thing is then serialised into a single Vec<u8>.
pub fn hybrid_encrypt(plain_text: &[u8],
                      asym_nonce: &::sodiumoxide::crypto::box_::Nonce,
                      asym_public_key: &::sodiumoxide::crypto::box_::PublicKey,
                      asym_secret_key: &::sodiumoxide::crypto::box_::SecretKey) -> Result<Vec<u8>, ::errors::ClientError> {
    let sym_key = ::sodiumoxide::crypto::secretbox::gen_key();
    let sym_nonce = ::sodiumoxide::crypto::secretbox::gen_nonce();

    let mut asym_plain_text = [0u8; ::sodiumoxide::crypto::secretbox::KEYBYTES + ::sodiumoxide::crypto::secretbox::NONCEBYTES];
    for it in sym_key.0.iter().chain(sym_nonce.0.iter()).enumerate() {
        asym_plain_text[it.0] = *it.1;
    }

    let sym_cipher_text = ::sodiumoxide::crypto::secretbox::seal(plain_text, &sym_nonce, &sym_key);
    let asym_cipher_text = ::sodiumoxide::crypto::box_::seal(&asym_plain_text, asym_nonce, asym_public_key, asym_secret_key);

    serialise(&(asym_cipher_text, sym_cipher_text))
}

/// Reverse of hybrid_encrypt. Refer hybrid_encrypt.
pub fn hybrid_decrypt(cipher_text: &[u8],
                      asym_nonce: &::sodiumoxide::crypto::box_::Nonce,
                      asym_public_key: &::sodiumoxide::crypto::box_::PublicKey,
                      asym_secret_key: &::sodiumoxide::crypto::box_::SecretKey) -> Result<Vec<u8>, ::errors::ClientError> {
    let (asym_cipher_text, sym_cipher_text): (Vec<u8>, Vec<u8>) = try!(deserialise(cipher_text));

    let asym_plain_text = try!(::sodiumoxide::crypto::box_::open(&asym_cipher_text, asym_nonce, asym_public_key, asym_secret_key).map_err(|_| ::errors::ClientError::AsymmetricDecipherFailure));
    if asym_plain_text.len() != ::sodiumoxide::crypto::secretbox::KEYBYTES + ::sodiumoxide::crypto::secretbox::NONCEBYTES {
        Err(::errors::ClientError::AsymmetricDecipherFailure)
    } else {
        let mut sym_key = ::sodiumoxide::crypto::secretbox::Key([0u8; ::sodiumoxide::crypto::secretbox::KEYBYTES]);
        let mut sym_nonce = ::sodiumoxide::crypto::secretbox::Nonce([0u8; ::sodiumoxide::crypto::secretbox::NONCEBYTES]);

        for it in asym_plain_text.iter().take(::sodiumoxide::crypto::secretbox::KEYBYTES).enumerate() {
            sym_key.0[it.0] = *it.1;
        }
        for it in asym_plain_text.iter().skip(::sodiumoxide::crypto::secretbox::KEYBYTES).enumerate() {
            sym_nonce.0[it.0] = *it.1;
        }

        ::sodiumoxide::crypto::secretbox::open(&sym_cipher_text, &sym_nonce, &sym_key).map_err(|_| ::errors::ClientError::SymmetricDecipherFailure)
    }
}

/// utility function to serialise an Encodable type
pub fn serialise<T>(data: &T) -> Result<Vec<u8>, ::errors::ClientError>
                                 where T: ::rustc_serialize::Encodable {
    let mut encoder = ::cbor::Encoder::from_memory();
    try!(encoder.encode(&[data]));
    Ok(encoder.into_bytes())
}

/// utility function to deserialise a Decodable type
pub fn deserialise<T>(data: &[u8]) -> Result<T, ::errors::ClientError>
                                      where T: ::rustc_serialize::Decodable {
    let mut d = ::cbor::Decoder::from_bytes(data);
    Ok(try!(try!(d.decode().next().ok_or(::errors::ClientError::UnsuccessfulEncodeDecode))))
}

/// Generates a random string for specified size
pub fn generate_random_string(length: usize) -> Result<String, ::errors::ClientError> {
    let mut os_rng = try!(::rand::OsRng::new().map_err(|error| {
        debug!("Error {:?}", error);
        ::errors::ClientError::RandomDataGenerationFailure
    }));
    Ok((0..length).map(|_| os_rng.gen::<char>()).collect())
}

/// Generate a random vector of given length
pub fn generate_random_vector<T>(length: usize) -> Result<Vec<T>, ::errors::ClientError>
                                                   where T: ::rand::Rand {
    let mut os_rng = try!(::rand::OsRng::new().map_err(|error| {
        debug!("Error {:?}", error);
        ::errors::ClientError::RandomDataGenerationFailure
    }));
    Ok((0..length).map(|_| os_rng.gen()).collect())
}

/// Generate a random array of 64 u8's
pub fn generate_random_array_u8_64() -> Result<[u8; 64], ::errors::ClientError> {
    let mut arr = [0; 64];
    let mut os_rng = try!(::rand::OsRng::new().map_err(|error| {
        debug!("Error {:?}", error);
        ::errors::ClientError::RandomDataGenerationFailure
    }));
    for it in arr.iter_mut() {
        *it = os_rng.gen();
    }
    Ok(arr)
}

/// Returns true if both slices are equal in length, and have equal contents
pub fn slice_equal<T: PartialEq>(lhs: &[T], rhs: &[T]) -> bool {
    lhs.len() == rhs.len() && lhs.iter().zip(rhs.iter()).all(|(a, b)| a == b)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hybrid_encrypt_decrypt() {
        // Identical Plain Texts
        let plain_text_0 = vec![123u8; 1000];
        let plain_text_1 = plain_text_0.clone();

        let nonce = ::sodiumoxide::crypto::box_::gen_nonce();
        let (public_key, secret_key) = ::sodiumoxide::crypto::box_::gen_keypair();

        // Encrypt
        let cipher_text_0 = eval_result!(hybrid_encrypt(&plain_text_0[..], &nonce, &public_key, &secret_key));
        let cipher_text_1 = eval_result!(hybrid_encrypt(&plain_text_1[..], &nonce, &public_key, &secret_key));

        // Same Plain Texts
        assert_eq!(plain_text_0, plain_text_1);

        // Different Results because of random "iv"
        assert!(cipher_text_0 != cipher_text_1);

        // Decrypt
        let deciphered_plain_text_0 = eval_result!(hybrid_decrypt(&cipher_text_0, &nonce, &public_key, &secret_key));
        let deciphered_plain_text_1 = eval_result!(hybrid_decrypt(&cipher_text_1, &nonce, &public_key, &secret_key));

        // Should have decrypted to the same Plain Texts
        assert_eq!(plain_text_0, deciphered_plain_text_0);
        assert_eq!(plain_text_1, deciphered_plain_text_1);
    }

    #[test]
    fn serialise_deserialise() {
        let original_data = (eval_result!(generate_random_vector::<u8>(13)),
                             eval_result!(generate_random_vector::<i64>(19)),
                             eval_result!(generate_random_string(10)));

        let serialised_data = eval_result!(serialise(&original_data));
        let deserialised_data: (Vec<u8>, Vec<i64>, String) = eval_result!(deserialise(&serialised_data));
        assert_eq!(original_data, deserialised_data);
    }

    #[test]
    fn random_string() {
        const SIZE: usize = 10;
        let str0 = eval_result!(generate_random_string(SIZE));
        let str1 = eval_result!(generate_random_string(SIZE));
        let str2 = eval_result!(generate_random_string(SIZE));

        assert!(str0 != str1);
        assert!(str0 != str2);
        assert!(str1 != str2);
    }

    #[test]
    fn random_vector() {
        const SIZE: usize = 10;
        let vec0 = eval_result!(generate_random_vector::<u8>(SIZE));
        let vec1 = eval_result!(generate_random_vector::<u8>(SIZE));
        let vec2 = eval_result!(generate_random_vector::<u8>(SIZE));

        assert!(vec0 != vec1);
        assert!(vec0 != vec2);
        assert!(vec1 != vec2);
    }

    #[test]
    fn random_array() {
        let arr0 = eval_result!(generate_random_array_u8_64());
        let arr1 = eval_result!(generate_random_array_u8_64());
        let arr2 = eval_result!(generate_random_array_u8_64());

        assert!(!slice_equal(&arr0, &arr1));
        assert!(!slice_equal(&arr0, &arr2));
        assert!(!slice_equal(&arr1, &arr2));
    }

    #[test]
    fn slice_eqality() {
        let arr0 = eval_result!(generate_random_array_u8_64());
        let arr1 = eval_result!(generate_random_array_u8_64());

        assert!(slice_equal(&arr0, &arr0));
        assert!(!slice_equal(&arr0, &arr1));
    }
}
