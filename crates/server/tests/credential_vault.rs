use secrecy::{ExposeSecret, SecretBox};
use std::collections::BTreeMap;
use waxdemon_discogs::oauth::OAuthCredentials;
use waxdemon_server::credential_vault::CredentialVault;

fn vault(active: &str, keys: &[(&str, u8)]) -> CredentialVault {
    CredentialVault::new(
        active.into(),
        keys.iter()
            .map(|(id, byte)| ((*id).into(), SecretBox::new(Box::new([*byte; 32]))))
            .collect(),
    )
    .unwrap()
}

fn credentials() -> OAuthCredentials {
    OAuthCredentials::new("user-access-token".into(), "user-access-secret".into()).unwrap()
}

#[test]
fn round_trips_credentials_that_need_json_escaping() {
    let vault = vault("primary", &[("primary", 1)]);
    let token = "token\"with\\escaping";
    let credentials = OAuthCredentials::new(token.into(), "secret\nvalue".into()).unwrap();
    let row = vault.encrypt(42, &credentials).unwrap();
    let decrypted = vault.decrypt(42, &row).unwrap();
    assert_eq!(decrypted.token().expose_secret(), token);
    assert_eq!(decrypted.secret().expose_secret(), "secret\nvalue");
}

#[test]
fn encrypts_both_credentials_with_a_fresh_nonce_and_round_trips() {
    let vault = vault("primary", &[("primary", 1)]);
    let first = vault.encrypt(42, &credentials()).unwrap();
    let second = vault.encrypt(42, &credentials()).unwrap();
    assert_ne!(first.nonce, second.nonce);
    assert_ne!(first.ciphertext, second.ciphertext);
    assert_eq!(first.nonce.len(), 24);
    assert!(
        !first
            .ciphertext
            .windows(17)
            .any(|w| w == b"user-access-token")
    );
    let decrypted = vault.decrypt(42, &first).unwrap();
    assert_eq!(decrypted.token().expose_secret(), "user-access-token");
    assert_eq!(decrypted.secret().expose_secret(), "user-access-secret");
}

#[test]
fn rejects_cross_user_copy_tampering_wrong_key_and_unknown_key() {
    let vault = vault("primary", &[("primary", 1), ("alias", 1)]);
    let encrypted = vault.encrypt(42, &credentials()).unwrap();
    assert!(vault.decrypt(43, &encrypted).is_err());
    let wrong_key = CredentialVault::new(
        "primary".into(),
        BTreeMap::from([("primary".into(), SecretBox::new(Box::new([2; 32])))]),
    )
    .unwrap();
    assert!(wrong_key.decrypt(42, &encrypted).is_err());
    for change in 0..5 {
        let mut changed = encrypted.clone();
        match change {
            0 => changed.ciphertext[0] ^= 1,
            1 => changed.nonce[0] ^= 1,
            2 => changed.key_id = "missing".into(),
            3 => changed.key_id = "alias".into(),
            _ => changed.nonce.clear(),
        }
        assert!(vault.decrypt(42, &changed).is_err());
    }
}

#[test]
fn rotates_new_writes_while_retained_keys_can_read_old_rows() {
    let old = vault("old", &[("old", 1)])
        .encrypt(42, &credentials())
        .unwrap();
    let rotated = vault("new", &[("old", 1), ("new", 2)]);
    assert!(rotated.decrypt(42, &old).is_ok());
    let new = rotated.encrypt(42, &credentials()).unwrap();
    assert_eq!(new.key_id, "new");
    assert!(vault("new", &[("new", 2)]).decrypt(42, &new).is_ok());
    assert!(vault("new", &[("new", 2)]).decrypt(42, &old).is_err());
}

#[test]
fn rejects_missing_keys_invalid_identifiers_and_invalid_owners() {
    assert!(CredentialVault::new("missing".into(), BTreeMap::new()).is_err());
    assert!(
        CredentialVault::new(
            " ".into(),
            BTreeMap::from([(" ".into(), SecretBox::new(Box::new([1; 32]))),])
        )
        .is_err()
    );
    let vault = vault("primary", &[("primary", 1)]);
    assert!(vault.encrypt(0, &credentials()).is_err());
    assert!(vault.encrypt(-1, &credentials()).is_err());
}
