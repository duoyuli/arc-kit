class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.9.15"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.15/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "06f251ffd880f55546ef1de7b9ef1070df951e5fb66465b53f7a0134673ee56a"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.15/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "5296a10644189fb3b7bc268bf4ba374f10888faa6c06980e5ad6bfeb3d37ad33"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
