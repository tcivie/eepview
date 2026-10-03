# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

# The ClusterFuzzLite build image for Rust, pinned by digest (tag v1 on 2026-10-03).
# Dependabot bumps the digest.
FROM gcr.io/oss-fuzz-base/base-builder-rust:v1@sha256:5fd6a6afd7369a3277b9697b39cb44bf8ed3f389bb440e7294a3f3869891ef97

# The system libraries that the Tauri crates link against, so that eepview_lib builds.
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . $SRC/eepview
WORKDIR $SRC/eepview
COPY .clusterfuzzlite/build.sh $SRC/build.sh
