class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.30"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.30/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "63f72db56c77e10117ee3e12c04914dba9f106fc3ac641f3e92034674a8bb8b8"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.30/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "b5c03f37dd95710793dc4f670a7a835a9533f3b95c448e16fdf5db2751b8a549"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
