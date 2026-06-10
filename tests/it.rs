// Several methods declare lifetimes that could be elided; they are explicit on
// purpose, to exercise lifetime forwarding.
#![allow(clippy::needless_lifetimes)]

use dyn_shim::dyn_shim;
use std::fmt::Display;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

#[allow(dead_code)]
#[dyn_shim(DynTrait)]
trait SizedTrait {
    fn name(&self) -> String;
    fn bump(&mut self, by: i32);
    fn add(&self, a: i32, b: i32) -> i32;
    fn drain(&mut self) -> String;
    fn consume(self, suffix: &str) -> String;
    fn tagged<'a>(&self, tag: &'a str) -> String
    where
        'a: 'a;
    fn show<'a>(&self, x: &'a dyn Display) -> String;
    fn make() -> Self;
    fn answer() -> i32;
}

struct Foo(i32);
struct Bar(String);

impl SizedTrait for Foo {
    fn name(&self) -> String {
        format!("Foo({})", self.0)
    }
    fn bump(&mut self, by: i32) {
        self.0 += by;
    }
    fn add(&self, a: i32, b: i32) -> i32 {
        self.0 + a + b
    }
    fn drain(&mut self) -> String {
        let s = format!("drained {}", self.0);
        self.0 = 0;
        s
    }
    fn consume(self, suffix: &str) -> String {
        format!("Foo gone:{}{}", self.0, suffix)
    }
    fn tagged<'a>(&self, tag: &'a str) -> String
    where
        'a: 'a,
    {
        format!("[{}]Foo{}", tag, self.0)
    }
    fn show<'a>(&self, x: &'a dyn Display) -> String {
        format!("Foo<{}>", x)
    }
    fn make() -> Self {
        Foo(0)
    }
    fn answer() -> i32 {
        42
    }
}

impl SizedTrait for Bar {
    fn name(&self) -> String {
        format!("Bar({})", self.0)
    }
    fn bump(&mut self, by: i32) {
        self.0.push_str(&"!".repeat(by.max(0) as usize));
    }
    fn add(&self, a: i32, b: i32) -> i32 {
        self.0.len() as i32 + a + b
    }
    fn drain(&mut self) -> String {
        let s = format!("drained {}", self.0);
        self.0.clear();
        s
    }
    fn consume(self, suffix: &str) -> String {
        format!("Bar gone:{}{}", self.0, suffix)
    }
    fn tagged<'a>(&self, tag: &'a str) -> String
    where
        'a: 'a,
    {
        format!("[{}]Bar{}", tag, self.0)
    }
    fn show<'a>(&self, x: &'a dyn Display) -> String {
        format!("Bar<{}>", x)
    }
    fn make() -> Self {
        Bar(String::new())
    }
    fn answer() -> i32 {
        42
    }
}

#[test]
fn shared_ref_dispatch() {
    let r: &dyn DynTrait = &Foo(7);
    assert_eq!(r.name(), "Foo(7)");
}

#[test]
fn multi_arg() {
    let r: &dyn DynTrait = &Foo(10);
    assert_eq!(r.add(3, 4), 17);
}

#[test]
fn mut_ref_dispatch() {
    let mut b = Bar("x".into());
    let r: &mut dyn DynTrait = &mut b;
    r.bump(3);
    assert_eq!(r.name(), "Bar(x!!!)");
}

#[test]
fn mut_ref_returns_value() {
    let mut f = Foo(5);
    let r: &mut dyn DynTrait = &mut f;
    assert_eq!(r.drain(), "drained 5");
    assert_eq!(r.name(), "Foo(0)");
}

#[test]
fn box_self_consume() {
    let boxed: Box<dyn DynTrait> = Box::new(Foo(9));
    assert_eq!(boxed.consume("-end"), "Foo gone:9-end");
}

#[test]
fn lifetime_where_method() {
    let r: &dyn DynTrait = &Bar("z".into());
    assert_eq!(r.tagged("T"), "[T]Barz");
}

#[test]
fn lifetime_no_where() {
    let r: &dyn DynTrait = &Foo(1);
    assert_eq!(r.show(&99), "Foo<99>");
    assert_eq!(r.show(&"hi"), "Foo<hi>");
}

#[test]
fn mixed_box_collection() {
    let mut zoo: Vec<Box<dyn DynTrait>> = vec![Box::new(Foo(100)), Box::new(Bar("hi".into()))];
    for item in zoo.iter_mut() {
        item.bump(1);
    }
    let names: Vec<String> = zoo.iter().map(|x| x.name()).collect();
    assert_eq!(names, vec!["Foo(101)", "Bar(hi!)"]);

    let out: Vec<String> = zoo.into_iter().map(|x| x.consume("#")).collect();
    assert_eq!(out, vec!["Foo gone:101#", "Bar gone:hi!#"]);
}

