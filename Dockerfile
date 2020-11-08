FROM rust:1.47 as build

WORKDIR /usr/app
COPY . .

RUN cargo build --release

FROM debian:10-slim

COPY --from=build /usr/app/target/release/veebot ./veebot

RUN apt-get update
RUN apt-get install -y curl opus-tools
RUN curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
RUN chmod a+rx /usr/local/bin/youtube-dl

CMD ["./veebot"]
