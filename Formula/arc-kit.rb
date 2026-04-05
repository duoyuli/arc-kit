class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.5"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.5/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "77e40e85a4dc29304c99bde6aebab45c14b079bbbe52e0e3c81e4b243e9d815a"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.5/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "88ffcc6a669415f5c42c092afc37b9d1d2ca5de73bb7dc7e1ec2fce5a2920d1a"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