// Every dispatchable receiver type is forwarded into the shim: `&self`,
// `&mut self`, and the explicit smart-pointer receivers `Box<Self>`,
// `Rc<Self>`, `Arc<Self>`, and `Pin<&mut Self>`. An explicit `self: Self`
// is the typed spelling of by-value `self` and is rewritten to
// `self: Box<Self>` just like the shorthand.
#[dyn_shim(DynRecv)]
trait Receivers {
    fn by_ref(&self) -> i32;
    fn by_mut(&mut self) -> i32;
    fn by_box(self: Box<Self>) -> i32;
    fn by_rc(self: Rc<Self>) -> i32;
    fn by_arc(self: Arc<Self>) -> i32;
    fn by_pin(self: Pin<&mut Self>) -> i32;
    fn by_self(self: Self) -> i32;
}

struct Recv(i32);
impl Receivers for Recv {
    fn by_ref(&self) -> i32 {
        self.0
    }
    fn by_mut(&mut self) -> i32 {
        self.0 += 1;
        self.0
    }
    fn by_box(self: Box<Self>) -> i32 {
        self.0 + 10
    }
    fn by_rc(self: Rc<Self>) -> i32 {
        self.0 + 20
    }
    fn by_arc(self: Arc<Self>) -> i32 {
        self.0 + 30
    }
    fn by_pin(self: Pin<&mut Self>) -> i32 {
        self.0 + 40
    }
    fn by_self(self) -> i32 {
        self.0 + 50
    }
}

#[test]
fn ref_receivers() {
    let r: &dyn DynRecv = &Recv(1);
    assert_eq!(r.by_ref(), 1);

    let mut owned = Recv(1);
    let m: &mut dyn DynRecv = &mut owned;
    assert_eq!(m.by_mut(), 2);
}

#[test]
fn box_receiver() {
    let b: Box<dyn DynRecv> = Box::new(Recv(1));
    assert_eq!(b.by_box(), 11);
}

#[test]
fn rc_receiver() {
    let rc: Rc<dyn DynRecv> = Rc::new(Recv(1));
    assert_eq!(rc.by_rc(), 21);
}

#[test]
fn arc_receiver() {
    let arc: Arc<dyn DynRecv> = Arc::new(Recv(1));
    assert_eq!(arc.by_arc(), 31);
}

#[test]
fn pin_receiver() {
    let mut pinned: Pin<Box<dyn DynRecv>> = Box::pin(Recv(1));
    assert_eq!(pinned.as_mut().by_pin(), 41);
}

#[test]
fn explicit_self_receiver() {
    let b: Box<dyn DynRecv> = Box::new(Recv(1));
    assert_eq!(b.by_self(), 51);
}

// A source trait that is itself not dyn-compatible (it carries an associated
// const and an associated type) still yields a working shim from its
// dispatchable methods. Associated items are not copied onto the shim, and the
// method that returns the associated type is skipped because it mentions Self.
#[allow(dead_code)]
#[dyn_shim(DynAssoc)]
trait HasAssoc {
    const TAG: u8;
    type Item;
    fn label(&self) -> String;
    fn item(&self) -> Self::Item;
}

struct Assoc;
impl HasAssoc for Assoc {
    const TAG: u8 = 9;
    type Item = i32;
    fn label(&self) -> String {
        "Assoc".into()
    }
    fn item(&self) -> i32 {
        1
    }
}

#[test]
fn assoc_items_trait_shimmed() {
    let d: &dyn DynAssoc = &Assoc;
    assert_eq!(d.label(), "Assoc");
}

// A forwarded method keeps its whole signature and its attributes. `unsafe`
// and an explicit ABI are carried onto the shim, and a `#[cfg]`-gated method
// is gated identically on the source trait, the shim trait, and the blanket
// impl: `sometimes` is compiled out everywhere here, so its missing impl (and
// nonexistent argument type) must not break the build.
#[allow(dead_code)]
#[dyn_shim(DynSig)]
trait SigPreserving {
    /// Doc carried onto the shim.
    #[must_use]
    fn answer(&self) -> i32;
    unsafe fn raw(&self) -> i32;
    extern "C" fn c_abi(&self) -> i32;
    #[cfg(any())]
    fn sometimes(&self, arg: NoSuchType) -> NoSuchType;
}

struct Sig(i32);
impl SigPreserving for Sig {
    fn answer(&self) -> i32 {
        self.0
    }
    unsafe fn raw(&self) -> i32 {
        -self.0
    }
    extern "C" fn c_abi(&self) -> i32 {
        self.0 * 2
    }
}

#[test]
fn signature_and_attrs_preserved() {
    let d: Box<dyn DynSig> = Box::new(Sig(7));
    assert_eq!(d.answer(), 7);
    // The shim method is `unsafe fn`, so it must be called in an unsafe block.
    assert_eq!(unsafe { d.raw() }, -7);
    assert_eq!(d.c_abi(), 14);
}
