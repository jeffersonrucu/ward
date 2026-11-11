# `make test-nm`, `test-extension` and `test-login` (Phase 65a): ubuntu:24.04 (the same
# digest as the other container tests) with Google Chrome stable at
# /opt/google/chrome/chrome and python3. No Ward policy and no native messaging manifest
# of the system: the image refuses to build if Chrome would see one. The network is only
# used in the build (Ubuntu apt and Google's Chrome repository, by HTTPS and with the
# repository key checked by fingerprint); the container runs with `--network none` and
# reaches the test services through `dev/tests/service_relay.py`.
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
# Fingerprint of the Google Linux Package Signing Key (primary key of dl.google.com/linux).
ARG GOOGLE_REPO_FINGERPRINT=EB4C1BFD4F042F6DDDCCEC917721F63BD38B4796
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
         ca-certificates curl gnupg python3 \
    && curl -q -fsSL --proto '=https' --tlsv1.2 -o /tmp/google.asc \
         https://dl.google.com/linux/linux_signing_key.pub \
    && gpg --batch --show-keys --with-colons /tmp/google.asc \
         | awk -F: '$1 == "fpr" { print $10 }' | grep -qx "$GOOGLE_REPO_FINGERPRINT" \
    && gpg --batch --dearmor -o /usr/share/keyrings/google-chrome.gpg /tmp/google.asc \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] https://dl.google.com/linux/chrome/deb/ stable main" \
         > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends google-chrome-stable \
    && apt-get purge -y curl gnupg && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/* /tmp/google.asc /etc/cron.daily/google-chrome \
    && test -x /opt/google/chrome/chrome \
    && test ! -e /etc/opt/chrome/policies && test ! -e /etc/opt/chrome/native-messaging-hosts \
    && test ! -e /etc/chromium && test ! -e /usr/local/bin/ward-nm
