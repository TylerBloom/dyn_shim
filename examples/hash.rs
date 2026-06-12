//! Hashable boxed trait objects with `#[dyn_shim]`.
//!
//! The `Event` trait below wants hashable implementors, but `Hash` cannot be
//! one of its supertraits without giving up dyn-compatibility (`hash` is
//! generic over the hasher). Instead, a `Hash` in the attribute's bounds is
//! recognized and handled specially: the generated `DynEvent` shim carries
//! hidden machinery so `dyn DynEvent` implements `Hash` (covering
//! `&dyn DynEvent` and `Box<dyn DynEvent>`), and only `Hash` implementors of
//! `Event` receive the shim.
//!
//! Coming from the `dyn-hash` crate? `examples/migrate_dyn_hash.rs` walks
//! through the conversion step by step.
//!
//! Run with: `cargo run --example hash`

use dyn_shim::dyn_shim;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher, Hash};

#[dyn_shim(DynEvent: Hash)]
trait Event {
    fn describe(&self) -> String;
}

#[derive(Hash)]
struct KeyPress(char);

impl Event for KeyPress {
    fn describe(&self) -> String {
        format!("key {:?}", self.0)
    }
}

#[derive(Hash)]
struct Click {
    x: u32,
    y: u32,
}

impl Event for Click {
    fn describe(&self) -> String {
        format!("click at {},{}", self.x, self.y)
    }
}

// Box<dyn DynEvent> is Hash, so structs containing trait objects can derive
// Hash again.
#[derive(Hash)]
struct Logged {
    tick: u64,
    event: Box<dyn DynEvent>,
}

fn fingerprint<T: Hash + ?Sized>(value: &T) -> u64 {
    BuildHasherDefault::<DefaultHasher>::default().hash_one(value)
}

fn main() {
    let log: Vec<Logged> = vec![
        Logged {
            tick: 1,
            event: Box::new(KeyPress('q')),
        },
        Logged {
            tick: 2,
            event: Box::new(Click { x: 10, y: 20 }),
        },
    ];

    for entry in &log {
        println!(
            "{:016x} tick {}: {}",
            fingerprint(entry),
            entry.tick,
            entry.event.describe()
        );
    }

    // Hashing through the trait object matches hashing the concrete value.
    assert_eq!(fingerprint(&*log[0].event), fingerprint(&KeyPress('q')));
}
