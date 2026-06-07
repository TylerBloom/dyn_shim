use dyn_shim::dyn_shim;
use std::fmt::Display;

// ---------------------------------------------------------------------------
// A deliberately NON-dyn-compatible "foreign" trait. It contains one of each
// problem class discussed: by-value `self`, receiverless `-> Self`,
// receiverless `-> i32`, plus several forwardable shapes.
// ---------------------------------------------------------------------------
#[allow(dead_code)]
trait SizedTrait {
    fn name(&self) -> String;                 // &self, no args
    fn bump(&mut self, by: i32);              // &mut self
    fn add(&self, a: i32, b: i32) -> i32;     // &self, multi-arg
    fn drain(&mut self) -> String;            // &mut self, returns value
    fn consume(self, suffix: &str) -> String; // by-value -> Box<Self> in shim
    fn tagged<'a>(&self, tag: &'a str) -> String where 'a: 'a; // lifetime + where
    fn show<'a>(&self, x: &'a dyn Display) -> String;          // lifetime, no where
    fn make() -> Self;                        // receiverless -> Self (omitted)
    fn answer() -> i32 { 42 }                 // receiverless -> i32  (omitted)
}

struct Foo(i32);
struct Bar(String);

impl SizedTrait for Foo {
    fn name(&self) -> String { format!("Foo({})", self.0) }
    fn bump(&mut self, by: i32) { self.0 += by; }
    fn add(&self, a: i32, b: i32) -> i32 { self.0 + a + b }
    fn drain(&mut self) -> String { let s = format!("drained {}", self.0); self.0 = 0; s }
    fn consume(self, suffix: &str) -> String { format!("Foo gone:{}{}", self.0, suffix) }
    fn tagged<'a>(&self, tag: &'a str) -> String where 'a: 'a { format!("[{}]Foo{}", tag, self.0) }
    fn show<'a>(&self, x: &'a dyn Display) -> String { format!("Foo<{}>", x) }
    fn make() -> Self { Foo(0) }
}

impl SizedTrait for Bar {
    fn name(&self) -> String { format!("Bar({})", self.0) }
    fn bump(&mut self, by: i32) { self.0.push_str(&"!".repeat(by.max(0) as usize)); }
    fn add(&self, a: i32, b: i32) -> i32 { self.0.len() as i32 + a + b }
    fn drain(&mut self) -> String { let s = format!("drained {}", self.0); self.0.clear(); s }
    fn consume(self, suffix: &str) -> String { format!("Bar gone:{}{}", self.0, suffix) }
    fn tagged<'a>(&self, tag: &'a str) -> String where 'a: 'a { format!("[{}]Bar{}", tag, self.0) }
    fn show<'a>(&self, x: &'a dyn Display) -> String { format!("Bar<{}>", x) }
    fn make() -> Self { Bar(String::new()) }
}

// Generate the dyn-compatible shim. `make` and `answer` are omitted (receiverless).
dyn_shim! {
    trait DynTrait for SizedTrait {
        fn name(&self) -> String;
        fn bump(&mut self, by: i32);
        fn add(&self, a: i32, b: i32) -> i32;
        fn drain(&mut self) -> String;
        fn consume(self: Box<Self>, suffix: &str) -> String;
        fn tagged<'a>(&self, tag: &'a str) -> String [where 'a: 'a];
        fn show<'a>(&self, x: &'a dyn Display) -> String;
    }
}

// --- &self, no args ---
#[test]
fn shared_ref_dispatch() {
    let r: &dyn DynTrait = &Foo(7);
    assert_eq!(r.name(), "Foo(7)");
}

// --- &self, multiple args, value return ---
#[test]
fn multi_arg() {
    let r: &dyn DynTrait = &Foo(10);
    assert_eq!(r.add(3, 4), 17);
}

// --- &mut self, no return ---
#[test]
fn mut_ref_dispatch() {
    let mut b = Bar("x".into());
    let r: &mut dyn DynTrait = &mut b;
    r.bump(3);
    assert_eq!(r.name(), "Bar(x!!!)");
}

// --- &mut self, with return value ---
#[test]
fn mut_ref_returns_value() {
    let mut f = Foo(5);
    let r: &mut dyn DynTrait = &mut f;
    assert_eq!(r.drain(), "drained 5");
    assert_eq!(r.name(), "Foo(0)");
}

// --- self: Box<Self>, consuming through the box ---
#[test]
fn box_self_consume() {
    let boxed: Box<dyn DynTrait> = Box::new(Foo(9));
    assert_eq!(boxed.consume("-end"), "Foo gone:9-end");
}

// --- lifetime param + where clause ---
#[test]
fn lifetime_where_method() {
    let r: &dyn DynTrait = &Bar("z".into());
    assert_eq!(r.tagged("T"), "[T]Barz");
}

// --- lifetime param, no where clause, trait-object argument ---
#[test]
fn lifetime_no_where() {
    let r: &dyn DynTrait = &Foo(1);
    assert_eq!(r.show(&99), "Foo<99>");
    assert_eq!(r.show(&"hi"), "Foo<hi>");
}

// --- the payoff: heterogeneous witnesses behind one erased type ---
#[test]
fn heterogeneous_existential() {
    let mut zoo: Vec<Box<dyn DynTrait>> = vec![
        Box::new(Foo(1)),
        Box::new(Bar("hi".into())),
        Box::new(Foo(100)),
    ];
    for item in zoo.iter_mut() {
        item.bump(1);
    }
    let names: Vec<String> = zoo.iter().map(|x| x.name()).collect();
    assert_eq!(names, vec!["Foo(2)", "Bar(hi!)", "Foo(101)"]);

    let out: Vec<String> = zoo.into_iter().map(|x| x.consume("#")).collect();
    assert_eq!(out, vec!["Foo gone:2#", "Bar gone:hi!#", "Foo gone:101#"]);
}

// --- shim object stored and dispatched via &mut behind a struct field ---
#[test]
fn stored_behind_field() {
    struct Holder { inner: Box<dyn DynTrait> }
    let mut h = Holder { inner: Box::new(Bar("q".into())) };
    h.inner.bump(2);
    assert_eq!(h.inner.name(), "Bar(q!!)");
}
