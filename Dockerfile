FROM rust:1.47

RUN curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
RUN chmod a+rx /usr/local/bin/youtube-dl

WORKDIR /usr/app

COPY . .

CMD ["cargo", "run", "--release"]
