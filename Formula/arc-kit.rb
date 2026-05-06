class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.5.7"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.5.7/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "6d595df44b46ba2d92dd4d4c08d96efc818014abe67d1db486f0d85865767df5"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.5.7/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "69c5f6132ce2f49d0e08c7fbf9b6a7699c0a957d7ba534a94bbc6efdff30ecdb"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
