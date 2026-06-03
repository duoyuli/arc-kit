class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.6.3"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.6.3/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "1703726ee97f7a0804d69b4b07ef01c6c917ff7d9b33a2291d0c4693745e2f0e"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.6.3/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "464a2c88246a0c3d217e8cf87addb9d72d7b60e534377a90962e630f670c7f87"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
