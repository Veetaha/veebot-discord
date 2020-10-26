//! Dependency injection related stuff.
//!
//! Unfortunately DI is way too dynamic and flexible.
//! There is no good implementation for it in Rust because it is very
//! strict about the scope of each value in the program.
//! Having some globaly-accessible dependency container that everyone
//! has a reference to requires considerable thread syncronization (locking).
//!
//! Even though this seems not pretty, this way we may be sure there are no
//! concurrency and ownership bugs in our program, and this is what [`serenity`]
//! framework already enforces anyway...

use serenity::{
    async_trait,
    client::bridge::voice::ClientVoiceManager,
    prelude::{Mutex, RwLock, TypeMap, TypeMapKey},
};
use std::sync::Arc;

macro_rules! def_type_map_keys {
    ($($key:ident => $val:ty),*$(,)?) => {
        $(
            pub(crate) struct $key;
            impl TypeMapKey for $key {
                type Value = $val;
            }
        )*
    }
}

// Define all the DI keys and their respective values.
def_type_map_keys! {
    ClientVoiceManagerToken => Arc<Mutex<ClientVoiceManager>>,
    YtServiceToken => Arc<crate::yt::YtService>,
}

/// Utility trait to reduce boilerplate for retrieving and acquiring locks
/// on dependencies from the global DI container.
#[async_trait]
pub(crate) trait DiExt<T> {
    /// Locks the given `Arc<Mutex<T>>` dependency and returns.
    ///
    /// # Panics
    /// Panics if the dependency is not present in the `TypeMap`.
    async fn lock_dep<K>(&self) -> tokio::sync::OwnedMutexGuard<T>
    where
        T: Send + Sync,
        K: TypeMapKey<Value = Arc<Mutex<T>>>;

    /// Returns an `Arc<T>` for the given dependency token.
    ///
    /// # Panics
    /// Panics if the dependency is not present in the `TypeMap`.
    async fn expect_dep<K: TypeMapKey>(&self) -> Arc<T>
    where
        T: Send + Sync,
        K: TypeMapKey<Value = Arc<T>>;
}

#[async_trait]
impl<T: 'static> DiExt<T> for RwLock<TypeMap> {
    async fn lock_dep<K>(&self) -> tokio::sync::OwnedMutexGuard<T>
    where
        T: Send + Sync,
        K: TypeMapKey<Value = Arc<Mutex<T>>>,
    {
        self.expect_dep::<K>().await.lock_owned().await
    }

    async fn expect_dep<K: TypeMapKey>(&self) -> Arc<T>
    where
        T: Send + Sync,
        K: TypeMapKey<Value = Arc<T>>,
    {
        self.read()
            .await
            .get::<K>()
            .unwrap_or_else(|| {
                panic!(
                    "BUG: dependency value of type {} was not found using the token {}",
                    std::any::type_name::<K::Value>(),
                    std::any::type_name::<K>(),
                )
            })
            .clone()
    }
}
