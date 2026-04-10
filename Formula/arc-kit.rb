class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.10"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.10/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "c6e5e69c7f3fbc0318b15278456481dd58bd9ee37346b4306db471959341585c"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.10/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "d252323dd1f74d737ff13e525d1363311fb26684cf50fdcb4f1d42ce41ec3b05"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
