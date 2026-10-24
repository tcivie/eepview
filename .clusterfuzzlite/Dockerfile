# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

# The ClusterFuzzLite build image for Rust on Ubuntu 24.04, pinned by digest (tag
# ubuntu-24-04 on 2026-10-03). The Tauri crates need libwebkit2gtk-4.1, which Ubuntu 20.04 lacks.
# Dependabot bumps the digest.
FROM gcr.io/oss-fuzz-base/base-builder-rust:ubuntu-24-04@sha256:bceca1feaa6ee67cd11a4ac6104b754afb6da7f0a81d191212aff1d678c71f21

# The system libraries that the Tauri crates link against, so that eepview_lib builds.
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . $SRC/eepview
WORKDIR $SRC/eepview
COPY .clusterfuzzlite/build.sh $SRC/build.sh
