# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

# The ClusterFuzzLite build image for Rust on Ubuntu 24.04, pinned by digest (tag
# ubuntu-24-04 on 2026-10-03). The Tauri crates need libwebkit2gtk-4.1, which Ubuntu 20.04 lacks.
# Dependabot bumps the digest.
FROM gcr.io/oss-fuzz-base/base-builder-rust:ubuntu-24-04@sha256:18d964446414087d7930a1cd380786ed4387e07415b33b8dbb6a974456bb8fa1

# The system libraries that the Tauri crates link against, so that eepview_lib builds.
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . $SRC/eepview
WORKDIR $SRC/eepview
COPY .clusterfuzzlite/build.sh $SRC/build.sh
