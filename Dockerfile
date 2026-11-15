# Container image for the multiplayer game server, for hosting services such
# as Render, Fly.io or Railway. Browsers connect over WebSockets on $PORT.
#
# Build and run locally:
#   docker build -t subhunt-server .
#   docker run -p 8080:8080 subhunt-server

FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p subhunt-server

FROM debian:bookworm-slim
COPY --from=build /src/target/release/subhunt-server /usr/local/bin/subhunt-server
# Most hosts set PORT themselves; 8080 is the fallback.
ENV PORT=8080
EXPOSE 8080
CMD ["subhunt-server"]
