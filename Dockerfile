FROM rust:1.47 as build

WORKDIR /usr/app
COPY . .
# TODO: this fails with some weird error at `heroku container:push worker` (see the bottom of the file)
# COPY .env Cargo.lock Cargo.toml veebot veebot-cmd ./

RUN cargo build --release

FROM debian:10-slim

COPY --from=build /usr/app/target/release/veebot ./veebot

RUN apt-get update


# - `youtube-dl` - used by `serenity` to get audio stream from youtube
# - `python` - required for `youtube-dl`
# - `ffmpeg` - used by `serenity` to further process `youtube-dl` stream
# - `opus-tools` - required to get some shared library (@Veetaha doesn't recall which one exactly)
#
# I also had to install `dh-autoreconf` on my personal laptop, but it is already installed
# in this docker setup (I guess?)

RUN apt-get install -y curl opus-tools ffmpeg python
RUN curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
RUN chmod a+rx /usr/local/bin/youtube-dl

CMD ["./veebot"]

# Output of the bug when using COPY with enumerated files:

# ~/dev/veebot (master) $ heroku container:push worker
# === Building worker (/home/veetaha/dev/veebot/Dockerfile)
# Sending build context to Docker daemon  465.4kB
# Step 1/11 : FROM rust:1.47 as build
#  ---> 2f75dad0e7a5
# Step 2/11 : WORKDIR /usr/app
#  ---> Using cache
#  ---> 67678c06ba07
# Step 3/11 : COPY .env Cargo.lock Cargo.toml veebot veebot-cmd ./
#  ---> Using cache
#  ---> 0ca313699cbd
# Step 4/11 : RUN cargo build --release
#  ---> Running in eb9ca44a83e8
#     Updating crates.io index
#  Downloading crates ...
#   Downloaded unicode-xid v0.2.1
#   Downloaded proc-macro2 v1.0.24
#   Downloaded quote v1.0.7
#   Downloaded syn v1.0.48
#    Compiling proc-macro2 v1.0.24
#    Compiling unicode-xid v0.2.1
#    Compiling syn v1.0.48
#    Compiling quote v1.0.7
#    Compiling veebot-cmd v0.1.0 (/usr/app)
# error[E0433]: failed to resolve: could not find `FnArg` in `syn`
#   --> src/lib.rs:28:22
#    |
# 28 |                 syn::FnArg::Receiver(_) => unreachable!(),
#    |                      ^^^^^ could not find `FnArg` in `syn`

# error[E0433]: failed to resolve: could not find `FnArg` in `syn`
#   --> src/lib.rs:29:22
#    |
# 29 |                 syn::FnArg::Typed(it) => it,
#    |                      ^^^^^ could not find `FnArg` in `syn`

# error[E0412]: cannot find type `ItemFn` in crate `syn`
#   --> src/lib.rs:17:60
#    |
# 17 |     let mut fn_item = syn::parse_macro_input!(item as syn::ItemFn);
#    |                                                            ^^^^^^ not found in `syn`

# error: aborting due to 3 previous errors

# Some errors have detailed explanations: E0412, E0433.
# For more information about an error, try `rustc --explain E0412`.
# error: could not compile `veebot-cmd`.

# To learn more, run the command again with --verbose.
# The command '/bin/sh -c cargo build --release' returned a non-zero code: 101
#  ▸    Error: docker build exited with Error: 101
