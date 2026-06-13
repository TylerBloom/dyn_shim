// Several methods declare lifetimes that could be elided; they are explicit on
// purpose, to exercise lifetime forwarding.
#![allow(clippy::needless_lifetimes)]

use dyn_shim::{dyn_shim, dyn_shim_foreign};
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

// Bounds on the shim's name become its supertraits (and bounds on the blanket
// impl): an auto trait lets the trait object cross threads, and a re-added
// dyn-compatible supertrait is callable on the `dyn` type.
#[dyn_shim(DynTask: Send + Display)]
trait Task: Display {
    fn id(&self) -> i32;
}

struct Job(i32);
impl std::fmt::Display for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "job#{}", self.0)
    }
}
impl Task for Job {
    fn id(&self) -> i32 {
        self.0
    }
}

#[test]
fn bounds_on_shim() {
    let t: Box<dyn DynTask> = Box::new(Job(4));
    assert_eq!(format!("{t}"), "job#4");
    assert_eq!(std::thread::spawn(move || t.id()).join().unwrap(), 4);
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

// A recognized `Clone` bound is not a supertrait of the shim (that would
// break dyn-compatibility). It becomes a bound on the blanket impl plus
// hidden machinery that makes the shim's boxed trait objects cloneable.
#[dyn_shim(DynSticker: Clone)]
trait Sticker {
    fn label(&self) -> String;
    fn relabel(&mut self, to: &str);
}

#[derive(Clone)]
struct Tag(String);
impl Sticker for Tag {
    fn label(&self) -> String {
        self.0.clone()
    }
    fn relabel(&mut self, to: &str) {
        self.0 = to.into();
    }
}

#[test]
fn clone_bound_clones_box() {
    let original: Box<dyn DynSticker> = Box::new(Tag("a".into()));
    let mut copy = original.clone();
    copy.relabel("b");
    assert_eq!(original.label(), "a");
    assert_eq!(copy.label(), "b");
}

// The hidden clone machinery dispatches through `Clone` by name, so it works
// even when the source trait declares its own method called `clone`. The two
// never collide: the box's `Clone` impl copies the value, the shim method
// forwards to the implementor's inherent trait method.
#[dyn_shim(DynRevision: Clone)]
trait Revision {
    fn clone(&self) -> u32; // domain method, not std `Clone`
}

#[derive(Clone)]
struct Doc(u32);
impl Revision for Doc {
    fn clone(&self) -> u32 {
        self.0
    }
}

#[test]
fn clone_bound_with_clone_named_method() {
    let original: Box<dyn DynRevision> = Box::new(Doc(7));
    let copy = std::clone::Clone::clone(&original);
    assert_eq!(DynRevision::clone(&*copy), 7);
}

// Recognized bounds compose with the rest of the list: auto traits stay
// supertraits of the shim and additionally select which `dyn` marker
// combinations get the `Clone` and `Hash` machinery, in any order, alongside
// ordinary method skipping.
#[allow(dead_code)]
#[dyn_shim(DynColor: Hash + Send + Clone + Sync)]
trait Color {
    fn rgb(&self) -> (u8, u8, u8);
    fn mix(&self, other: Self) -> Self; // skipped: mentions Self
}

#[derive(Clone, Hash)]
struct Red;
impl Color for Red {
    fn rgb(&self) -> (u8, u8, u8) {
        (255, 0, 0)
    }
    fn mix(&self, _other: Self) -> Self {
        Red
    }
}

#[test]
fn clone_covers_marker_combinations() {
    let plain: Box<dyn DynColor> = Box::new(Red);
    assert_eq!(plain.clone().rgb(), (255, 0, 0));

    // The clone keeps the marker, so it can cross the thread boundary.
    let send: Box<dyn DynColor + Send> = Box::new(Red);
    let copy = send.clone();
    assert_eq!(
        std::thread::spawn(move || copy.rgb()).join().unwrap(),
        (255, 0, 0)
    );

    let sync: Box<dyn DynColor + Sync> = Box::new(Red);
    assert_eq!(sync.clone().rgb(), (255, 0, 0));

    // Marker order in the type is irrelevant; one impl covers each subset.
    let both: Box<dyn Sync + Send + DynColor> = Box::new(Red);
    assert_eq!(both.clone().rgb(), (255, 0, 0));
}

// The recognized auto traits are not limited to Send and Sync; any listed
// std auto trait selects marker combinations.
#[dyn_shim(DynGauge: Clone + Unpin)]
trait Gauge {
    fn level(&self) -> i32;
}

#[derive(Clone)]
struct Dial(i32);
impl Gauge for Dial {
    fn level(&self) -> i32 {
        self.0
    }
}

#[test]
fn clone_covers_listed_unpin_marker() {
    let pinned: Box<dyn DynGauge + Unpin> = Box::new(Dial(7));
    assert_eq!(pinned.clone().level(), 7);
}

#[test]
fn hash_matches_concrete_value() {
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
    let bh = BuildHasherDefault::<DefaultHasher>::default();

    let boxed: Box<dyn DynColor> = Box::new(Red);
    let by_ref: &dyn DynColor = &Red;
    assert_eq!(bh.hash_one(&*boxed), bh.hash_one(&Red));
    assert_eq!(bh.hash_one(by_ref), bh.hash_one(&Red));
    // Box<dyn DynColor> hashes via std's forwarding impl for Box<T: Hash>.
    assert_eq!(bh.hash_one(&boxed), bh.hash_one(&Red));
    // Marker-variant dyn types hash through their own impls.
    let marked: &(dyn DynColor + Send + Sync) = &Red;
    assert_eq!(bh.hash_one(marked), bh.hash_one(&Red));
}

#[test]
fn to_owned_escapes_borrow() {
    use std::borrow::Cow;

    let concrete = Red;
    let borrowed: &dyn DynColor = &concrete;
    // `.clone()` on a `&dyn` would copy the reference; `.to_owned()` returns
    // an owned box.
    let owned: Box<dyn DynColor> = borrowed.to_owned();
    assert_eq!(owned.rgb(), (255, 0, 0));

    let cow: Cow<'_, dyn DynColor> = Cow::Borrowed(borrowed);
    let owned: Box<dyn DynColor> = cow.into_owned();
    assert_eq!(owned.rgb(), (255, 0, 0));

    // Marker variants get `ToOwned` too.
    let borrowed: &(dyn DynColor + Send + Sync) = &concrete;
    let owned: Box<dyn DynColor + Send + Sync> = borrowed.to_owned();
    assert_eq!(
        std::thread::spawn(move || owned.rgb()).join().unwrap(),
        (255, 0, 0)
    );
}

// Duplicate bounds are harmless, deduplicated like the language itself
// dedupes `trait Foo: A + A`: the machinery and marker combos are generated
// once.
#[dyn_shim(DynBadge: Clone + Send + Clone + Send)]
trait Badge {
    fn number(&self) -> u32;
}

#[derive(Clone)]
struct Lanyard(u32);
impl Badge for Lanyard {
    fn number(&self) -> u32 {
        self.0
    }
}

#[test]
fn duplicate_bounds_dedupe() {
    let badge: Box<dyn DynBadge + Send> = Box::new(Lanyard(3));
    assert_eq!(badge.clone().number(), 3);
}

// A pass-through bound whose trait has associated types works when the
// `dyn` type binds them at the use site. The bound mirrors the source
// trait's supertrait, re-adding it to the shim.
#[dyn_shim(DynSamples: Iterator)]
trait Samples: Iterator {
    fn label(&self) -> String;
}

struct Ramp(u8);
impl Iterator for Ramp {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        self.0 += 1;
        Some(self.0)
    }
}
impl Samples for Ramp {
    fn label(&self) -> String {
        "ramp".into()
    }
}

