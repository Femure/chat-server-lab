# Rust lab work - chat service
  
## Setup

* install Rust
* make sure everything works by running `cargo test`

## Part 1, network protocol

In the `chatproto/src/netproto` directory, edit `decode.rs` and `encode.rs`. The network protocol
work in the following way:

* structs are encoded by encoding each field, in the order they appear in the definition
* enums are encoded by first inserting a byte designing the variant we are in (starting at 0),
   and then encoding the enum fields
* numbers are encoded in the following way:
  * with `u < 251`, encode it as a single byte with that value,
  * with `251 <= u < 2**16`, encode it as `251` followed by a little-endian `u16` value,
  * with `2**16 <= u < 2**32`, encode it as `252` followed by a little-endian `u32` value,
  * with `2**32 <= u < 2**64`, encode it as `253` followed by a little-endian `u64` value,
  * with `2**64 <= u < 2**128`, encode it as `254` followed by a little-endian `u128` value.
* collections are encoded by first putting their sizes, and they the contents

You can test your implementation by running:

```shell
cargo test netproto
```

It is suggested to implement the encoding/decoding pair of functions in the order they are presented.

## Part 2, simple server logic

Before starting, do the following:

* rename the `sample.rs` file using a name unique to your group;
* in the renamed file, edit the `GROUP_NAME` constant and put your names in it.

You will only have to edit the renamed file, do not edit any other file! You will be graded on that file,
and you should only send this file to the course teacher.

You will have to find fill the `Server` structure with data structures that will allow you to write the
traits methods. You will most likely want to use `RwLock` (the one from `async_std::sync::RwLock`).

## Part 3, federation

Run tests and executables with the `-F federation` flag.

## Running the client and server

### Server

```shell
$ RUST_LOG=debug cargo run --bin server
...
```

### Client

```shell
$ RUST_LOG=debug cargo run --bin client -- --name my_name
...
```

On a crée une branche avec une version détaillée des fonctions écrites dans `descamps_femery`. Enjoy ;)