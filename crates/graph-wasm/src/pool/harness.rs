//! The pool served by `std::thread`s, for the native tests of the pool and of the live session.

use super::Pool;

/// Runs `body` over a pool served by `helpers` `std::thread`s in place of wasm instances: the
/// same `Mutex`/`Condvar` code the shared-memory build runs. The pool is closed when `body`
/// ends, panicking or not, so the scope can join.
pub(crate) fn with_pool(helpers: u32, body: impl FnOnce(&Pool)) {
    struct Closing<'p>(&'p Pool);
    impl Drop for Closing<'_> {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    let pool = Pool::new();
    std::thread::scope(|scope| {
        for _ in 0..helpers {
            scope.spawn(|| pool.serve());
        }
        let _closing = Closing(&pool);
        while pool.helpers() < helpers {
            std::thread::yield_now();
        }
        body(&pool);
    });
}
