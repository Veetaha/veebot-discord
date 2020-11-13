FROM rust:1.47 as build

WORKDIR /usr/app
COPY .env Cargo.lock Cargo.toml veebot veebot-cmd ./

RUN cargo build --release

FROM debian:10-slim

COPY --from=build /usr/app/target/release/veebot ./veebot

RUN apt-get update

# - `youtube-dl` - used by `serenity` to get audio stream from youtube
# - `python` - required for `youtube-dl`
# - `ffmpeg` - used by `serenity` to further process `youtube-dl` stream
# - `opus-tools` - required to get some shared library (@Veetaha doesn't recall which one exactly)

RUN apt-get install -y curl opus-tools ffmpeg python
RUN curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
RUN chmod a+rx /usr/local/bin/youtube-dl

CMD ["./veebot"]
