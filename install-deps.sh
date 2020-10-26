# @Veetaha was missing only youtube-dl, it is required to be available through `$PATH`
# Here is how to install it: https://ytdl-org.github.io/youtube-dl/download.html
#
# FIXME: move this to a Dockerfile which will be deployed to heroku
#
sudo curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
sudo chmod a+rx /usr/local/bin/youtube-dl
