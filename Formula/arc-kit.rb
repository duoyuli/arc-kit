class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.13"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.13/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "200202bdc46d2924f60fdd2744a5c65f751d88741a88817ae5c298aafa9726af"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.13/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "67b5ea2f946269bd3b0e4c4420b4b1fea90284cc7bba7c980ea8d723649fece4"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
