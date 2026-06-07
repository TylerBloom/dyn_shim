//! Broadcasting a message to a heterogeneous set of notification channels.
//!
//! `Channel` (below) is an ordinary trait, but it is NOT dyn-compatible. It has
//! a by-value `close(self)` shutdown method and a receiverless `connect() ->
//! Self` constructor. Either one alone is enough to make `Box<dyn Channel>`
//! illegal, so you cannot keep a mixed list of channel types at runtime.
//!
//! `dyn_shim!` generates a dyn-compatible `DynChannel` shim plus a blanket impl,
//! forwarding only the methods that make sense behind a trait object. The
//! by-value `close(self)` is exposed as `close(self: Box<Self>)`, and the
//! receiverless `connect` is simply left out (you call it on the concrete
//! type).

use dyn_shim::dyn_shim;

trait Channel {
    fn connect() -> Self;
    fn label(&self) -> String;
    fn set_prefix(&mut self, prefix: &str);
    fn deliver(&mut self, message: &str);
    fn close(self) -> u32;
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

dyn_shim! {
    trait DynChannel for Channel {
        fn label(&self) -> String;
        fn set_prefix(&mut self, prefix: &str);
        fn deliver(&mut self, message: &str);
        fn close(self: Box<Self>) -> u32;  // forwards to Channel::close(*self)
    }
}

fn main() {
    // Now we have a DynChannel which *is* dyn-compatible.
    let mut channels: Vec<Box<dyn DynChannel>> =
        vec![Box::new(Email::connect()), Box::new(Webhook::connect())];

    for ch in channels.iter_mut() {
        ch.set_prefix("[staging] ");
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
