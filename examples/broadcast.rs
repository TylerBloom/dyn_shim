//! Broadcasting a message to a heterogeneous set of notification channels.
//!
//! `Channel` is not dyn-compatible. It has a by-value `close(self)` shutdown
//! method and a receiverless `connect() -> Self` constructor, so you cannot keep
//! a mixed list of channel types behind one `Box<dyn Channel>`.
//!
//! `#[dyn_shim(DynChannel)]` reads the trait and generates a dyn-compatible
//! `DynChannel` shim plus a blanket impl. It forwards `label`, `set_prefix`, and
//! `deliver` unchanged, rewrites the by-value `close(self)` to
//! `close(self: Box<Self>)`, and skips the receiverless `connect`.
//!
//! Run with: `cargo run --example broadcast`

use dyn_shim::dyn_shim;

#[dyn_shim(DynChannel)]
trait Channel {
    fn connect() -> Self; // receiverless: skipped
    fn label(&self) -> String;
    fn set_prefix(&mut self, prefix: &str);
    fn deliver(&mut self, message: &str);
    fn close(self) -> u32; // by-value self: forwarded as self: Box<Self>
}

struct Email {
    address: String,
    prefix: String,
    sent: u32,
}

impl Channel for Email {
    fn connect() -> Self {
        Email {
            address: "ops@example.com".into(),
            prefix: String::new(),
            sent: 0,
        }
    }
    fn label(&self) -> String {
        format!("email<{}>", self.address)
    }
    fn set_prefix(&mut self, prefix: &str) {
        self.prefix = prefix.into();
    }
    fn deliver(&mut self, message: &str) {
        println!("  [email -> {}] {}{}", self.address, self.prefix, message);
        self.sent += 1;
    }
    fn close(self) -> u32 {
        self.sent
    }
}

struct Webhook {
    url: String,
    prefix: String,
    sent: u32,
}

impl Channel for Webhook {
    fn connect() -> Self {
        Webhook {
            url: "https://hooks.example.com/abc".into(),
            prefix: String::new(),
            sent: 0,
        }
    }
    fn label(&self) -> String {
        format!("webhook<{}>", self.url)
    }
    fn set_prefix(&mut self, prefix: &str) {
        self.prefix = prefix.into();
    }
    fn deliver(&mut self, message: &str) {
        println!("  [webhook -> {}] {}{}", self.url, self.prefix, message);
        self.sent += 1;
    }
    fn close(self) -> u32 {
        self.sent
    }
}

fn main() {
    // A mixed fleet of channels behind one erased type.
    let mut channels: Vec<Box<dyn DynChannel>> =
        vec![Box::new(Email::connect()), Box::new(Webhook::connect())];

    for ch in channels.iter_mut() {
        ch.set_prefix("[prod] ");
    }

    let messages = ["deploy finished", "nightly backup completed"];
    for msg in messages {
        println!("broadcasting: {msg:?}");
        for ch in channels.iter_mut() {
            ch.deliver(msg);
        }
    }

    println!("\nshutting down:");
    for ch in channels {
        let label = ch.label();
        let sent = ch.close();
        println!("  {label} sent {sent} message(s)");
    }
}
