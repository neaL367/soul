//! Integration tests for Windows DPAPI cryptographic protection.

use platform_windows::Dpapi;

#[test]
fn test_dpapi_protect_unprotect_roundtrip() {
    let secret = b"super_secret_cookie_token_12345";
    let encrypted = Dpapi::protect(secret).expect("DPAPI protect failed");
    assert_ne!(secret.as_slice(), encrypted.as_slice());

    let decrypted = Dpapi::unprotect(&encrypted).expect("DPAPI unprotect failed");
    assert_eq!(secret.as_slice(), decrypted.as_slice());
}
