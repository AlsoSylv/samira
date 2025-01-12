# Irelia

[![Crates.io Total Downloads](https://img.shields.io/crates/d/irelia?style=flat-square)](https://crates.io/crates/irelia)
[![GitHub Repo stars](https://img.shields.io/github/stars/alsosylv/irelia?style=flat-square)](https://github.com/AlsoSylv/Irelia/stargazers)
[![Crates.io Version](https://img.shields.io/crates/v/irelia?style=flat-square)](https://crates.io/crates/irelia/versions)
[![docs.rs](https://img.shields.io/docsrs/irelia?style=flat-square)](https://docs.rs/irelia)

**Irelia is a wrapper for the local https APIs provided by riot games for LoL**

---

```toml
[dependencies]
irelia = "0.9"
```

### Cargo Features

---
This crate is designed with modularity in mind, and as such API support has been split into different cargo features.

By default, everything but the replay feature is enabled

- `["full"]` - enables support for all APIs avaiable in the client by default (enabled by default)
- `["ws"]` - enables support for the LCU websocket
- `["in_game"]` - enables support for the native in game API
- `["replay"]` - enables the replay API interface (disabled by default)

### Making a request to the LCU

---
Making a request to the LCU with irelia is simple

```rust
use irelia::{Error, RequestClient, rest::LcuClient};
use serde_json::Value;

#[tokio::main]
async fn main() {
    let lcu_client = irelia::rest::LcuClient::connect().unwrap();

    let current_summoner: Value = lcu_client
        .get("/lol-summoner/v1/current-summoner")
        .await
        .unwrap();

    println!("{current_summoner}");
}
```

Up-to-date examples can always be found [here](irelia/examples)
