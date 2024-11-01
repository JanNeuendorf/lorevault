use crate::*;
use age;

pub fn decrypt_agev1(
    encrypted: &Vec<u8>,
    ids_to_try: &Vec<age::x25519::Identity>,
) -> Result<Vec<u8>> {
    let decryptor = match age::Decryptor::new(&encrypted[..])? {
        age::Decryptor::Recipients(d) => d,
        _ => return Err(format_err!("The data was not encrypted for a recipient")),
    };
    let mut decrypted = vec![];
    let mut reader = decryptor
        .decrypt(ids_to_try.iter().map(|k| k as &dyn age::Identity))
        .context("No matching age-keys found")?;
    reader.read_to_end(&mut decrypted)?;

    Ok(decrypted)
}

pub fn load_agev1keys(paths: &Vec<PathBuf>) -> Result<Vec<age::x25519::Identity>> {
    let mut ids = vec![];
    for p in paths {
        let entries = age::IdentityFile::from_file(p.to_owned().to_string_lossy().into_owned())?
            .into_identities();
        for e in entries {
            match e {
                age::IdentityFileEntry::Native(n) => ids.push(n.clone() as age::x25519::Identity),
            }
        }
    }
    Ok(ids)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum DecryptionMethod {
    #[default]
    None,
    #[serde(rename = "agev1")]
    AgeV1,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;
    #[test]
    fn decryption_test() {
        let encrypted = include_bytes!("../testing/testsecret.age").to_vec();
        let keys =
            load_agev1keys(&vec![PathBuf::from_str("testing/testkey.txt").unwrap()]).unwrap();
        assert_eq!(keys.len(), 1);
        let decrypted = decrypt_agev1(&encrypted, &keys).unwrap();
        let decrypted_string = String::from_utf8(decrypted).unwrap();
        assert_eq!(decrypted_string, "Peter Parker is Spiderman\n");
    }
}
