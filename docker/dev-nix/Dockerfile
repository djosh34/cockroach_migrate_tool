FROM busybox:1.36.1-musl AS busybox

FROM nixos/nix:2.32.4

COPY --from=busybox /bin/busybox /usr/local/bin/busybox
COPY entrypoint.sh /usr/local/bin/dev-nix-entrypoint

RUN mkdir -p /etc/nix /opt/dev-nix-seed /home/dev \
    && cat > /etc/nix/nix.conf <<'EOF'
experimental-features = nix-command flakes
accept-flake-config = true
allowed-users = *
trusted-users = *
EOF

RUN ln -s "$(readlink -f /root/.nix-profile/bin/nix)" /usr/local/bin/nix \
    && mkdir -p /usr/local/share/dev-nix \
    && ln -s "$(readlink -f /nix/var/nix/profiles/default/etc/ssl/certs/ca-bundle.crt)" /usr/local/share/dev-nix/ca-bundle.crt

RUN chmod 0555 /usr/local/bin/busybox /usr/local/bin/dev-nix-entrypoint

ENTRYPOINT ["/usr/local/bin/busybox", "sh", "/usr/local/bin/dev-nix-entrypoint"]
WORKDIR /workspace
