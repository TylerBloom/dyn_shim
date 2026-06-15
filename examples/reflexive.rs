//! Letting erased shim objects flow back into `Rule`-generic code with both
//! reflexive forms.
//!
//! `Rule` is not dyn-compatible: `threshold` is generic over its return type,
//! so it cannot enter a vtable. `#[dyn_shim(DynRule)]` generates a
//! dyn-compatible `DynRule` shim for the dispatchable methods, which is what a
//! mixed `Vec<Box<dyn DynRule>>` holds. But `DynRule` is a *separate* trait, so
//! on its own a `dyn DynRule` is not a `Rule`, and existing `Rule`-generic
//! functions cannot take one.
//!
//! `reflexive = bare + boxed` bridges that gap from both directions:
//!
//! - `bare` emits `impl Rule for dyn DynRule`, so a borrow (`&dyn DynRule`)
//!   satisfies `Rule` by reference.
//! - `boxed` emits `impl Rule for Box<dyn DynRule>`, so an owned box satisfies
//!   `Rule` by value.
//!
//! `threshold` cannot forward through the shim, so it is opted into a panicking
//! stub with `#[dyn_shim(panic)]`. Call it on a concrete rule, before erasing.
//!
//! Run with: `cargo run --example reflexive`

use dyn_shim::dyn_shim;

#[dyn_shim(DynRule, reflexive = bare + boxed)]
trait Rule {
    fn name(&self) -> &str;
    fn check(&self, value: i32) -> bool;
    fn tighten(&mut self, by: i32);
    // Generic over the return type, so not dyn-compatible: it cannot forward
    // through the shim. The reflexive impls provide a panicking stub instead.
    #[dyn_shim(panic)]
    fn threshold<T: From<i32>>(&self) -> T;
}

struct AtLeast {
    floor: i32,
}
impl Rule for AtLeast {
    fn name(&self) -> &str {
        "at_least"
    }
    fn check(&self, value: i32) -> bool {
        value >= self.floor
    }
    fn tighten(&mut self, by: i32) {
        self.floor += by;
    }
    fn threshold<T: From<i32>>(&self) -> T {
        T::from(self.floor)
    }
}

struct Even;
impl Rule for Even {
    fn name(&self) -> &str {
        "even"
    }
    fn check(&self, value: i32) -> bool {
        value % 2 == 0
    }
    fn tighten(&mut self, _by: i32) {}
    fn threshold<T: From<i32>>(&self) -> T {
        T::from(0)
    }
}

// Generic over `Rule` by reference. A `&dyn DynRule` satisfies the bound
// through the bare reflexive impl, so it forwards without an allocation.
fn passes<R: Rule + ?Sized>(rule: &R, value: i32) -> bool {
    rule.check(value)
}

// Generic over `Rule` by value. A `Box<dyn DynRule>` satisfies the bound
// through the boxed reflexive impl, so the owned object can be consumed here.
fn into_name(rule: impl Rule) -> String {
    rule.name().to_string()
}

fn main() {
    let mut rules: Vec<Box<dyn DynRule>> = vec![Box::new(AtLeast { floor: 10 }), Box::new(Even)];

    // With both reflexive impls in scope, a shim object carries `tighten` from
    // `DynRule` and from `Rule`, so the call is qualified to the shim.
    for rule in rules.iter_mut() {
        DynRule::tighten(&mut **rule, 1);
    }

    // bare: `&**rule` is a `&dyn DynRule`, accepted as a `&impl Rule`.
    let value = 12;
    for rule in &rules {
        let name = DynRule::name(&**rule);
        println!("{name}: {value} passes = {}", passes(&**rule, value));
    }

    // boxed: each `Box<dyn DynRule>` is an owned `impl Rule`, consumed by value.
    for rule in rules {
        println!("consumed rule named {}", into_name(rule));
    }

    // `threshold` is generic, so it lives only on concrete rules. Calling it on
    // a `dyn DynRule` or `Box<dyn DynRule>` would hit the panicking stub.
    let floor: i64 = AtLeast { floor: 11 }.threshold();
    println!("concrete threshold: {floor}");
}
