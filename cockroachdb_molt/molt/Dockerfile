FROM golang:1.26-bookworm AS builder

WORKDIR /workspace

COPY . .

RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-s -w" -o /out/molt .

FROM scratch AS runtime

COPY --from=builder /out/molt /usr/local/bin/molt

ENTRYPOINT ["/usr/local/bin/molt", "verify"]
