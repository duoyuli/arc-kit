class ArcKit < Formula
  desc "CLI tool for managing coding agent capabilities"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "0.1.30"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v0.1.30/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "f70f7b6686d7bb21acf78eab3f9921f3736ff4ba526ac6430914caad8db6fa95"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v0.1.30/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "a0ecb88eb37677d813a42515ae5f0d0dcb63a2b31577be888ee95a0a74340497"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
