class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.4.6"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.6/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "4a49cefe1ceb085f3db910eaacac2e31e97c0314cc89b1ee4f6601d954355b7f"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.4.6/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "17f80128000824a8cc73ee30056ecefcaac92ae331c2b6b9b43815239a5351e9"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
