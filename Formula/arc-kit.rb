class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.16"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.16/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "7f925bf603b8a115a915659166df2f73e3a067f9c98c33d560704e289213fa99"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.16/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "9dde4eb03e408a26959c8170c7b45ed2925bc8f8c0837184d959c6fd1a8d3a25"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