#[test]
fn assoc_type_bound_binds_at_use_site() {
    let mut source: Box<dyn DynSamples<Item = u8>> = Box::new(Ramp(0));
    assert_eq!(source.label(), "ramp");
    let head: Vec<u8> = source.by_ref().take(3).collect();
    assert_eq!(head, [1, 2, 3]);
}

// `#[dyn_shim_foreign]` shims a trait the annotating crate does not own. The
// module here stands in for a dependency: the macro cannot read its body, so
// the annotated trait restates the signatures to forward (and is itself
// discarded, not re-emitted). The blanket impl forwards to the foreign path.
mod thirdparty {
    #[allow(dead_code)]
    pub trait Channel {
        fn connect() -> Self; // receiverless; not restated below
        fn label(&self) -> String;
        fn deliver(&mut self, message: &str) -> usize;
        fn close(self) -> usize; // by-value self
    }
}

struct Email(usize);
impl thirdparty::Channel for Email {
    fn connect() -> Self {
        Email(0)
    }
    fn label(&self) -> String {
        "email".into()
    }
    fn deliver(&mut self, _message: &str) -> usize {
        self.0 += 1;
        self.0
    }
    fn close(self) -> usize {
        self.0
    }
}

// The annotated trait is the shim; only the dyn-compatible methods to forward
// are listed (the receiverless `connect` is omitted). The foreign trait's path
// is the attribute argument.
#[dyn_shim_foreign(thirdparty::Channel)]
trait DynChannel {
    fn label(&self) -> String;
    fn deliver(&mut self, message: &str) -> usize;
    fn close(self) -> usize; // by-value -> self: Box<Self>
}

#[test]
fn foreign_trait_shimmed() {
    let mut ch: Box<dyn DynChannel> = Box::new(Email(0));
    assert_eq!(ch.label(), "email");
    assert_eq!(ch.deliver("hi"), 1);
    assert_eq!(ch.deliver("again"), 2);
    assert_eq!(ch.close(), 2); // by-value self forwarded through Box<Self>
}

// Recognized bounds, marker combinations, and supertraits work through the
// foreign form exactly as the local one: `Clone` makes the boxed trait
// objects cloneable, `Send` selects the marker variant, and a path to the
// foreign trait carrying generic arguments is forwarded verbatim.
mod widgets {
    pub trait Paint<C> {
        fn color(&self) -> C;
    }
}

#[derive(Clone)]
struct Square;
impl widgets::Paint<u8> for Square {
    fn color(&self) -> u8 {
        7
    }
}

#[dyn_shim_foreign(widgets::Paint<u8>)]
trait DynPaint: Clone + Send {
    fn color(&self) -> u8;
}

#[test]
fn foreign_recognized_bound_and_markers() {
    let plain: Box<dyn DynPaint> = Box::new(Square);
    assert_eq!(plain.clone().color(), 7);

    let send: Box<dyn DynPaint + Send> = Box::new(Square);
    let copy = send.clone();
    assert_eq!(std::thread::spawn(move || copy.color()).join().unwrap(), 7);
}
