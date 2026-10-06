#!/usr/bin/env bash
# Fetch the CC0 Cetotherium riabinini skeleton reconstruction (figshare
# doi:10.6084/m9.figshare.29644028, file 58189099, "Skeleton reconstruction.zip")
# and extract its STL. Checksums: the zip's MD5 is figshare's own
# `computed_md5`; the STL's SHA-256 was measured on extraction.
set -euo pipefail
dir="$1"
mkdir -p "$dir"
zip_md5=2972c9576b8f79b84159a3c317338704
stl_sha=57657cfb04e7ce01ade43af9be36e76bb665012b6cc83b8a85d1091ac77dcc71
if [ -f "$dir/recon.stl" ] && [ "$(shasum -a 256 "$dir/recon.stl" | cut -d' ' -f1)" = "$stl_sha" ]; then
  echo "mesh: $dir/recon.stl (sha256 ok)"; exit 0
fi
if [ ! -f "$dir/recon.zip" ] || [ "$(md5 -q "$dir/recon.zip" 2>/dev/null || md5sum "$dir/recon.zip" | cut -d' ' -f1)" != "$zip_md5" ]; then
  curl -sSL --retry 3 -o "$dir/recon.zip" https://ndownloader.figshare.com/files/58189099
fi
got=$(md5 -q "$dir/recon.zip" 2>/dev/null || md5sum "$dir/recon.zip" | cut -d' ' -f1)
[ "$got" = "$zip_md5" ] || { echo "refusing: zip md5 $got != $zip_md5" >&2; exit 1; }
unzip -o -j -q "$dir/recon.zip" "*reconstruction.stl" -d "$dir"
mv "$dir/Cetotherium riabinini NMNHU-P OF 668_1 reconstruction.stl" "$dir/recon.stl"
got=$(shasum -a 256 "$dir/recon.stl" | cut -d' ' -f1)
[ "$got" = "$stl_sha" ] || { echo "refusing: stl sha256 $got != $stl_sha" >&2; exit 1; }
echo "mesh: $dir/recon.stl (sha256 ok)"
