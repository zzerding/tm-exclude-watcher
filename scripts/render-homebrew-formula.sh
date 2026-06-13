# ABOUTME: Renders the Homebrew formula for tm-watcher's prebuilt macOS tarballs.

set -euo pipefail

if [[ "$#" -ne 4 ]]; then
  echo "usage: $0 <version> <aarch64-sha256> <x86_64-sha256> <output-path>" >&2
  exit 64
fi

version="$1"
aarch64_sha256="$2"
x86_64_sha256="$3"
output_path="$4"
release_base_url="https://github.com/zzerding/tm-exclude-watcher/releases/download/v${version}"

mkdir -p "$(dirname "$output_path")"

cat > "$output_path" <<FORMULA
class TmWatcher < Formula
  desc "Automatically manage macOS Time Machine exclusions for development directories"
  homepage "https://github.com/zzerding/tm-exclude-watcher"
  license "MIT"
  version "${version}"

  if Hardware::CPU.arm?
    url "${release_base_url}/tm-watcher-v${version}-aarch64-apple-darwin.tar.gz"
    sha256 "${aarch64_sha256}"
  else
    url "${release_base_url}/tm-watcher-v${version}-x86_64-apple-darwin.tar.gz"
    sha256 "${x86_64_sha256}"
  end

  def install
    bin.install "tm-watcher"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/tm-watcher --version")
    assert_match "tm-watcher", shell_output("#{bin}/tm-watcher --help")
  end

  def caveats
    <<~EOS
      tm-watcher is installed but not started automatically.

      To enable background monitoring:
        tm-watcher daemon start

      To check daemon status:
        tm-watcher daemon status

      To stop background monitoring:
        tm-watcher daemon stop
    EOS
  end
end
FORMULA
