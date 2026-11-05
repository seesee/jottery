// Password hashing takes a core and a good deal of memory for its whole run, so it
// mustn't happen on the async runtime, and an unknown address must take as long
// as a wrong password.

mod common;

use common::*;
use jottery_server::{
    config::Config,
    utils::password::{hash_for_storage, verify_sign_in, verify_stored},
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

/// Costly enough that a hash outlasts a timer tick.
fn costly_config() -> Config {
    Config {
        argon2_m_cost: 32 * 1024,
        argon2_t_cost: 3,
        ..test_config()
    }
}

#[tokio::test]
async fn hashing_and_verifying_round_trip() {
    let config = test_config();
    let password = random_password();

    let hash = hash_for_storage(&config, &password).await.unwrap();

    assert!(verify_stored(&password, &hash).await.unwrap());
    assert!(!verify_stored(&random_password(), &hash).await.unwrap());
}

#[tokio::test]
async fn a_sign_in_checks_the_hash_it_is_given() {
    let config = test_config();
    let password = random_password();
    let hash = hash_for_storage(&config, &password).await.unwrap();

    assert!(verify_sign_in(&config, &password, Some(&hash)).await.unwrap());
    assert!(!verify_sign_in(&config, &random_password(), Some(&hash)).await.unwrap());
}

#[tokio::test]
async fn nobody_signs_in_to_an_account_that_does_not_exist() {
    let config = test_config();

    assert!(!verify_sign_in(&config, &random_password(), None).await.unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn the_runtime_keeps_running_during_a_hash() {
    // With one thread, a hash done in place would stop this ticker dead
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = {
        let ticks = ticks.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1)).await;
                ticks.fetch_add(1, Ordering::SeqCst);
            }
        })
    };
    tokio::task::yield_now().await;

    hash_for_storage(&costly_config(), &random_password()).await.unwrap();

    ticker.abort();
    assert!(ticks.load(Ordering::SeqCst) > 0, "the hash blocked the runtime");
}
