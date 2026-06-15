class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.6.15"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.6.15/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "54470151988782139651294f21457bf5f38c918457fe796d9dd2b9c50a80d50a"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.6.15/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "261338525e2a6af9b6a206180e3ed800aed2b021c0af62f4b53c9ce3727ddb62"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
