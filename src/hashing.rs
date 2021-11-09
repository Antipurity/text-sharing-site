use sha2::Digest;



/// Computes an SHA256 hash of a concatenation of byte arrays, given in the iterator.
/// Irreversible hash: given output, input is practically impossible to predict.
/// Example:
/// ```
/// println!("SHA256: {}", hash(["hello".as_bytes()].iter()));
/// // SHA256: 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
/// ```
fn hash<T>(of: T) -> String
where T:Iterator, <T as Iterator>::Item: AsRef<[u8]> {
    let mut hasher = sha2::Sha256::new();
    for item in of {
        hasher.update(item);
    }
    return hex::encode(hasher.finalize())
}

/// Hashes username+password info (an access token), so that we don't store them, and attackers can't realistically guess them.
pub fn access_token_hash(access: &String) -> String {
    hash(["saltghdcexg".as_bytes(), access.as_bytes(), "nhlfjeryhbbugvtj6vtt6i67vtiv998".as_bytes()].iter())
}