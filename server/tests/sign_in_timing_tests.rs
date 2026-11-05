// A sign-in for an address with no account takes as long as a wrong password, or
// the time taken says which addresses have accounts. Alone in its own file, because
// the hash it compares against is made once per process, on first use.

mod common;

use common::*;
use jottery_server::{
    config::Config,
    utils::password::{hash_for_storage, verify_sign_in},
};
use std::time::Instant;

#[tokio::test]
async fn an_unknown_address_takes_as_long_as_a_wrong_password() {
    let config = Config {
        argon2_m_cost: 32 * 1024,
        argon2_t_cost: 3,
        ..test_config()
    };
    let hash = hash_for_storage(&config, &random_password()).await.unwrap();
    // Makes the hash that unknown addresses are checked against
    verify_sign_in(&config, &random_password(), None).await.unwrap();

    let started = Instant::now();
    verify_sign_in(&config, &random_password(), Some(&hash)).await.unwrap();
    let wrong_password = started.elapsed();

    let started = Instant::now();
    verify_sign_in(&config, &random_password(), None).await.unwrap();
    let unknown_address = started.elapsed();

    assert!(
        unknown_address > wrong_password / 3,
        "wrong password {wrong_password:?}, unknown address {unknown_address:?}"
    );
}
